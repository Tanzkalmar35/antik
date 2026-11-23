//! Process runner: the only adapter allowed to produce a check result.
//!
//! There is no shell. A program from the allowlist is spawned with an argv
//! array in the project's directory, its own process group, a two-pipe capture
//! that keeps a bounded tail, and a deadline. Timeouts and cancels kill the
//! process group so a test binary cannot outlive the run that started it.

use std::collections::{BTreeSet, VecDeque};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use antik_app::ports::{CancelToken, RunOutput, RunRequest, RunStop, Runner};

/// Kept per stream. A failing test suite prints a lot; the last 64 KiB is what
/// the developer reads.
const TAIL: usize = 64 * 1024;
const POLL: Duration = Duration::from_millis(50);

pub struct ProcessRunner {
    allowed: BTreeSet<String>,
}

impl Default for ProcessRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessRunner {
    /// The production runner: exactly the domain's standard program set.
    pub fn new() -> Self {
        Self {
            allowed: antik_domain::sensor::Program::STANDARD
                .iter()
                .map(|program| program.as_str().to_owned())
                .collect(),
        }
    }

    /// A narrower or test allowlist. Production never calls this; the tests use
    /// it so they can exercise deadlines and capture with a harmless binary.
    pub fn with_allowed(names: &[&str]) -> Self {
        Self {
            allowed: names.iter().map(|name| (*name).to_owned()).collect(),
        }
    }
}

impl Runner for ProcessRunner {
    fn run(&self, request: &RunRequest, cancel: &CancelToken) -> RunOutput {
        let started = Instant::now();
        if !self.allowed.contains(&request.program) {
            return refused(format!(
                "`{}` is not in the program allowlist",
                request.program
            ));
        }
        if !request.cwd.is_dir() {
            return refused(format!(
                "{} is not a directory this run may use",
                request.cwd.display()
            ));
        }

        let mut command = Command::new(&request.program);
        command
            .args(&request.args)
            .current_dir(&request.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            // Its own process group, so a kill takes the children with it.
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return refused(format!("could not start {}: {error}", request.program));
            }
        };
        let pid = child.id();
        let stdout = child
            .stdout
            .take()
            .map(|pipe| thread::spawn(move || tail(pipe)));
        let stderr = child
            .stderr
            .take()
            .map(|pipe| thread::spawn(move || tail(pipe)));
        let deadline = started + Duration::from_millis(request.timeout_ms.max(1));

        let (stop, exit_code) = loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    break (RunStop::Exited(status.code().unwrap_or(-1)), status.code());
                }
                Ok(None) => {}
                Err(error) => break (RunStop::Failed(format!("wait failed: {error}")), None),
            }
            if cancel.is_cancelled() {
                kill_group(&mut child, pid);
                break (RunStop::Cancelled, None);
            }
            if Instant::now() >= deadline {
                kill_group(&mut child, pid);
                break (RunStop::TimedOut, None);
            }
            thread::sleep(POLL);
        };

        let (stdout_tail, stdout_truncated) = stdout.and_then(join).unwrap_or_default();
        let (stderr_tail, stderr_truncated) = stderr.and_then(join).unwrap_or_default();
        RunOutput {
            stop,
            exit_code,
            duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            stdout_tail,
            stderr_tail,
            truncated: stdout_truncated || stderr_truncated,
        }
    }
}

fn refused(message: String) -> RunOutput {
    RunOutput {
        stop: RunStop::Failed(message.clone()),
        exit_code: None,
        duration_ms: 0,
        stdout_tail: String::new(),
        stderr_tail: message,
        truncated: false,
    }
}

fn join(handle: thread::JoinHandle<(String, bool)>) -> Option<(String, bool)> {
    handle.join().ok()
}

/// Read to the end, keeping only the last `TAIL` bytes. Draining matters: a
/// full pipe blocks the child, which would turn a chatty test suite into a
/// timeout.
fn tail(mut pipe: impl Read) -> (String, bool) {
    let mut kept: VecDeque<u8> = VecDeque::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                kept.extend(&buffer[..read]);
                while kept.len() > TAIL {
                    kept.pop_front();
                    truncated = true;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    (
        String::from_utf8_lossy(&kept.into_iter().collect::<Vec<_>>()).into_owned(),
        truncated,
    )
}

/// Kill the whole group, then reap the child so no zombie is left behind.
fn kill_group(child: &mut Child, pid: u32) {
    #[cfg(unix)]
    {
        // `kill(1)` reads a negative pid as a process group. Using the binary
        // keeps the crate free of unsafe code.
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(program: &str, args: &[&str]) -> RunRequest {
        RunRequest {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            cwd: std::env::current_dir().expect("cwd"),
            timeout_ms: 5_000,
        }
    }

    #[test]
    fn the_allowlist_is_the_domain_set() {
        let runner = ProcessRunner::new();
        assert!(runner.allowed.contains("cargo"));
        assert!(!runner.allowed.contains("sh"));
    }

    #[test]
    fn a_program_outside_the_allowlist_never_starts() {
        let runner = ProcessRunner::new();
        let output = runner.run(&request("sh", &["-c", "echo hi"]), &CancelToken::new());
        assert!(matches!(output.stop, RunStop::Failed(_)));
        assert!(output.exit_code.is_none());
    }

    #[test]
    fn the_exit_status_is_the_result() {
        let runner = ProcessRunner::with_allowed(&["sh"]);
        let ok = runner.run(&request("sh", &["-c", "echo done"]), &CancelToken::new());
        assert_eq!(ok.stop, RunStop::Exited(0));
        assert_eq!(ok.exit_code, Some(0));
        assert!(ok.stdout_tail.contains("done"));

        let bad = runner.run(&request("sh", &["-c", "exit 3"]), &CancelToken::new());
        assert_eq!(bad.exit_code, Some(3));
    }

    #[test]
    fn a_slow_process_is_killed_at_the_deadline() {
        let runner = ProcessRunner::with_allowed(&["sh"]);
        let mut req = request("sh", &["-c", "sleep 30"]);
        req.timeout_ms = 200;
        let started = Instant::now();
        let output = runner.run(&req, &CancelToken::new());
        assert_eq!(output.stop, RunStop::TimedOut);
        assert!(output.exit_code.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the deadline has to end the process, not the sleep"
        );
    }

    #[test]
    fn cancelling_ends_the_process_without_a_result() {
        let runner = ProcessRunner::with_allowed(&["sh"]);
        let cancel = CancelToken::new();
        let flag = cancel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            flag.cancel();
        });
        let output = runner.run(&request("sh", &["-c", "sleep 30"]), &cancel);
        assert_eq!(output.stop, RunStop::Cancelled);
    }

    #[test]
    fn the_tail_is_bounded_and_flagged() {
        let runner = ProcessRunner::with_allowed(&["sh"]);
        // 200 KB of output; the tail keeps the last 64 KB.
        let output = runner.run(
            &request("sh", &["-c", "yes X | head -c 200000"]),
            &CancelToken::new(),
        );
        assert_eq!(output.stop, RunStop::Exited(0));
        assert!(output.truncated, "long output must be marked truncated");
        assert!(output.stdout_tail.len() <= TAIL);
    }

    #[test]
    fn a_missing_program_reports_a_failure_not_a_pass() {
        let runner = ProcessRunner::with_allowed(&["antik-not-a-real-binary"]);
        let output = runner.run(
            &request("antik-not-a-real-binary", &[]),
            &CancelToken::new(),
        );
        assert!(matches!(output.stop, RunStop::Failed(_)));
        assert!(!output.stderr_tail.is_empty());
    }
}

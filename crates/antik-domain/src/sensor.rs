//! Sensor policy: what a check is allowed to run, and how its spec is shaped.
//!
//! A check is a program from a fixed set plus arguments. There is no shell, so
//! a spec can describe a command line but never a pipeline, a redirect or an
//! interpolation. That is the whole safety model: the exit status is the only
//! result, and nothing the developer types becomes shell text.

use serde::{Deserialize, Serialize};

/// Two minutes is long enough for a test suite and short enough that a hung
/// process is noticed in the same session.
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;
pub const MAX_ARGS: usize = 32;
pub const MAX_ARG_LEN: usize = 512;

/// The programs a check may name. Anything else is refused before a process
/// is spawned, in the domain and again at the process edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Program {
    Cargo,
    Bun,
    Npm,
    Node,
    Tsc,
    Pytest,
    Go,
}

impl Program {
    /// The complete allowlist. There is no way to add to it at runtime.
    pub const STANDARD: [Program; 7] = [
        Program::Cargo,
        Program::Bun,
        Program::Npm,
        Program::Node,
        Program::Tsc,
        Program::Pytest,
        Program::Go,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Bun => "bun",
            Self::Npm => "npm",
            Self::Node => "node",
            Self::Tsc => "tsc",
            Self::Pytest => "pytest",
            Self::Go => "go",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        Self::STANDARD
            .into_iter()
            .find(|program| program.as_str() == value)
    }
}

impl std::fmt::Display for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One executable check. Stored as JSON in `checks.spec_json` so a later
/// change to the allowlist cannot reinterpret an old row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckSpec {
    pub program: Program,
    #[serde(default)]
    pub args: Vec<String>,
}

impl CheckSpec {
    /// The authoring lane: a program name and the arguments as typed.
    pub fn new(program: &str, args: &str) -> Result<Self, String> {
        let program = Program::parse(program)
            .ok_or_else(|| format!("`{program}` is not a program a check may run"))?;
        Ok(Self {
            program,
            args: split_args(args)?,
        })
    }

    /// A drill stores its command as one line, so the first word is the
    /// program and the rest are arguments.
    pub fn from_line(line: &str) -> Result<Self, String> {
        let mut words = line.split_whitespace();
        let program = words
            .next()
            .ok_or_else(|| "a drill needs a command".to_owned())?;
        Self::new(program, words.collect::<Vec<_>>().join(" ").as_str())
    }

    pub fn command_line(&self) -> String {
        if self.args.is_empty() {
            self.program.to_string()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// Whitespace splitting only. Quoting is deliberately unsupported: a shell
/// would need it, and there is no shell.
pub fn split_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    for token in input.split_whitespace() {
        if token.len() > MAX_ARG_LEN {
            return Err(format!("an argument is longer than {MAX_ARG_LEN} bytes"));
        }
        if let Some(bad) = token
            .chars()
            .find(|c| matches!(c, ';' | '|' | '&' | '>' | '<' | '`' | '$'))
        {
            return Err(format!(
                "`{bad}` needs a shell, and checks run without one. Pass options and paths only."
            ));
        }
        args.push(token.to_owned());
    }
    if args.len() > MAX_ARGS {
        return Err(format!("a check may pass at most {MAX_ARGS} arguments"));
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_standard_programs_parse() {
        assert_eq!(Program::parse("cargo"), Some(Program::Cargo));
        assert_eq!(Program::parse("sh"), None);
        assert_eq!(Program::parse("/bin/sh"), None);
        assert_eq!(Program::STANDARD.len(), 7);
    }

    #[test]
    fn a_spec_round_trips_as_json() {
        let spec = CheckSpec::new("cargo", "test --lib").unwrap();
        assert_eq!(spec.program, Program::Cargo);
        assert_eq!(spec.args, vec!["test", "--lib"]);
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(json, r#"{"program":"cargo","args":["test","--lib"]}"#);
        assert_eq!(spec.command_line(), "cargo test --lib");
    }

    #[test]
    fn shell_syntax_is_refused_because_there_is_no_shell() {
        assert!(CheckSpec::new("cargo", "test && rm -rf /").is_err());
        assert!(CheckSpec::new("cargo", "test | head").is_err());
        assert!(CheckSpec::new("cargo", "test $(whoami)").is_err());
        assert!(CheckSpec::new("sh", "test").is_err());
    }

    #[test]
    fn a_drill_line_splits_into_program_and_args() {
        let spec = CheckSpec::from_line("cargo test --test m4_sensors").unwrap();
        assert_eq!(spec.program, Program::Cargo);
        assert_eq!(spec.args, vec!["test", "--test", "m4_sensors"]);
        assert!(CheckSpec::from_line("").is_err());
    }

    #[test]
    fn arguments_are_capped() {
        assert!(split_args(&"a".repeat(MAX_ARG_LEN + 1)).is_err());
        let many = vec!["x"; MAX_ARGS + 1].join(" ");
        assert!(split_args(&many).is_err());
    }
}

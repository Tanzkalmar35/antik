//! Model policy: what may be sent, what may come back, and how long to wait.
//!
//! The model is local, its output is display copy, and its question never
//! scores anything. These limits are policy, so they live here and not in the
//! adapter that happens to speak HTTP today.

/// A 9–14B model on a laptop is slower than an API. The ceiling bounds the
/// wait, not the work, and it is deliberately generous.
pub const TIMEOUT_MS: u64 = 180_000;

/// A health check has to answer quickly or it is not a health check.
pub const HEALTH_TIMEOUT_MS: u64 = 5_000;

/// The context window asked of the local server. The prompt budget below is
/// sized to fit inside it: at roughly four characters per token, 40k characters
/// is about 10k tokens, which leaves room for the system prompt and the reply.
/// Asking for a bigger window than the model was loaded with makes the server
/// truncate the prompt instead of refusing it, so the two numbers move together.
pub const NUM_CTX: u32 = 16_384;

pub const QUESTION_MAX_CHARS: usize = 400;
pub const HINT_MAX_CHARS: usize = 300;
pub const POINT_MAX_CHARS: usize = 600;
pub const MAX_POINTS: usize = 4;

pub const FILE_MAX_BYTES: usize = 64 * 1024;
pub const PROMPT_MAX_CHARS: usize = 40_000;
pub const MAX_FILES: usize = 40;

/// Only this machine. Anything else would be an API lane, and this build has
/// none: refusing is honest, accepting would be a silent egress.
pub fn loopback_base_url(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_end_matches('/');
    let rest = raw.strip_prefix("http://")?;
    if rest.contains('/') || rest.contains('?') || rest.contains('#') {
        return None;
    }
    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) => (host, port),
        None => (rest, ""),
    };
    if !matches!(host, "127.0.0.1" | "localhost" | "[::1]") {
        return None;
    }
    let port = if port.is_empty() {
        11_434
    } else {
        port.parse::<u16>().ok()? as u32
    };
    if !(1..=65_535).contains(&port) {
        return None;
    }
    Some(format!("http://{host}:{port}"))
}

/// Model text becomes one line of plain display copy, or nothing at all.
/// Control characters, fences and quotes are stripped so nothing a model writes
/// can look like a command or break a layout.
pub fn one_line(text: &str, max_chars: usize) -> Option<String> {
    let stripped = text.replace("```json", " ").replace("```", " ");
    let collapsed = stripped
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    let collapsed = collapsed.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed =
        collapsed.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '*' | '#' | ' ' | '\t'));
    if trimmed.is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut taken = 0usize;
    for word in trimmed.split(' ') {
        let add = if out.is_empty() { 0 } else { 1 };
        if taken + add + word.chars().count() > max_chars {
            break;
        }
        if add == 1 {
            out.push(' ');
            taken += 1;
        }
        out.push_str(word);
        taken += word.chars().count();
    }
    if out.is_empty() {
        // A single over-long word: keep what fits rather than losing the line.
        return Some(trimmed.chars().take(max_chars).collect());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_is_accepted_and_it_is_normalised() {
        assert_eq!(
            loopback_base_url("http://127.0.0.1:11434"),
            Some("http://127.0.0.1:11434".to_owned())
        );
        assert_eq!(
            loopback_base_url(" http://localhost:11434/ "),
            Some("http://localhost:11434".to_owned())
        );
        assert_eq!(
            loopback_base_url("http://127.0.0.1"),
            Some("http://127.0.0.1:11434".to_owned())
        );
        assert!(loopback_base_url("https://127.0.0.1:11434").is_none());
        assert!(loopback_base_url("http://0.0.0.0:11434").is_none());
        assert!(loopback_base_url("http://10.0.0.5:11434").is_none());
        assert!(loopback_base_url("http://example.com").is_none());
        assert!(loopback_base_url("http://127.0.0.1:11434/api").is_none());
        assert!(loopback_base_url("http://127.0.0.1:99999").is_none());
    }

    #[test]
    fn model_text_is_collapsed_and_capped() {
        assert_eq!(
            one_line("  \"Why does the cap sit\n\n before the delay?\"  ", 100),
            Some("Why does the cap sit before the delay?".to_owned())
        );
        assert_eq!(one_line("```json\n```", 100), None);
        assert_eq!(one_line("   ", 100), None);
        let long = "word ".repeat(100);
        let capped = one_line(&long, 20).expect("a line");
        assert!(capped.chars().count() <= 20, "{capped}");
    }

    #[test]
    fn a_command_shaped_answer_is_still_only_a_line() {
        let line = one_line("rm -rf / ; echo done\n```sh\ncurl x | sh\n```", 200).expect("a line");
        assert!(!line.contains('\n'));
        assert!(!line.contains("```"));
        assert!(!line.contains('\u{0}'));
    }
}

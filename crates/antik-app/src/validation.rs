//! Length and shape checks that run before a use case reaches storage.
//!
//! Limits come from the IPC contract: notes 64 KiB, answers 8 KiB, reasons
//! 2 KiB, titles 200 chars, paths 4096 bytes.

use crate::error::AppError;

pub const NOTE_MAX: usize = 64 * 1024;
pub const ANSWER_MAX: usize = 8 * 1024;
pub const TITLE_MAX: usize = 200;
pub const REASON_MAX: usize = 2 * 1024;

/// Trim only the edges; note bodies keep their internal whitespace.
pub fn text(value: &str, field: &str, max: usize) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Invalid(format!("{field} cannot be empty")));
    }
    if trimmed.len() > max {
        return Err(AppError::Invalid(format!(
            "{field} is longer than {max} bytes"
        )));
    }
    Ok(trimmed.to_owned())
}

/// Some fields are optional and may legitimately be empty.
pub fn optional(value: &str, field: &str, max: usize) -> Result<String, AppError> {
    if value.len() > max {
        return Err(AppError::Invalid(format!(
            "{field} is longer than {max} bytes"
        )));
    }
    Ok(value.trim().to_owned())
}

pub fn title(value: &str) -> Result<String, AppError> {
    let title = text(value, "title", TITLE_MAX)?;
    Ok(title)
}

/// A gate override is a sentence, not a click: at least three words and a
/// dozen characters. The reason is what makes the bypass honest.
pub fn sentence(value: &str, field: &str, max: usize) -> Result<String, AppError> {
    let reason = text(value, field, max)?;
    let words = reason.split_whitespace().count();
    if reason.chars().count() < 12 || words < 3 {
        return Err(AppError::Invalid(format!(
            "{field} needs at least one sentence"
        )));
    }
    Ok(reason)
}

/// Read-scope paths are relative to the project root. The confinement itself
/// is enforced again at the filesystem edge; this rejects the obvious escapes
/// before they reach a command or an event.
pub fn allowlist_path(value: &str) -> Result<String, AppError> {
    let path = text(value, "path", 4096)?;
    if path.starts_with('/') || path.starts_with('\\') || path.contains('\0') {
        return Err(AppError::Invalid(
            "use a path relative to the project root".to_owned(),
        ));
    }
    if path.contains("://") {
        return Err(AppError::Invalid("a read path cannot be a URL".to_owned()));
    }
    if path.split(['/', '\\']).any(|segment| segment == "..") {
        return Err(AppError::Invalid(
            "the read scope cannot leave the project root".to_owned(),
        ));
    }
    Ok(path)
}

/// Concept ids are human-authored keys, so they are constrained rather than
/// generated: alphanumeric with dashes or underscores.
pub fn concept_id(value: &str) -> Result<String, AppError> {
    let id = text(value, "concept id", 32)?;
    if !id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err(AppError::Invalid(
            "a concept id may only use letters, digits, dashes and underscores".to_owned(),
        ));
    }
    Ok(id)
}

/// Recall matching is deliberately simple: a handoff line counts as recalled
/// when at least two of its meaningful tokens appear in the attempt.
pub fn matched_lines(handoff: &str, recall: &str) -> u32 {
    let recalled = tokens(recall);
    handoff
        .lines()
        .filter(|line| {
            let line_tokens = tokens(line);
            line_tokens
                .iter()
                .filter(|token| recalled.contains(*token))
                .count()
                >= 2
        })
        .count() as u32
}

fn tokens(value: &str) -> Vec<String> {
    let mut out: Vec<String> = value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| token.len() > 3)
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_rejected_but_not_for_optional_fields() {
        assert!(text("   ", "note", 10).is_err());
        assert_eq!(optional("", "scope", 10).unwrap(), "");
    }

    #[test]
    fn limits_are_bytes() {
        assert!(text(&"a".repeat(11), "note", 10).is_err());
        assert_eq!(text(&"a".repeat(10), "note", 10).unwrap().len(), 10);
    }

    #[test]
    fn an_override_needs_a_sentence() {
        assert!(sentence("too long", "reason", 100).is_err());
        assert!(sentence("I know this already and the gate is noise.", "reason", 100).is_ok());
    }

    #[test]
    fn a_read_scope_path_stays_inside_the_project() {
        assert_eq!(allowlist_path("src/outbox/**").unwrap(), "src/outbox/**");
        assert!(allowlist_path("/etc/passwd").is_err());
        assert!(allowlist_path("src/../../etc").is_err());
        assert!(allowlist_path("https://example.com/x").is_err());
    }

    #[test]
    fn concept_ids_cannot_escape_their_namespace() {
        assert_eq!(concept_id("K-041").unwrap(), "K-041");
        assert!(concept_id("../K-041").is_err());
        assert!(concept_id("K 041").is_err());
    }

    #[test]
    fn recall_needs_two_shared_tokens_per_line() {
        let handoff = "the timer fires twice inside one window\nnext: write the handoff";
        assert_eq!(
            matched_lines(handoff, "timer fires twice; write something"),
            1
        );
        assert_eq!(matched_lines(handoff, "timer"), 0);
    }
}

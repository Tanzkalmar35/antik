//! Read scope: which project files the model may see.
//!
//! The matcher is pure and runs before anything touches the disk. A pattern is
//! project-relative and `/`-separated, may use `*` and `?` within one segment and
//! `**` for any depth, and names a subtree when it ends on a real segment
//! (`src/queue` covers `src/queue/flush.rs`). Absolute paths, `..`, backslashes
//! and control characters are refused, so a stored pattern cannot name anything
//! outside the project even if it is edited by hand in SQLite.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Deep,
    Text(String),
}

impl Pattern {
    /// Parses one allowlist entry. `None` means the entry is not usable and is
    /// dropped: an unusable entry opens nothing, never everything.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() || raw.len() > 512 || raw.starts_with('/') || raw.contains('\\') {
            return None;
        }
        if raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return None;
        }
        let mut segments = Vec::new();
        for part in raw.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                return None;
            }
            if part == "**" {
                segments.push(Segment::Deep);
                continue;
            }
            segments.push(Segment::Text(part.to_owned()));
        }
        if segments.is_empty() {
            return None;
        }
        Some(Self { segments })
    }

    /// True when `path` (project-relative) is inside this pattern.
    pub fn matches(&self, path: &str) -> bool {
        match parts(path) {
            Some(path) => walk(&self.segments, &path),
            None => false,
        }
    }
}

/// Any of the patterns. The empty list opens nothing.
pub fn allows(patterns: &[Pattern], path: &str) -> bool {
    patterns.iter().any(|pattern| pattern.matches(path))
}

/// A batch's scope is a comma- or newline-separated list of project-relative
/// patterns. Unusable entries are dropped rather than rejected: the scope is a
/// description of the work, and the allowlist is the permission.
pub fn parse_scope(raw: &str) -> Vec<Pattern> {
    raw.split([',', '\n'])
        .filter_map(|entry| Pattern::parse(entry))
        .collect()
}

fn parts(path: &str) -> Option<Vec<&str>> {
    let path = path.trim();
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return None;
    }
    let mut out = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            other if other.chars().any(char::is_control) => return None,
            other => out.push(other),
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn walk(pattern: &[Segment], path: &[&str]) -> bool {
    let Some((first, rest)) = pattern.split_first() else {
        // The pattern is exhausted, so everything beneath the matched point is
        // inside it: `src/queue` covers the folder, not just the name.
        return true;
    };
    match first {
        Segment::Deep => (0..=path.len()).any(|skip| walk(rest, &path[skip..])),
        Segment::Text(segment) => match path.split_first() {
            Some((head, tail)) => wildcard(segment, head) && walk(rest, tail),
            None => false,
        },
    }
}

/// `*` matches any run inside one segment, `?` exactly one character.
fn wildcard(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(at) = star {
            pi = at + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(raw: &str) -> Pattern {
        Pattern::parse(raw).expect("a usable pattern")
    }

    #[test]
    fn a_literal_entry_covers_its_subtree() {
        let queue = pattern("src/queue");
        assert!(queue.matches("src/queue"));
        assert!(queue.matches("src/queue/flush.rs"));
        assert!(queue.matches("src/queue/nested/deep.rs"));
        assert!(!queue.matches("src/queue_other/flush.rs"));
        assert!(!queue.matches("src/flush.rs"));
    }

    #[test]
    fn a_star_stays_inside_one_segment() {
        let sources = pattern("src/*.rs");
        assert!(sources.matches("src/main.rs"));
        assert!(!sources.matches("src/nested/main.rs"));
        assert!(!sources.matches("tests/main.rs"));
        assert!(pattern("src/*_test.rs").matches("src/outbox_test.rs"));
        assert!(pattern("tests/?.rs").matches("tests/a.rs"));
        assert!(!pattern("tests/?.rs").matches("tests/ab.rs"));
    }

    #[test]
    fn a_deep_segment_crosses_folders() {
        let all_under_src = pattern("src/**");
        assert!(all_under_src.matches("src/a.rs"));
        assert!(all_under_src.matches("src/a/b/c.rs"));
        assert!(!all_under_src.matches("tests/a.rs"));
        assert!(pattern("**/migration.sql").matches("db/migration.sql"));
        assert!(pattern("**/migration.sql").matches("db/old/migration.sql"));
    }

    #[test]
    fn escapes_are_refused_rather_than_matched() {
        assert!(Pattern::parse("").is_none());
        assert!(Pattern::parse("   ").is_none());
        assert!(Pattern::parse("/etc/passwd").is_none());
        assert!(Pattern::parse("../../etc/passwd").is_none());
        assert!(Pattern::parse("src/../../etc").is_none());
        assert!(Pattern::parse("src\\queue").is_none());
        assert!(Pattern::parse("src/queue\nflush.rs").is_none());
        // And a path cannot smuggle itself out either.
        assert!(!pattern("**").matches("../outside.txt"));
        assert!(!pattern("src/**").matches("/etc/passwd"));
    }

    #[test]
    fn a_batch_scope_is_a_list_of_patterns() {
        let patterns = parse_scope("src/queue/**, tests/outbox/**\nnotes/quorum.md");
        assert_eq!(patterns.len(), 3);
        assert!(allows(&patterns, "src/queue/flush.rs"));
        assert!(allows(&patterns, "notes/quorum.md"));
        assert!(!allows(&patterns, "src/billing/invoice.rs"));
        // An entry that cannot be confined is dropped, never widened.
        assert!(parse_scope("/etc/passwd, ../outside").is_empty());
    }

    #[test]
    fn the_scope_is_the_union_and_nothing_else() {
        let patterns: Vec<Pattern> = ["src/queue/**", "tests/outbox/**"]
            .iter()
            .filter_map(|raw| Pattern::parse(raw))
            .collect();
        assert!(allows(&patterns, "src/queue/flush.rs"));
        assert!(allows(&patterns, "tests/outbox/t1.rs"));
        assert!(!allows(&patterns, "src/billing/invoice.rs"));
        assert!(!allows(&[], "src/queue/flush.rs"));
    }
}

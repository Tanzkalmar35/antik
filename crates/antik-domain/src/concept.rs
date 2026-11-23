//! Concept review policy: a pass pushes the concept out, a thin answer keeps it
//! close. Pure, table-tested, and independent of which model (if any) is set.

use crate::gate::tokens;
use crate::time::Timestamp;

/// A passing explanation is resurfaced three weeks later.
pub const PASS_INTERVAL_DAYS: i64 = 21;
/// A thin one comes back tomorrow.
pub const THIN_INTERVAL_DAYS: i64 = 1;
const MILLIS_PER_DAY: i64 = 86_400_000;
const MIN_WORDS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptVerdict {
    Pass,
    Thin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptReview {
    pub verdict: ConceptVerdict,
    pub note: String,
}

impl ConceptReview {
    pub fn passed(&self) -> bool {
        self.verdict == ConceptVerdict::Pass
    }
}

/// An answer passes when it is long enough to be an explanation and shares at
/// least two meaningful words with the concept's name or definition. The check
/// is deliberately shallow: it is an honesty floor, not a grader.
pub fn review_concept(name: &str, definition: &str, answer: &str) -> ConceptReview {
    let words = answer
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .count();
    if words < MIN_WORDS {
        return ConceptReview {
            verdict: ConceptVerdict::Thin,
            note: format!(
                "Thin. {words} words is shorter than an explanation; use at least {MIN_WORDS}."
            ),
        };
    }

    let concept = tokens(&format!("{name} {definition}"));
    let said = tokens(answer);
    let needed = concept.len().clamp(1, 2);
    let shared = concept.iter().filter(|token| said.contains(token)).count();

    if shared >= needed {
        ConceptReview {
            verdict: ConceptVerdict::Pass,
            note: "Recorded. Next review is three weeks out.".to_owned(),
        }
    } else {
        ConceptReview {
            verdict: ConceptVerdict::Thin,
            note: "Thin. It never names the concept itself; it stays due tomorrow.".to_owned(),
        }
    }
}

pub fn next_due(now: Timestamp, verdict: ConceptVerdict) -> Timestamp {
    let days = match verdict {
        ConceptVerdict::Pass => PASS_INTERVAL_DAYS,
        ConceptVerdict::Thin => THIN_INTERVAL_DAYS,
    };
    Timestamp::from_millis(now.as_millis() + days * MILLIS_PER_DAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    #[test]
    fn a_pass_moves_out_three_weeks() {
        assert_eq!(
            next_due(NOW, ConceptVerdict::Pass).as_millis(),
            NOW.as_millis() + 21 * MILLIS_PER_DAY
        );
    }

    #[test]
    fn a_thin_answer_returns_tomorrow() {
        assert_eq!(
            next_due(NOW, ConceptVerdict::Thin).as_millis(),
            NOW.as_millis() + MILLIS_PER_DAY
        );
    }

    #[test]
    fn an_answer_that_names_the_concept_passes() {
        let review = review_concept(
            "Idempotency",
            "The same request applied twice leaves the same state.",
            "Idempotency means applying the same request twice leaves one state, not two.",
        );
        assert!(review.passed(), "{review:?}");
    }

    #[test]
    fn a_generic_answer_stays_thin() {
        let review = review_concept(
            "Idempotency",
            "The same request applied twice leaves the same state.",
            "It is when you make sure that doing something more than once is fine.",
        );
        assert!(!review.passed(), "{review:?}");
        assert!(review.note.contains("stays due"), "{}", review.note);
    }

    #[test]
    fn a_short_answer_is_thin_before_the_overlap_check() {
        let review = review_concept(
            "Idempotency",
            "The same request applied twice leaves the same state.",
            "Idempotency is repetition.",
        );
        assert!(!review.passed());
        assert!(review.note.starts_with("Thin."));
    }
}

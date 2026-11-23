//! Gate policy: a rubric is data, scoring is a pure function.
//!
//! The model may draft a question in M5, but it never scores one. Every gate
//! carries the criteria it was drafted against, and this module decides what
//! "met" means without IO, storage or a network.

use serde::{Deserialize, Serialize};

/// Stop words are dropped from generated rubrics so a title like "Retry policy
/// for the outbox" does not make the answer match on "for" or "the".
const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "in", "into", "is", "it", "of",
    "on", "or", "that", "the", "then", "this", "to", "with",
];

/// The cost vocabulary is deliberately generic: a gate asks what breaks, not
/// which words the developer happened to use in the plan.
const COST_TERMS: &[&str] = &[
    "break",
    "cost",
    "downstream",
    "expensive",
    "fail",
    "failure",
    "latency",
    "leak",
    "loss",
    "rework",
    "risk",
    "stale",
    "timeout",
    "waste",
    "wrong",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateRubric {
    #[serde(default = "default_min_words")]
    pub min_words: u32,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    pub id: String,
    pub label: String,
    /// A token matches when it equals an expected term or its singular form.
    pub expect: Vec<String>,
    #[serde(default = "default_min_hits")]
    pub min_hits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateScore {
    pub passed: bool,
    pub criteria_hit: u32,
    pub criteria_total: u32,
    /// Shown next to the attempt. Short, and it names what is missing.
    pub note: String,
}

/// An app-authored gate, drafted from the task rather than from a model.
#[derive(Debug, Clone)]
pub struct GateDraft {
    pub question: String,
    pub hint: String,
    pub rubric: GateRubric,
}

impl GateDraft {
    pub fn from_task(title: &str, scope: &str) -> Self {
        let mechanism = rubric_terms(&format!("{title} {scope}"));
        let mechanism = if mechanism.is_empty() {
            vec!["approach".to_owned(), "decision".to_owned()]
        } else {
            mechanism
        };

        let question = format!(
            "What does \"{title}\" change, and what would break if it were done the naive way?"
        );
        let hint = if scope.trim().is_empty() {
            "Name the decision you made and the failure it prevents.".to_owned()
        } else {
            format!("Start from `{scope}`: what does it control, and what fails without it?")
        };

        Self {
            question,
            hint,
            rubric: GateRubric {
                min_words: 10,
                criteria: vec![
                    Criterion {
                        id: "mechanism".to_owned(),
                        label: "the mechanism".to_owned(),
                        expect: mechanism,
                        min_hits: 1,
                    },
                    Criterion {
                        id: "consequence".to_owned(),
                        label: "the cost of getting it wrong".to_owned(),
                        expect: COST_TERMS.iter().map(|term| (*term).to_owned()).collect(),
                        min_hits: 1,
                    },
                ],
            },
        }
    }

    /// A gate drafted from a concept asks the developer to use the idea, not
    /// only to describe the batch. The rubric is the concept's own vocabulary.
    pub fn from_concept(name: &str, definition: &str, area: &str) -> Self {
        let mechanism = rubric_terms(&format!("{name} {definition}"));
        let mechanism = if mechanism.is_empty() {
            vec!["concept".to_owned(), "idea".to_owned()]
        } else {
            mechanism
        };
        let question = format!(
            "Explain \"{name}\" as it applies to this batch: what is it, and what goes wrong without it?"
        );
        let hint = if area.trim().is_empty() {
            format!("Start from the failure \"{name}\" prevents.")
        } else {
            format!("Start from the failure \"{name}\" prevents in {area}.")
        };

        Self {
            question,
            hint,
            rubric: GateRubric {
                min_words: 10,
                criteria: vec![
                    Criterion {
                        id: "mechanism".to_owned(),
                        label: "the concept itself".to_owned(),
                        expect: mechanism,
                        min_hits: 1,
                    },
                    Criterion {
                        id: "consequence".to_owned(),
                        label: "the cost of getting it wrong".to_owned(),
                        expect: COST_TERMS.iter().map(|term| (*term).to_owned()).collect(),
                        min_hits: 1,
                    },
                ],
            },
        }
    }
}

impl GateRubric {
    pub fn score(&self, answer: &str) -> GateScore {
        let words = answer
            .split_whitespace()
            .filter(|word| !word.is_empty())
            .count() as u32;
        let tokens = tokens(answer);

        let mut criteria_hit = 0;
        let mut missing = None;
        for criterion in &self.criteria {
            let hits = criterion
                .expect
                .iter()
                .filter(|expected| {
                    let expected = singular(&expected.to_lowercase());
                    tokens.contains(&expected)
                })
                .count() as u32;
            if hits >= criterion.min_hits.max(1) {
                criteria_hit += 1;
            } else if missing.is_none() {
                missing = Some(criterion.label.as_str());
            }
        }

        let criteria_total = self.criteria.len() as u32;
        let passed = words >= self.min_words && criteria_hit == criteria_total;
        let note = if passed {
            format!("Met. {criteria_hit} of {criteria_total} criteria, {words} words.")
        } else if words < self.min_words {
            format!(
                "Not met. {words} words is shorter than an explanation; at least {} are needed.",
                self.min_words
            )
        } else {
            match missing {
                Some(label) => format!("Not met. The answer never names {label}."),
                None => "Not met. The answer misses the rubric.".to_owned(),
            }
        };

        GateScore {
            passed,
            criteria_hit,
            criteria_total,
            note,
        }
    }
}

fn default_min_words() -> u32 {
    10
}

fn default_min_hits() -> u32 {
    1
}

/// Meaningful tokens from a title and scope, deduplicated and bounded.
fn rubric_terms(value: &str) -> Vec<String> {
    let mut terms = tokens(value)
        .into_iter()
        .filter(|token| !STOP_WORDS.contains(&token.as_str()))
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms.truncate(6);
    terms
}

/// Lowercase, alphanumeric, at least three characters, singularised and
/// deduplicated. Shared by gate and concept scoring so both read an answer the
/// same way.
pub(crate) fn tokens(value: &str) -> Vec<String> {
    let mut tokens = value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() >= 3)
        .map(|token| singular(&token.to_lowercase()))
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn singular(token: &str) -> String {
    let length = token.len();
    if length > 4 && token.ends_with("ies") {
        let mut stem = token[..length - 3].to_owned();
        stem.push('y');
        return stem;
    }
    if length > 3 && token.ends_with('s') && !token.ends_with("ss") {
        return token[..length - 1].to_owned();
    }
    token.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rubric() -> GateRubric {
        GateRubric {
            min_words: 5,
            criteria: vec![
                Criterion {
                    id: "mechanism".to_owned(),
                    label: "the mechanism".to_owned(),
                    expect: vec!["cap".to_owned(), "attempt".to_owned()],
                    min_hits: 1,
                },
                Criterion {
                    id: "cost".to_owned(),
                    label: "the cost".to_owned(),
                    expect: vec!["timeout".to_owned(), "leak".to_owned()],
                    min_hits: 1,
                },
            ],
        }
    }

    #[test]
    fn passes_only_when_every_criterion_and_the_word_floor_are_met() {
        let score = rubric().score("The cap exists because a downstream timeout is expensive.");
        assert!(score.passed, "{score:?}");
        assert_eq!(score.criteria_hit, 2);
    }

    #[test]
    fn names_the_first_missing_criterion() {
        let score = rubric().score("The cap is decided before any delay is computed here.");
        assert!(!score.passed);
        assert!(score.note.contains("the cost"), "{}", score.note);
    }

    #[test]
    fn enforces_the_word_floor_before_the_rubric() {
        let score = rubric().score("Cap timeout.");
        assert!(!score.passed);
        assert!(score.note.contains("shorter"), "{}", score.note);
    }

    #[test]
    fn plural_forms_match_their_singular() {
        let score = rubric().score("The attempts stop at the cap before a timeout happens.");
        assert!(score.passed, "{score:?}");
    }

    #[test]
    fn a_draft_always_names_a_mechanism_and_a_consequence() {
        let draft = GateDraft::from_task("Retry policy for the outbox", "src/outbox/policy.ts");
        assert!(draft.question.contains("Retry policy"));
        assert_eq!(draft.rubric.criteria.len(), 2);
        assert!(
            draft.rubric.criteria[0]
                .expect
                .iter()
                .any(|term| term == "outbox")
        );
        assert!(
            draft
                .rubric
                .score("The outbox policy stops retrying to avoid a timeout downstream.")
                .passed
        );
    }

    #[test]
    fn a_concept_draft_scores_the_concept_and_its_cost() {
        let draft = GateDraft::from_concept(
            "attempt cap",
            "The line past which a retry becomes a leak.",
            "reliability",
        );
        assert!(draft.question.contains("attempt cap"));
        assert!(
            draft
                .rubric
                .score("The attempt cap stops retries so the downstream does not fail.",)
                .passed
        );
        assert!(
            !draft
                .rubric
                .score("It is a number that stops the loop when it gets too large.",)
                .passed
        );
    }

    #[test]
    fn a_title_without_meaningful_words_falls_back_to_a_generic_mechanism() {
        let draft = GateDraft::from_task("Do it", "");
        assert!(
            draft.rubric.criteria[0]
                .expect
                .contains(&"decision".to_owned())
        );
    }
}

//! The questions a part asks and the verdicts its answers give. Jev answers
//! no prose, so a verdict has no reason.

use mergegate_core::judge::{Request, Verdict};
use mergegate_core::triage::Tier;
use serde_json::{Map, Value, json};

/// Lowest first, as Jev wants a score's levels.
const LEVELS: [&str; 10] = [
    "Nothing to review: generated output, a lockfile, formatting or a mechanical rename",
    "Trivial: comments, wording, test fixtures",
    "Minor: a small local edit whose effect is obvious",
    "Routine: straightforward code that follows the patterns around it",
    "Moderate: new logic whose effect stays local",
    "Notable: logic other code relies on",
    "Significant: changes what callers or users see",
    "Important: a public interface, data model, configuration or migration",
    "Critical: security, authentication, money, data loss or concurrency",
    "Central: where a mistake in this change would do the most damage",
];

/// The questions about `files`, indexes into the request's files: how much
/// each needs a reviewer, and for one a rule trusted, whether it needs a
/// look anyway.
pub(crate) fn questions(request: &Request, files: &[usize]) -> Value {
    let mut questions = Map::new();
    for &index in files {
        let file = &request.files[index];
        let n = index + 1;
        questions.insert(
            format!("f{n}_importance"),
            json!({
                "type": "score",
                "instructions": format!(
                    "How carefully must the human reviewing this change read file {n}, {}? \
                     Judge it against the whole change.",
                    file.path
                ),
                "criteria": LEVELS,
            }),
        );
        if file.tier == Tier::Trusted {
            questions.insert(
                format!("f{n}_look"),
                json!({
                    "type": "noul",
                    "instructions": format!(
                        "A rule marked file {n}, {}, safe to skip in review. \
                         Does it need a human look anyway?",
                        file.path
                    ),
                    "criteria": {
                        "true": "It changes behaviour, security or data in a way a reviewer must see",
                        "false": "It is as routine as the rule assumes",
                    },
                }),
            );
        }
    }
    Value::Object(questions)
}

/// A verdict for each of `files` whose importance came back as a score.
pub(crate) fn verdicts(request: &Request, files: &[usize], answers: &Value) -> Vec<Verdict> {
    files
        .iter()
        .filter_map(|&index| {
            let n = index + 1;
            let score = answers[format!("f{n}_importance")]["score"].as_f64()?;
            let top = (LEVELS.len() - 1) as f64;
            let importance = 1.0 + (score.clamp(0.0, top) * 99.0 / top).round();
            Some(Verdict {
                path: request.files[index].path.clone(),
                importance: importance as u8,
                reason: None,
                promote: answers[format!("f{n}_look")]["noul"].as_f64(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "ask_tests.rs"]
mod tests;

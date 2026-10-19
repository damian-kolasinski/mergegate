//! Telling a bot's summary from its findings. Review bots post an overview
//! of the pull request, or of the review they just left, besides a comment
//! for each thing they found. The overview is for the operator to read: it
//! asks nothing of the agent, so holding it for a vote would only ask the
//! operator to rule on it, again with every edit the bot makes to it.

use crate::github::PullComment;

/// Whether `comment` is a bot's summary: Codex's, CodeRabbit's or Copilot's
/// overview of the pull request or of one of their reviews, told by the
/// markers the bots put in it. A person's comment never is, whatever it
/// quotes.
pub fn summary(comment: &PullComment) -> bool {
    if !comment.bot {
        return false;
    }
    let body = comment.body.trim_start();
    let has = |marker: &str| body.contains(marker);
    let codex = has("<!-- codex-pull-request-review-summary -->")
        || has("<!-- codex-review-receipt:")
        // A receipt from before Codex marked them.
        || body.starts_with("### 💡 Codex Review");
    // CodeRabbit puts the findings it can't post inline in its review's body,
    // where they're as much for the agent as the inline ones.
    let coderabbit_review =
        has("<!-- This is an auto-generated comment by CodeRabbit for review status -->")
            && !has("<!-- cr-comment:")
            && ![
                "Outside diff range comments",
                "Nitpick comments",
                "Duplicate comments",
            ]
            .iter()
            .any(|section| has(section));
    let coderabbit = has("<!-- This is an auto-generated comment: summarize by coderabbit.ai -->")
        || coderabbit_review;
    // Copilot folds the findings it isn't sure of into its overview.
    let copilot = body
        .get(.."## Pull request overview".len())
        .is_some_and(|heading| heading.eq_ignore_ascii_case("## Pull request overview"))
        && !has("Comments suppressed due to low confidence");
    codex || coderabbit || copilot
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;

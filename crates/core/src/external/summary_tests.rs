use super::*;
use crate::github::PullCommentKind;

fn by(login: &str, kind: PullCommentKind, body: &str) -> PullComment {
    PullComment {
        kind,
        id: 1,
        login: login.into(),
        bot: login.ends_with("[bot]"),
        url: "https://github.com/o/app/pull/1".into(),
        body: body.into(),
        lines: None,
        dated: 0,
    }
}

const CODEX: &str = "chatgpt-codex-connector[bot]";
const CODERABBIT: &str = "coderabbitai[bot]";
const COPILOT: &str = "copilot-pull-request-reviewer[bot]";

const CODERABBIT_REVIEW: &str = "**Actionable comments posted: 1**\n\n---\n\n\
    <details>\n<summary>ℹ️ Review info</summary>\n</details>\n\n\
    <!-- This is an auto-generated comment by CodeRabbit for review status -->\n";

#[test]
fn codex_summaries_are_summaries() {
    let status = "<!-- codex-pull-request-review-summary -->\n\
                  <!-- codex-review-state:v1 {} -->\n\n## Codex Review Summary\n";
    let receipt = "<!-- codex-review-receipt:v1 -->\n\n\n### 💡 Codex Review\n\n\
                   Here are some automated review suggestions for this pull request.\n";
    let unmarked = "\n### 💡 Codex Review\n\nHere are some automated review suggestions.\n";
    assert!(summary(&by(CODEX, PullCommentKind::IssueComment, status)));
    assert!(summary(&by(CODEX, PullCommentKind::Review, receipt)));
    assert!(summary(&by(CODEX, PullCommentKind::Review, unmarked)));
}

#[test]
fn coderabbits_walkthrough_and_bare_review_are_summaries() {
    let walkthrough = "<!-- This is an auto-generated comment: summarize by coderabbit.ai -->\n\
                       <!-- walkthrough_start -->\n## Walkthrough\n";
    assert!(summary(&by(
        CODERABBIT,
        PullCommentKind::IssueComment,
        walkthrough
    )));
    assert!(summary(&by(
        CODERABBIT,
        PullCommentKind::Review,
        CODERABBIT_REVIEW
    )));
}

#[test]
fn a_coderabbit_review_carrying_findings_is_not() {
    let outside = format!(
        "> **⚠️ Outside diff range comments (1)**\n> \
         <!-- cr-comment:v1:2229f065e4b3d100f9cdb8fd -->\n{CODERABBIT_REVIEW}"
    );
    let nitpicks = format!("<summary>🧹 Nitpick comments (2)</summary>\n{CODERABBIT_REVIEW}");
    assert!(!summary(&by(CODERABBIT, PullCommentKind::Review, &outside)));
    assert!(!summary(&by(
        CODERABBIT,
        PullCommentKind::Review,
        &nitpicks
    )));
}

#[test]
fn coderabbits_findings_and_replies_are_not() {
    let finding = "**🎯 Functional Correctness** | **🟡 Minor**\n\nGuard the fetch.\n\n\
                   <!-- This is an auto-generated comment by CodeRabbit -->";
    let reply = "`@someone` Thanks, that settles it.\n\n\
                 <!-- This is an auto-generated reply by CodeRabbit -->";
    assert!(!summary(&by(
        CODERABBIT,
        PullCommentKind::ReviewComment,
        finding
    )));
    assert!(!summary(&by(
        CODERABBIT,
        PullCommentKind::ReviewComment,
        reply
    )));
}

#[test]
fn copilots_overview_is_a_summary() {
    let overview = "## Pull request overview\n\nThis PR refactors staging.\n";
    let older = "## Pull Request Overview\n\n**Key changes:**\n";
    assert!(summary(&by(COPILOT, PullCommentKind::Review, overview)));
    assert!(summary(&by(COPILOT, PullCommentKind::Review, older)));
}

#[test]
fn a_copilot_overview_carrying_findings_is_not() {
    let overview = "## Pull request overview\n\nThis PR refactors staging.\n\n\
                    <details>\n<summary>Comments suppressed due to low confidence (1)</summary>\n\
                    **Sources/App.swift:12**\n* Guard the fetch.\n</details>\n";
    assert!(!summary(&by(COPILOT, PullCommentKind::Review, overview)));
}

#[test]
fn a_bots_finding_is_not() {
    let finding = "**<sub><sub>![P1 Badge](https://img.shields.io/badge/P1-orange)</sub></sub> \
                   Release decoded images**";
    assert!(!summary(&by(
        CODEX,
        PullCommentKind::ReviewComment,
        finding
    )));
}

#[test]
fn a_person_quoting_a_summary_is_not() {
    let quoted = "## Pull request overview\n\nI disagree with the bot here.";
    assert!(!summary(&by(
        "octocat",
        PullCommentKind::IssueComment,
        quoted
    )));
}

use super::*;

const CODEX: &str = "chatgpt-codex-connector[bot]";
/// As the hub stamps a revision.
const PUSHED: &str = "2026-10-19T10:45:13.420Z";

fn reaction(login: &str, bot: bool, reaction: Reaction) -> PullReaction {
    reacted(login, bot, reaction, "2026-10-19T10:50:07Z")
}

fn reacted(login: &str, bot: bool, reaction: Reaction, at: &str) -> PullReaction {
    PullReaction {
        login: login.into(),
        bot,
        reaction,
        at: at.into(),
    }
}

#[test]
fn eyes_is_reviewing_and_thumbs_up_approved() {
    let reviews = bot_reviews(
        &[
            reaction(CODEX, true, Reaction::ThumbsUp),
            reaction("lint[bot]", true, Reaction::Eyes),
        ],
        PUSHED,
    );
    assert_eq!(
        reviews,
        [
            BotReview {
                login: CODEX.into(),
                name: "Codex".into(),
                state: BotReviewState::Approved,
            },
            BotReview {
                login: "lint[bot]".into(),
                name: "lint".into(),
                state: BotReviewState::Reviewing,
            },
        ]
    );
}

#[test]
fn eyes_outweighs_an_earlier_thumbs_up() {
    for order in [
        [Reaction::ThumbsUp, Reaction::Eyes],
        [Reaction::Eyes, Reaction::ThumbsUp],
    ] {
        let reactions: Vec<_> = order.map(|r| reaction(CODEX, true, r)).into();
        let reviews = bot_reviews(&reactions, PUSHED);
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].state, BotReviewState::Reviewing, "{order:?}");
    }
}

#[test]
fn a_thumbs_up_from_before_the_push_approved_older_commits() {
    // Left on r1, kept while Codex's review of the push found something.
    let stale = reacted(CODEX, true, Reaction::ThumbsUp, "2026-10-19T10:18:20Z");
    assert_eq!(bot_reviews(&[stale], PUSHED), []);

    // Within the push's second: GitHub drops the milliseconds.
    let same = reacted(CODEX, true, Reaction::ThumbsUp, "2026-10-19T10:45:13Z");
    let reviews = bot_reviews(&[same], PUSHED);
    assert_eq!(reviews[0].state, BotReviewState::Approved);
}

#[test]
fn people_and_other_reactions_say_nothing() {
    let reviews = bot_reviews(
        &[
            reaction("operator", false, Reaction::ThumbsUp),
            reaction("teammate", false, Reaction::Eyes),
            reaction(CODEX, true, Reaction::Other),
        ],
        PUSHED,
    );
    assert_eq!(reviews, []);
}

use super::*;
use crate::fake::{rebased, submit_login};
use crate::item::ItemId;
use crate::item::ItemState;
use crate::review::{NewReview, ReviewStore, Verdict};
use crate::triage::{RulesStore, Tier, Triage};

fn review(store: &mut crate::fake::FakeStore, revision: u32) {
    let review = NewReview {
        item: ItemId::parse("app#1").unwrap(),
        revision,
        verdict: Verdict::ChangesRequested,
        summary: String::new(),
        moved: Vec::new(),
        carried_from: None,
    };
    store
        .insert_review(review, ItemState::ChangesRequested)
        .unwrap();
}

#[test]
fn counts_what_changed_since_the_last_review() {
    let (mut store, git) = rebased();
    let item = submit_login(&mut store, &git, 2);
    assert_eq!(changed_since_review(&store, &item).unwrap(), None);
    review(&mut store, 1);
    // Reviewed the newest: nothing new to count.
    assert_eq!(changed_since_review(&store, &item).unwrap(), None);

    let item = submit_login(&mut store, &git, 4);
    // api.rs, db.rs, lib.rs and token.rs; login.rs is unchanged.
    let changed = ChangedSince {
        revision: 1,
        files: 4,
        approved: false,
    };
    assert_eq!(changed_since_review(&store, &item).unwrap(), Some(changed));

    review(&mut store, 2);
    let item = submit_login(&mut store, &git, 5);
    let changed = ChangedSince {
        revision: 2,
        files: 1,
        approved: false,
    };
    assert_eq!(changed_since_review(&store, &item).unwrap(), Some(changed));
}

#[test]
fn leaves_out_trusted_files() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    review(&mut store, 1);
    let item = submit_login(&mut store, &git, 4);
    let trusted = Triage {
        tier: Tier::Trusted,
        by: None,
    };
    store
        .set_triage(&item.id, 2, &[("src/token.rs".into(), trusted)])
        .unwrap();
    let changed = ChangedSince {
        revision: 1,
        files: 3,
        approved: false,
    };
    assert_eq!(changed_since_review(&store, &item).unwrap(), Some(changed));
}

#[test]
fn counts_nothing_without_an_interdiff() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    review(&mut store, 1);
    let item = submit_login(&mut store, &git, 4);
    store.forget_interdiffs();
    assert_eq!(changed_since_review(&store, &item).unwrap(), None);
}

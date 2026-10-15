use super::*;
use crate::fake::{FakeGit, FakeStore, lines, remapped};
use crate::review::{ReviewStore, WriteDraft, write_draft};

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn draft(
    store: &mut FakeStore,
    git: &FakeGit,
    revision: u32,
    path: &str,
    side: Side,
    lines: (u32, u32),
) -> CommentId {
    let request = WriteDraft {
        id: id(),
        revision,
        path: path.into(),
        side,
        start_line: lines.0,
        line: lines.1,
        body: "Why?".into(),
    };
    write_draft(store, git, request).unwrap().id
}

#[test]
fn places_comments_by_how_their_lines_changed() {
    for rebase in [false, true] {
        let (mut store, git) = remapped(rebase);
        let mut on =
            |revision, path, side, lines| draft(&mut store, &git, revision, path, side, lines);
        let moved = on(1, "src/moved.rs", Side::New, (2, 2));
        let base = on(1, "src/moved.rs", Side::Old, (2, 2));
        let changed = on(1, "src/changed.rs", Side::New, (2, 2));
        let gone = on(1, "src/gone.rs", Side::New, (2, 2));
        let reverted = on(1, "src/hidden.rs", Side::New, (2, 2));
        let hidden = on(1, "src/hidden.rs", Side::Old, (2, 2));
        let dropped = on(1, "README.md", Side::New, (1, 1));
        let own = on(2, "src/changed.rs", Side::New, (2, 2));

        let read = git.blobs_read().len();
        let placed = place(&store, &git, &id(), 2, &store.comments(&id()).unwrap()).unwrap();
        let want = BTreeMap::from([
            (
                moved,
                Placement::Moved {
                    start_line: 3,
                    line: 3,
                },
            ),
            (
                base,
                Placement::Moved {
                    start_line: 2,
                    line: 2,
                },
            ),
            (
                changed,
                Placement::Changed {
                    start_line: 2,
                    line: 2,
                },
            ),
            (gone, Placement::Outdated),
            // r2 no longer changes the line, so only its diff since r1 shows
            // it, on the new side.
            (
                reverted,
                Placement::Changed {
                    start_line: 2,
                    line: 2,
                },
            ),
            (hidden, Placement::Outdated),
            (dropped, Placement::Outdated),
        ]);
        assert_eq!(placed, want, "rebase: {rebase}");
        assert!(!placed.contains_key(&own));

        // Revisions on one base diff their files at submit; a moved base
        // leaves moved.rs's two sides to be diffed here, from their blobs.
        let mut read = git.blobs_read().split_off(read);
        read.sort();
        let mut want = if rebase {
            vec![
                lines("a B c d e f g h i j"),
                lines("x a B c d e f g h i j k"),
                lines("a b c d e f g h i j"),
                lines("a b c d e f g h i j k"),
            ]
        } else {
            Vec::new()
        };
        want.sort();
        assert_eq!(read, want, "rebase: {rebase}");
    }
}

#[test]
fn places_nothing_onto_an_earlier_revision() {
    let (mut store, git) = remapped(false);
    draft(&mut store, &git, 2, "src/changed.rs", Side::New, (2, 2));
    let comments = store.comments(&id()).unwrap();
    assert_eq!(
        place(&store, &git, &id(), 1, &comments).unwrap(),
        BTreeMap::new()
    );
}

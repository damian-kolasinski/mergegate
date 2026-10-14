use super::*;
use crate::attachment::{Attach, Media, attach, prune};
use crate::fake::FakeStore;
use crate::item::ItemState;
use crate::settings;

fn attachment(revision: u32, name: &str) -> Attachment {
    Attachment {
        revision,
        name: name.into(),
        media: Media::Png,
        size: 1,
        sha256: name.into(),
    }
}

/// The attachments, then the revision and names the demo shows.
type Case<'a> = (&'a [(u32, &'a str)], Option<(u32, &'a [&'a str])>);

fn names(demo: &Option<Demo>) -> Option<(u32, Vec<&str>)> {
    demo.as_ref().map(|d| {
        (
            d.revision,
            d.attachments.iter().map(|a| a.name.as_str()).collect(),
        )
    })
}

#[test]
fn shows_the_newest_revision_that_has_any() {
    let cases: &[Case] = &[
        (&[], None),
        (&[(1, "a"), (1, "b")], Some((1, &["a", "b"]))),
        (&[(1, "a"), (3, "c"), (3, "d")], Some((3, &["c", "d"]))),
        (&[(3, "c"), (1, "a")], Some((3, &["c"]))),
    ];
    for (attachments, expected) in cases {
        let all = attachments.iter().map(|(r, n)| attachment(*r, n)).collect();
        let expected = expected.map(|(r, n)| (r, n.to_vec()));
        assert_eq!(names(&choose(all)), expected, "{attachments:?}");
    }
}

#[test]
fn prune_follows_the_hub_ttl() {
    let mut store = FakeStore::with_item(1);
    let app = ItemId::parse("app#1").unwrap();
    let request = Attach {
        id: &app,
        revision: None,
        name: "a.png",
        bytes: b"\x89PNG\r\n\x1a\n\0\0\0\r",
    };
    attach(&mut store, request).unwrap();
    store.set_state(&app, ItemState::Abandoned);
    // The default TTL keeps a just-closed item's demo.
    assert_eq!(prune(&mut store).unwrap(), []);
    assert_eq!(
        names(&demo(&store, &app).unwrap()),
        Some((1, vec!["a.png"]))
    );

    settings::set(&mut store, "attachment-ttl", "0s").unwrap();
    assert_eq!(prune(&mut store).unwrap(), std::slice::from_ref(&app));
    assert_eq!(demo(&store, &app).unwrap(), None);
}

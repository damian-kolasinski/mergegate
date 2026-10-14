use super::*;
use crate::fake::FakeStore;
use crate::item::ItemState;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const MP4: &[u8] = b"\0\0\0\x20ftypisom\0\0\x02\0";

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn request<'a>(
    id: &'a ItemId,
    revision: Option<u32>,
    name: &'a str,
    bytes: &'a [u8],
) -> Attach<'a> {
    Attach {
        id,
        revision,
        name,
        bytes,
    }
}

#[test]
fn attaches_to_the_newest_revision_unless_told() {
    let mut store = FakeStore::with_item(2);
    let app = id("app#1");
    let shot = attach(&mut store, request(&app, None, "after.png", PNG)).unwrap();
    assert_eq!(shot.revision, 2);
    assert_eq!(shot.media, Media::Png);
    assert_eq!(shot.size, PNG.len() as u64);
    let clip = attach(&mut store, request(&app, Some(1), "flow.mp4", MP4)).unwrap();
    assert_eq!(clip.revision, 1);
    assert_eq!(clip.media, Media::Mp4);

    let names: Vec<_> = store
        .attachments(&app)
        .unwrap()
        .into_iter()
        .map(|a| (a.revision, a.name))
        .collect();
    assert_eq!(names, [(1, "flow.mp4".into()), (2, "after.png".into())]);
}

#[test]
fn refuses_what_it_cant_keep() {
    let mut store = FakeStore::with_item(1);
    let app = id("app#1");
    let other = id("app#9");
    let mut err = |r: Attach<'_>| attach(&mut store, r).unwrap_err().to_string();
    assert_eq!(err(request(&other, None, "a.png", PNG)), "no item app#9");
    assert_eq!(
        err(request(&app, Some(2), "a.png", PNG)),
        "app#1 has no revision 2"
    );
    assert_eq!(
        err(request(&app, Some(0), "a.png", PNG)),
        "app#1 has no revision 0"
    );
    assert_eq!(
        err(request(&app, None, "../a.png", PNG)),
        "\"../a.png\" is not a file name"
    );
    assert_eq!(
        err(request(&app, None, "logo.svg", b"<svg onload=alert(1)>")),
        "logo.svg is not a PNG, JPEG, GIF or WebP image, or an MP4 or WebM video"
    );
    let big = vec![0; MAX_SIZE as usize + 1];
    assert_eq!(
        err(request(&app, None, "big.mp4", &big)),
        format!("big.mp4 is {} bytes, over the 100 MB limit", big.len())
    );
    assert_eq!(store.attachments(&app).unwrap(), []);
}

#[test]
fn refuses_closed_items() {
    let mut store = FakeStore::with_item(1);
    let app = id("app#1");
    store.set_state(&app, ItemState::Merged);
    assert_eq!(
        attach(&mut store, request(&app, None, "a.png", PNG))
            .unwrap_err()
            .to_string(),
        "app#1 is closed; attach to an open item"
    );
}

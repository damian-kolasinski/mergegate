use std::time::Duration;

use super::*;
use crate::fake::FakeStore;

#[test]
fn parses_and_prints_durations() {
    let cases = [
        ("2d", Some(2 * 86_400), "2d"),
        ("48h", Some(2 * 86_400), "2d"),
        ("90m", Some(90 * 60), "90m"),
        ("120s", Some(120), "2m"),
        ("45s", Some(45), "45s"),
        ("0s", Some(0), "0s"),
        ("0d", Some(0), "0s"),
        ("3650d", Some(3650 * 86_400), "3650d"),
        ("3651d", None, ""),
        ("2", None, ""),
        ("d", None, ""),
        ("-1d", None, ""),
        ("+1d", None, ""),
        ("1.5h", None, ""),
        ("2w", None, ""),
        ("", None, ""),
        ("99999999999999999999d", None, ""),
    ];
    for (value, secs, printed) in cases {
        let ttl = Ttl::parse(value);
        assert_eq!(ttl.map(|t| t.duration().as_secs()), secs, "{value}");
        if let Some(ttl) = ttl {
            assert_eq!(ttl.to_string(), printed, "{value}");
        }
    }
}

#[test]
fn defaults_until_set() {
    let mut store = FakeStore::default();
    let defaults = settings(&store).unwrap();
    assert_eq!(
        defaults.attachment_ttl.duration(),
        Duration::from_secs(2 * 86_400)
    );

    let changed = set(&mut store, "attachment-ttl", "36h").unwrap();
    assert_eq!(changed.attachment_ttl.to_string(), "36h");
    assert_eq!(settings(&store).unwrap(), changed);
    // Stored as it prints, so `mg hub set` shows what it keeps.
    set(&mut store, "attachment-ttl", "1440m").unwrap();
    assert_eq!(
        store.setting("attachment-ttl").unwrap().as_deref(),
        Some("1d")
    );
}

#[test]
fn listens_locally_until_told_the_tailnet() {
    let mut store = FakeStore::default();
    assert_eq!(settings(&store).unwrap().listen, Listen::Local);
    let changed = set(&mut store, "listen", "tailnet").unwrap();
    assert_eq!(changed.listen, Listen::Tailnet);
    assert_eq!(
        set(&mut store, "listen", "local").unwrap().listen,
        Listen::Local
    );
}

#[test]
fn refuses_unknown_keys_and_bad_values() {
    let mut store = FakeStore::default();
    assert_eq!(
        set(&mut store, "port", "7801").unwrap_err().to_string(),
        "there is no hub setting \"port\"; settings: attachment-ttl, listen"
    );
    assert_eq!(
        set(&mut store, "listen", "lan").unwrap_err().to_string(),
        "listen can't be \"lan\"; expected local or tailnet"
    );
    assert_eq!(
        set(&mut store, "attachment-ttl", "soon")
            .unwrap_err()
            .to_string(),
        "attachment-ttl can't be \"soon\"; expected a duration such as 2d, 12h, 30m or 0s"
    );
    assert_eq!(store.setting("attachment-ttl").unwrap(), None);
}

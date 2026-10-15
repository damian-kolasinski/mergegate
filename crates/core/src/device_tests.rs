use std::cell::RefCell;

use super::*;
use crate::fake::FakeStore;

fn user(id: u64) -> Owner {
    Owner::User {
        id,
        login: format!("user{id}@example.com"),
    }
}

fn device(name: &str, owner: Owner, ip: &str) -> Device {
    Device {
        id: DeviceId(format!("n{name}")),
        name: name.into(),
        dns_name: format!("{name}.tail0.ts.net"),
        owner,
        ips: vec![ip.parse().unwrap()],
    }
}

fn hub() -> Device {
    device("hub", user(1), "100.64.0.1")
}

struct FakeTailnet {
    status: Result<Status, TailnetError>,
    asked: RefCell<u32>,
}

impl Tailnet for FakeTailnet {
    fn status(&self) -> Result<Status, TailnetError> {
        *self.asked.borrow_mut() += 1;
        self.status.clone()
    }

    fn whois(&self, ip: IpAddr) -> Result<Device, TailnetError> {
        Err(TailnetError::Unknown(ip))
    }
}

fn tailnet(peers: Vec<Device>) -> FakeTailnet {
    FakeTailnet {
        status: Ok(Status { me: hub(), peers }),
        asked: RefCell::new(0),
    }
}

#[test]
fn trusts_the_owners_devices_and_decisions() {
    let me = hub();
    let mine = device("laptop", user(1), "100.64.0.2");
    let theirs = device("worker-2", user(2), "100.64.0.3");
    let tagged = device("ci", Owner::Tags(vec!["tag:ci".into()]), "100.64.0.4");
    let cases = [
        (&mine, None, Ok(Trust::Owner)),
        (&mine, Some(Access::Deny), Err(Refusal::Denied)),
        (&mine, Some(Access::Allow), Ok(Trust::Allowed)),
        (&theirs, None, Err(Refusal::NotAllowed)),
        (&theirs, Some(Access::Allow), Ok(Trust::Allowed)),
        (&theirs, Some(Access::Deny), Err(Refusal::Denied)),
        (&tagged, None, Err(Refusal::NotAllowed)),
        (&tagged, Some(Access::Allow), Ok(Trust::Allowed)),
        (&me, None, Ok(Trust::Owner)),
    ];
    for (caller, access, expected) in cases {
        assert_eq!(
            trust(&me, caller, access),
            expected,
            "{} {access:?}",
            caller.name
        );
    }
}

#[test]
fn a_hub_owned_by_tags_trusts_no_device_on_its_own() {
    let me = device("hub", Owner::Tags(vec!["tag:hub".into()]), "100.64.0.1");
    let other = device("ci", Owner::Tags(vec!["tag:hub".into()]), "100.64.0.4");
    assert_eq!(trust(&me, &other, None), Err(Refusal::NotAllowed));
}

#[test]
fn remembers_who_was_refused() {
    let mut store = FakeStore::default();
    let theirs = device("worker-2", user(2), "100.64.0.3");
    let mine = device("laptop", user(1), "100.64.0.2");

    assert_eq!(
        admit(&mut store, &hub(), &mine, 10).unwrap(),
        Ok(Trust::Owner)
    );
    assert_eq!(store.devices().unwrap(), []);

    assert_eq!(
        admit(&mut store, &hub(), &theirs, 20).unwrap(),
        Err(Refusal::NotAllowed)
    );
    assert_eq!(
        store.device(&theirs.id).unwrap(),
        Some(Record {
            id: theirs.id.clone(),
            name: "worker-2".into(),
            owner: "user2@example.com".into(),
            access: None,
            asked_at: Some(20),
        })
    );
}

#[test]
fn allow_and_deny_find_a_device_by_any_of_its_names() {
    let worker = device("worker-2", user(2), "100.64.0.3");
    let net = tailnet(vec![worker.clone()]);
    let mut store = FakeStore::default();
    let refused = admit(&mut store, &hub(), &worker, 5).unwrap();
    assert_eq!(refused, Err(Refusal::NotAllowed));

    for name in [
        "worker-2",
        "WORKER-2",
        "worker-2.tail0.ts.net.",
        "100.64.0.3",
    ] {
        let record = decide(&mut store, &net, name, Access::Allow).unwrap();
        assert_eq!(record.id, worker.id, "{name}");
    }
    let record = decide(&mut store, &net, "worker-2", Access::Deny).unwrap();
    assert_eq!(record.access, Some(Access::Deny));
    // The refusal stays on record, so the listing knows it once asked.
    assert_eq!(record.asked_at, Some(5));
    assert_eq!(
        admit(&mut store, &hub(), &worker, 6).unwrap(),
        Err(Refusal::Denied)
    );

    let err = decide(&mut store, &net, "worker-3", Access::Allow).unwrap_err();
    assert_eq!(
        err.to_string(),
        "no device named \"worker-3\" on the tailnet"
    );
}

#[test]
fn decisions_need_tailscale() {
    let net = FakeTailnet {
        status: Err(TailnetError::Unavailable("Tailscale is stopped".into())),
        asked: RefCell::new(0),
    };
    let err = decide(&mut FakeStore::default(), &net, "x", Access::Allow).unwrap_err();
    assert_eq!(err.to_string(), "Tailscale is stopped");
    assert_eq!(*net.asked.borrow(), 1);
}

#[test]
fn lists_the_tailnet_and_the_devices_that_left_it() {
    let laptop = device("laptop", user(1), "100.64.0.2");
    let asked = device("worker-2", user(2), "100.64.0.3");
    let allowed = device("worker-3", user(2), "100.64.0.5");
    let quiet = device("phone", user(3), "100.64.0.6");
    let gone = Record {
        id: DeviceId("ngone".into()),
        name: "old-box".into(),
        owner: "user2@example.com".into(),
        access: Some(Access::Allow),
        asked_at: None,
    };
    let net = tailnet(vec![laptop, asked.clone(), allowed.clone(), quiet]);
    let mut store = FakeStore::default();
    let refused = admit(&mut store, &hub(), &asked, 1).unwrap();
    assert_eq!(refused, Err(Refusal::NotAllowed));
    decide(&mut store, &net, "worker-3", Access::Allow).unwrap();
    store.put_device(&gone).unwrap();

    let listed = list(&net.status().unwrap(), &store.devices().unwrap());
    let summary: Vec<_> = listed
        .iter()
        .map(|l| (l.name.as_str(), l.standing, l.on_tailnet))
        .collect();
    assert_eq!(
        summary,
        [
            ("hub", Standing::Owner, true),
            ("laptop", Standing::Owner, true),
            ("old-box", Standing::Allowed, false),
            ("phone", Standing::NotAllowed, true),
            ("worker-2", Standing::Asked, true),
            ("worker-3", Standing::Allowed, true),
        ]
    );
}

use super::*;
use mergegate_core::device::{DeviceId, Owner};
use std::sync::mpsc;
use std::thread;

fn ip() -> IpAddr {
    "100.64.0.1".parse().unwrap()
}

fn device(name: &str) -> Device {
    Device {
        id: DeviceId(name.into()),
        name: name.into(),
        dns_name: format!("{name}.tail0.ts.net"),
        owner: Owner::User {
            id: 1,
            login: "owner@example.com".into(),
        },
        ips: vec![ip()],
    }
}

// Hold the external lookup until every follower has joined its flight,
// rather than rely on sleeps to make the concurrent requests overlap.
fn wait_for_followers(cache: &Whois, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let entries = cache.entries.lock().unwrap();
        if let Some(Entry::Running(flight)) = entries.get(&ip())
            && Arc::strong_count(flight) == count + 2
        {
            return;
        }
        drop(entries);
        assert!(
            Instant::now() < deadline,
            "followers did not join the lookup"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn concurrent_requests_share_success_and_failure_then_retry() {
    for success in [true, false] {
        let cache = Whois::default();
        let (started, ready) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        thread::scope(|scope| {
            let cache = &cache;
            let leader = scope.spawn(move || {
                cache.get(ip(), Duration::from_secs(60), || {
                    started.send(()).unwrap();
                    blocked.recv_timeout(Duration::from_secs(15)).unwrap();
                    if success {
                        Ok(device("worker"))
                    } else {
                        Err(TailnetError::Unavailable("temporarily offline".into()))
                    }
                })
            });
            ready.recv().unwrap();
            let followers: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        cache.get(ip(), Duration::from_secs(60), || {
                            panic!("a follower started another lookup")
                        })
                    })
                })
                .collect();
            wait_for_followers(cache, followers.len());
            // A stalled address does not hold up any other address.
            assert_eq!(
                cache
                    .get(
                        "100.64.0.2".parse().unwrap(),
                        Duration::from_secs(60),
                        || { Ok(device("other")) }
                    )
                    .unwrap(),
                device("other")
            );
            release.send(()).unwrap();
            let answer = leader.join().unwrap();
            for follower in followers {
                assert_eq!(
                    follower.join().unwrap().map_err(|e| e.to_string()),
                    answer.clone().map_err(|e| e.to_string())
                );
            }
        });
        if success {
            assert_eq!(
                cache
                    .get(ip(), Duration::from_secs(60), || panic!("not cached"))
                    .unwrap(),
                device("worker")
            );
        } else {
            assert_eq!(
                cache
                    .get(ip(), Duration::from_secs(60), || Ok(device("recovered")))
                    .unwrap(),
                device("recovered")
            );
        }
    }
}

#[test]
fn expired_identity_is_never_used_when_refresh_fails() {
    let cache = Whois::default();
    cache.entries.lock().unwrap().insert(
        ip(),
        Entry::Ready(Instant::now() - Duration::from_secs(61), device("old")),
    );
    assert!(matches!(
        cache.get(ip(), Duration::from_secs(60), || Err(
            TailnetError::Unknown(ip())
        )),
        Err(TailnetError::Unknown(_))
    ));
    assert_eq!(
        cache
            .get(ip(), Duration::from_secs(60), || Ok(device("new")))
            .unwrap(),
        device("new")
    );
}

#[test]
fn zero_ttl_always_refreshes() {
    let cache = Whois::default();
    for name in ["first", "second"] {
        assert_eq!(
            cache
                .get(ip(), Duration::ZERO, || Ok(device(name)))
                .unwrap(),
            device(name)
        );
    }
}

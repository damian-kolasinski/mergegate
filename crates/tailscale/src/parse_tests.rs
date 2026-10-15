use super::*;

const STATUS: &str = r#"{
  "Version": "1.102.3",
  "BackendState": "Running",
  "Self": {
    "ID": "nSELF",
    "HostName": "Mac mini",
    "DNSName": "mac-mini.tail0.ts.net.",
    "UserID": 1,
    "TailscaleIPs": ["100.93.225.125", "fd7a:115c:a1e0::3229:e17e"],
    "Online": true
  },
  "Peer": {
    "nodekey:b": {
      "ID": "nCI",
      "HostName": "ci",
      "DNSName": "ci-runner.tail0.ts.net.",
      "UserID": 9,
      "TailscaleIPs": ["100.64.0.4"],
      "Tags": ["tag:ci"]
    },
    "nodekey:a": {
      "ID": "nLAPTOP",
      "HostName": "MacBook Pro",
      "DNSName": "macbook-pro.tail0.ts.net.",
      "UserID": 2,
      "TailscaleIPs": ["100.71.11.120"],
      "Tags": null
    }
  },
  "User": {
    "1": {"ID": 1, "LoginName": "me@example.com"},
    "2": {"ID": 2, "LoginName": "them@example.com"}
  }
}"#;

#[test]
fn reads_the_hub_machine_and_its_peers() {
    let status = status(STATUS.as_bytes()).unwrap();
    assert_eq!(
        status.me,
        Device {
            id: DeviceId("nSELF".into()),
            name: "mac-mini".into(),
            dns_name: "mac-mini.tail0.ts.net".into(),
            owner: Owner::User {
                id: 1,
                login: "me@example.com".into()
            },
            ips: vec![
                "100.93.225.125".parse().unwrap(),
                "fd7a:115c:a1e0::3229:e17e".parse().unwrap()
            ],
        }
    );
    let peers: Vec<_> = status
        .peers
        .iter()
        .map(|d| (d.id.0.as_str(), d.name.as_str(), d.owner.describe()))
        .collect();
    assert_eq!(
        peers,
        [
            ("nCI", "ci-runner", "tag:ci".to_owned()),
            ("nLAPTOP", "macbook-pro", "them@example.com".to_owned()),
        ]
    );
}

#[test]
fn a_tailnet_with_no_peers_is_still_a_tailnet() {
    let json = r#"{"BackendState": "Running", "Self": {"ID": "n1", "DNSName": "box.ts.net.", "UserID": 1}, "Peer": null}"#;
    let status = status(json.as_bytes()).unwrap();
    assert_eq!(status.me.name, "box");
    assert_eq!(status.peers, []);
    assert_eq!(status.me.owner.describe(), "1");
}

#[test]
fn a_stopped_or_logged_out_tailscale_is_unavailable() {
    for state in ["Stopped", "NeedsLogin"] {
        let json = format!(r#"{{"BackendState": "{state}", "Self": null}}"#);
        let err = status(json.as_bytes()).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("Tailscale isn't connected ({state}); start it and log in")
        );
    }
    let err = status(b"not json").unwrap_err();
    assert!(err.to_string().starts_with("can't read tailscale's answer"));
}

#[test]
fn reads_who_an_address_belongs_to() {
    let json = r#"{
      "Node": {
        "ID": 8499719159005948,
        "StableID": "nLAPTOP",
        "Name": "macbook-pro.tail0.ts.net.",
        "Addresses": ["100.71.11.120/32", "fd7a:115c:a1e0::501:bcd/128"],
        "Tags": null,
        "User": 2
      },
      "UserProfile": {"ID": 2, "LoginName": "them@example.com", "DisplayName": "Them"}
    }"#;
    let device = whois(json.as_bytes()).unwrap();
    assert_eq!(
        device,
        Device {
            id: DeviceId("nLAPTOP".into()),
            name: "macbook-pro".into(),
            dns_name: "macbook-pro.tail0.ts.net".into(),
            owner: Owner::User {
                id: 2,
                login: "them@example.com".into()
            },
            ips: vec![
                "100.71.11.120".parse().unwrap(),
                "fd7a:115c:a1e0::501:bcd".parse().unwrap()
            ],
        }
    );

    let tagged = json.replace(r#""Tags": null"#, r#""Tags": ["tag:ci"]"#);
    assert_eq!(
        whois(tagged.as_bytes()).unwrap().owner,
        Owner::Tags(vec!["tag:ci".into()])
    );
}

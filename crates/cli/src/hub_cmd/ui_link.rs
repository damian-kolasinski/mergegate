//! Where the operator opens the review UI: at the hub's tailnet name once it
//! listens there, so a phone or another machine opens the same link, else on
//! this machine.

use std::io::IsTerminal;

use api::settings::Settings;

use super::probe::{Running, agent};
use super::qr;

pub(super) struct UiLink {
    pub url: String,
    /// Whether other devices on the tailnet can open it.
    tailnet: bool,
}

impl UiLink {
    /// `localhost` rather than the hub's `127.0.0.1`: it reads as a place,
    /// and the hub answers to both.
    fn local(port: u16) -> Self {
        Self {
            url: format!("http://localhost:{port}/#/"),
            tailnet: false,
        }
    }

    /// `url` as the hub's settings report its tailnet listener.
    pub fn tailnet(url: &str) -> Self {
        Self {
            url: format!("{}/#/", url.trim_end_matches('/')),
            tailnet: true,
        }
    }

    /// The link, and in a terminal a QR code of it for a phone on the
    /// tailnet. Agents read `mg` through pipes, where it would be noise.
    pub fn text(&self) -> String {
        self.render(std::io::stdout().is_terminal())
    }

    fn render(&self, terminal: bool) -> String {
        let mut text = format!("open the review UI: {}", self.url);
        if terminal && self.tailnet {
            text.push_str("\nor scan it with a phone on the tailnet:\n");
            text.push_str(&qr::render(&self.url));
        }
        text
    }
}

/// The running hub's link: the tailnet's while it listens there.
pub(super) fn of(running: &Running) -> UiLink {
    let settings: Option<Settings> = agent()
        .get(format!("{}/api/hub/settings", running.url()))
        .call()
        .ok()
        .and_then(|mut response| response.body_mut().read_json().ok());
    settings.and_then(|s| s.tailnet?.url).map_or_else(
        || UiLink::local(running.port()),
        |url| UiLink::tailnet(&url),
    )
}

#[cfg(test)]
#[path = "ui_link_tests.rs"]
mod tests;

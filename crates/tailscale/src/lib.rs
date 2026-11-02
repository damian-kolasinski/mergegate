//! The operator's tailnet, through the `tailscale` CLI: who the hub's
//! machine is, who else is on the tailnet, and who an address belongs to.

mod cache;
mod parse;
mod run;

use std::net::IpAddr;
use std::time::Duration;

use mergegate_core::device::{Device, Status, Tailnet, TailnetError};

pub struct TailscaleCli {
    timeout: Duration,
    whois_ttl: Duration,
    whois: cache::Whois,
}

impl TailscaleCli {
    /// Caches who an address belongs to for `whois_ttl`, since the hub asks on
    /// every request from the tailnet and the CLI takes tens of milliseconds.
    /// A CLI that hasn't answered in `timeout` is killed.
    pub fn new(whois_ttl: Duration, timeout: Duration) -> Self {
        Self {
            timeout,
            whois_ttl,
            whois: cache::Whois::default(),
        }
    }

    /// A minute's cache and 5 seconds to answer, unless
    /// `MG_TAILSCALE_CACHE_MS` and `MG_TAILSCALE_TIMEOUT_MS` say otherwise:
    /// tests set the cache to 0 to change who an address is between
    /// requests, and the timeout high for a machine busy running them.
    pub fn from_env() -> Self {
        let ms = |var: &str, default| {
            std::env::var(var)
                .ok()
                .and_then(|ms| ms.parse().ok())
                .map_or(default, Duration::from_millis)
        };
        Self::new(
            ms("MG_TAILSCALE_CACHE_MS", Duration::from_secs(60)),
            ms("MG_TAILSCALE_TIMEOUT_MS", Duration::from_secs(5)),
        )
    }
}

impl Tailnet for TailscaleCli {
    fn status(&self) -> Result<Status, TailnetError> {
        parse::status(&run::tailscale(&["status", "--json"], self.timeout)?)
    }

    fn whois(&self, ip: IpAddr) -> Result<Device, TailnetError> {
        self.whois.get(ip, self.whois_ttl, || {
            match run::tailscale_retried(&["whois", "--json", &ip.to_string()], self.timeout) {
                Ok(json) => parse::whois(&json),
                Err(TailnetError::Unavailable(why)) if why.contains("peer not found") => {
                    Err(TailnetError::Unknown(ip))
                }
                Err(e) => Err(e),
            }
        })
    }
}

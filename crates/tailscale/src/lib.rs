//! The operator's tailnet, through the `tailscale` CLI: who the hub's
//! machine is, who else is on the tailnet, and who an address belongs to.

mod parse;
mod run;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mergegate_core::device::{Device, Status, Tailnet, TailnetError};

pub struct TailscaleCli {
    timeout: Duration,
    whois_ttl: Duration,
    whois: Mutex<HashMap<IpAddr, (Instant, Device)>>,
}

impl TailscaleCli {
    /// Caches who an address belongs to for `whois_ttl`, since the hub asks on
    /// every request from the tailnet and the CLI takes tens of milliseconds.
    /// A CLI that hasn't answered in `timeout` is killed.
    pub fn new(whois_ttl: Duration, timeout: Duration) -> Self {
        Self {
            timeout,
            whois_ttl,
            whois: Mutex::default(),
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
        let cache = || self.whois.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, device)) = cache().get(&ip)
            && at.elapsed() < self.whois_ttl
        {
            return Ok(device.clone());
        }
        // Unlocked while the CLI runs, so one slow answer doesn't hold every
        // other caller's request. Failures aren't cached: a device that just
        // joined, or Tailscale that just came up, is let in on the next
        // request.
        let device = match run::tailscale(&["whois", "--json", &ip.to_string()], self.timeout) {
            Ok(json) => parse::whois(&json)?,
            Err(TailnetError::Unavailable(why)) if why.contains("peer not found") => {
                return Err(TailnetError::Unknown(ip));
            }
            Err(e) => return Err(e),
        };
        if !self.whois_ttl.is_zero() {
            let mut cache = cache();
            cache.retain(|_, (at, _)| at.elapsed() < self.whois_ttl);
            cache.insert(ip, (Instant::now(), device.clone()));
        }
        Ok(device)
    }
}

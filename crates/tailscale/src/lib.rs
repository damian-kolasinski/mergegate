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
    whois_ttl: Duration,
    whois: Mutex<HashMap<IpAddr, (Instant, Device)>>,
}

impl TailscaleCli {
    /// Caches who an address belongs to for `whois_ttl`, since the hub asks on
    /// every request from the tailnet and the CLI takes tens of milliseconds.
    pub fn new(whois_ttl: Duration) -> Self {
        Self {
            whois_ttl,
            whois: Mutex::default(),
        }
    }

    /// A minute's cache, unless `MG_TAILSCALE_CACHE_MS` says otherwise, as
    /// tests set it to 0 to change who an address is between requests.
    pub fn from_env() -> Self {
        let ms = std::env::var("MG_TAILSCALE_CACHE_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(60_000);
        Self::new(Duration::from_millis(ms))
    }
}

impl Tailnet for TailscaleCli {
    fn status(&self) -> Result<Status, TailnetError> {
        parse::status(&run::tailscale(&["status", "--json"])?)
    }

    fn whois(&self, ip: IpAddr) -> Result<Device, TailnetError> {
        let cache = || self.whois.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, device)) = cache().get(&ip)
            && at.elapsed() < self.whois_ttl
        {
            return Ok(device.clone());
        }
        // Unlocked while the CLI runs, so one slow answer doesn't hold every
        // other caller's request. Failures aren't cached: a device that just joined, or Tailscale
        // that just came up, is let in on the next request.
        let device = match run::tailscale(&["whois", "--json", &ip.to_string()]) {
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

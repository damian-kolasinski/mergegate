//! One active identity lookup per address. Only successful answers outlive
//! their callers; an outage must not delay recovery or extend identity trust.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use mergegate_core::device::{Device, TailnetError};

type Answer = Result<Device, TailnetError>;

#[derive(Default)]
pub(crate) struct Whois {
    entries: Mutex<HashMap<IpAddr, Entry>>,
}

enum Entry {
    Ready(Instant, Device),
    Running(Arc<Flight>),
}

#[derive(Default)]
struct Flight {
    answer: Mutex<Option<Answer>>,
    finished: Condvar,
}

impl Whois {
    pub fn get(&self, ip: IpAddr, ttl: Duration, lookup: impl FnOnce() -> Answer) -> Answer {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        match entries.get(&ip) {
            Some(Entry::Ready(at, device)) if at.elapsed() < ttl => return Ok(device.clone()),
            Some(Entry::Running(flight)) => {
                let flight = Arc::clone(flight);
                drop(entries);
                let answer = flight.answer.lock().unwrap_or_else(|e| e.into_inner());
                return flight
                    .finished
                    .wait_while(answer, |answer| answer.is_none())
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                    .expect("the lookup finished")
                    .clone();
            }
            _ => {}
        }
        let flight = Arc::new(Flight::default());
        entries.retain(|_, entry| match entry {
            Entry::Ready(at, _) => at.elapsed() < ttl,
            Entry::Running(_) => true,
        });
        entries.insert(ip, Entry::Running(Arc::clone(&flight)));
        drop(entries);

        // Never hold the map while calling Tailscale: another device can
        // still authenticate while this lookup stalls.
        let answer = lookup();
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        match &answer {
            Ok(device) if !ttl.is_zero() => {
                entries.insert(ip, Entry::Ready(Instant::now(), device.clone()));
            }
            _ => {
                entries.remove(&ip);
            }
        }
        *flight.answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(answer.clone());
        flight.finished.notify_all();
        answer
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;

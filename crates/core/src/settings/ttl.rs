use std::fmt;
use std::time::Duration;

/// A whole number of seconds, minutes, hours or days, as in `2d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ttl(Duration);

/// Ten years: past it, `now - ttl` stops being a date SQLite can compute.
const MAX_SECS: u64 = 10 * 365 * 86_400;

impl Ttl {
    pub const DEFAULT_ATTACHMENTS: Self = Self(Duration::from_secs(2 * 86_400));

    pub fn parse(value: &str) -> Option<Self> {
        let unit = value.chars().last()?;
        let scale = match unit {
            's' => 1,
            'm' => 60,
            'h' => 3_600,
            'd' => 86_400,
            _ => return None,
        };
        let number = &value[..value.len() - 1];
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let secs = number.parse::<u64>().ok()?.checked_mul(scale)?;
        (secs <= MAX_SECS).then_some(Self(Duration::from_secs(secs)))
    }

    pub fn duration(self) -> Duration {
        self.0
    }
}

/// In the largest unit that divides it, so `48h` prints as `2d`.
impl fmt::Display for Ttl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();
        let (count, unit) = [(86_400, 'd'), (3_600, 'h'), (60, 'm')]
            .into_iter()
            .find(|(scale, _)| secs > 0 && secs.is_multiple_of(*scale))
            .map_or((secs, 's'), |(scale, unit)| (secs / scale, unit));
        write!(f, "{count}{unit}")
    }
}

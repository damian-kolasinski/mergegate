use std::fmt;
use std::time::Duration;

/// A whole number of seconds, minutes, hours or days, as in `2d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ttl(Duration);

/// Ten years: past it, `now - ttl` stops being a date SQLite can compute.
const MAX_SECS: u64 = 10 * 365 * 86_400;

/// Why a value isn't a [`Ttl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtlError {
    /// Not a whole number followed by s, m, h or d.
    Malformed,
    /// Well-formed, but longer than [`Ttl::MAX`].
    TooLong,
}

impl Ttl {
    pub const DEFAULT_ATTACHMENTS: Self = Self(Duration::from_secs(2 * 86_400));
    pub const MAX: Self = Self(Duration::from_secs(MAX_SECS));

    pub fn parse(value: &str) -> Result<Self, TtlError> {
        let unit = value.chars().last().ok_or(TtlError::Malformed)?;
        let scale = match unit {
            's' => 1,
            'm' => 60,
            'h' => 3_600,
            'd' => 86_400,
            _ => return Err(TtlError::Malformed),
        };
        let number = &value[..value.len() - 1];
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return Err(TtlError::Malformed);
        }
        // Only digits are left, so a number that doesn't parse overflows.
        let secs = number
            .parse::<u64>()
            .ok()
            .and_then(|n| n.checked_mul(scale))
            .filter(|secs| *secs <= MAX_SECS)
            .ok_or(TtlError::TooLong)?;
        Ok(Self(Duration::from_secs(secs)))
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

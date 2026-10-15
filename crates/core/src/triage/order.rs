//! The order of the files to review. By default: what the judge scores
//! highest, highest first, then the guide's list, then the judge's scores,
//! the heuristics and the path. A project can let the judge's scores order
//! them all instead.

use std::cmp::Reverse;

use super::Guide;
use super::heuristic::Hint;

/// A judge score from this up puts a file ahead of the guide's list, which
/// the authoring agent wrote about its own work.
pub const RAISE_AT: u8 = 80;

/// What a file the judge skipped counts as: an average one, so it neither
/// sinks below every scored file nor rises above them.
const UNSCORED: u8 = 50;

/// What leads the order of the files to review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderBy {
    /// The guide's list, with the files the judge scores [`RAISE_AT`] or more
    /// ahead of it.
    #[default]
    Guide,
    /// The judge's scores alone; the guide only breaks their ties.
    Judge,
}

impl OrderBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Guide => "guide",
            Self::Judge => "judge",
        }
    }

    pub fn parse(by: &str) -> Option<Self> {
        [Self::Guide, Self::Judge]
            .into_iter()
            .find(|b| b.as_str() == by)
    }
}

/// A file to review, as the order sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rank<'a> {
    pub path: &'a str,
    /// The judge's score, 1 to 100; none when it didn't judge the file.
    pub importance: Option<u8>,
    pub hint: Option<Hint>,
}

/// Where a file goes: `index` into the files given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub index: usize,
    /// The judge's score put it ahead of a file the guide, or the order
    /// without that score, put first. Never set when the judge orders every
    /// file.
    pub raised: bool,
}

/// `files` in review order.
pub fn order(files: &[Rank], guide: &Guide, by: OrderBy) -> Vec<Placed> {
    let position = |r: &Rank<'_>| guide.position(r.path).unwrap_or(usize::MAX);
    let score = |r: &Rank<'_>| Reverse(r.importance.unwrap_or(UNSCORED));
    let hint = |r: &Rank<'_>| Reverse(r.hint.map_or(0, Hint::weight));
    let mut before: Vec<usize> = (0..files.len()).collect();
    before.sort_by(|&a, &b| {
        let (a, b) = (&files[a], &files[b]);
        let (listed, scored) = (position(a).cmp(&position(b)), score(a).cmp(&score(b)));
        match by {
            OrderBy::Guide => listed.then(scored),
            OrderBy::Judge => scored.then(listed),
        }
        .then_with(|| hint(a).cmp(&hint(b)))
        .then_with(|| a.path.cmp(b.path))
    });
    if by == OrderBy::Judge {
        return before
            .into_iter()
            .map(|index| Placed {
                index,
                raised: false,
            })
            .collect();
    }
    // The raised highest first; stable, so ties and the rest keep the usual
    // order.
    let raise = |r: &Rank<'_>| r.importance.filter(|&i| i >= RAISE_AT);
    let mut after = before.clone();
    after.sort_by_key(|&i| {
        let raised = raise(&files[i]);
        (raised.is_none(), Reverse(raised))
    });
    let mut was = vec![0; files.len()];
    for (at, &index) in before.iter().enumerate() {
        was[index] = at;
    }
    // Raised past a file the usual order put ahead of it, which is still to
    // come. Comparing places instead would miss a file raised behind
    // another, and only the raised move ahead of anything.
    let mut placed = vec![false; files.len()];
    let mut first_unplaced = 0;
    after
        .into_iter()
        .map(|index| {
            while placed[before[first_unplaced]] {
                first_unplaced += 1;
            }
            placed[index] = true;
            Placed {
                index,
                raised: first_unplaced < was[index],
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "order_tests.rs"]
mod tests;

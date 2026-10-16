//! `mg prioritize`: the operator's agent ranks an open item in the review
//! queue, as the operator does with `p` in the UI.

use api::item::{Item, Prioritize, Priority};
use clap::ValueEnum;

use crate::client::{self, Client};
use crate::item_name;
use crate::output::{CliError, Outcome};

#[derive(Clone, Copy, ValueEnum)]
pub enum Level {
    /// Review it first
    P0,
    P1,
    P2,
    P3,
    /// Sort it after p3
    None,
}

impl Level {
    pub(crate) fn to_api(self) -> Option<Priority> {
        match self {
            Self::P0 => Some(Priority::P0),
            Self::P1 => Some(Priority::P1),
            Self::P2 => Some(Priority::P2),
            Self::P3 => Some(Priority::P3),
            Self::None => None,
        }
    }
}

pub fn run(item: String, priority: Level, note: Option<String>) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let request = Prioritize {
        priority: priority.to_api(),
        note,
    };
    let item = item_name::find(&client, &item)?;
    let path = format!("/api/items/{}/priority", client::segment(&item.id));
    let item = match client.post::<Item>(&path, &request)? {
        client::Reply::Done(item) => item,
        client::Reply::Decide(decision) => return Ok(Outcome::NeedsDecision(decision)),
    };
    let mut text = match item.priority {
        Some(priority) => format!("{} is {}", item.name, name(priority)),
        None => format!("{} has no priority", item.name),
    };
    if let Some(note) = &item.note {
        text.push_str(&format!(": {note}"));
    }
    Ok(Outcome::done(&item, text))
}

pub(crate) fn name(priority: Priority) -> &'static str {
    match priority {
        Priority::P0 => "p0",
        Priority::P1 => "p1",
        Priority::P2 => "p2",
        Priority::P3 => "p3",
    }
}

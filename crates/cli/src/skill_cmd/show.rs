//! `mg skill show`: the guide the skill stub sends agents to, from this
//! `mg`, so it always describes the commands this version has.

use serde::Serialize;

use crate::output::{CliError, Outcome};

#[derive(Serialize)]
struct Shown {
    /// The reference shown; `None` for the guide.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    text: String,
}

pub(super) fn run(name: Option<String>) -> Result<Outcome, CliError> {
    let text = skill::show(name.as_deref()).ok_or_else(|| {
        let names: Vec<_> = skill::references().collect();
        CliError::new(
            "unknown_reference",
            format!(
                "the guide has no reference {:?}; it has {}",
                name.as_deref().unwrap_or_default(),
                names.join(", ")
            ),
        )
        .hint(format!("mg skill show {}", names[0]))
    })?;
    let printed = text.trim_end().to_owned();
    Ok(Outcome::done(&Shown { name, text }, printed))
}

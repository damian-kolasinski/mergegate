//! `mg rules list`.

use api::rules::{Rules, Tier};
use clap::Subcommand;

use crate::client::Client;
use crate::output::{CliError, Outcome};
use crate::project_cmd::{project_here, route};

#[derive(Subcommand)]
pub enum RulesCommand {
    /// The rules that sort a project's files into review and trusted, first
    /// match wins [default: the project at the current directory]
    List { project: Option<String> },
}

pub fn run(command: RulesCommand) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    match command {
        RulesCommand::List { project } => {
            let project = match project {
                Some(project) => project,
                None => project_here(&client)?.name,
            };
            let rules: Rules = client.get(&format!("{}/rules", route(&project)?))?;
            let text = describe(&rules);
            Ok(Outcome::done(&rules, text))
        }
    }
}

fn describe(rules: &Rules) -> String {
    let mut out = Vec::new();
    if let Some(error) = &rules.error {
        out.push(format!(
            "{} can't be used, so the rules before it still apply: {error}",
            rules.location
        ));
    }
    if rules.rules.is_empty() {
        out.push(format!(
            "{} has no rules, so every file is reviewed; the operator writes them in {}",
            rules.project, rules.location
        ));
        return out.join("\n");
    }
    out.push(format!("{} ({})", rules.project, rules.location));
    for rule in &rules.rules {
        let tier = match rule.tier {
            Tier::Review => "review ",
            Tier::Trusted => "trusted",
        };
        out.push(format!(
            "  {}  {tier}  {} — {}",
            rule.id, rule.summary, rule.reason
        ));
    }
    out.join("\n")
}

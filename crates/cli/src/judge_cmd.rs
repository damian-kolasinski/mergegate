//! `mg judge set | show`.

use api::judge::{Judge, SetJudge};
use clap::{Subcommand, ValueEnum};

use crate::client::Client;
use crate::output::{CliError, Outcome};
use crate::project_cmd::{named, route};

#[derive(Subcommand)]
pub enum JudgeCommand {
    /// Pick the model that scores how much each file needs review, only when
    /// the operator asks: Jev, a command of theirs, or none
    Set {
        #[arg(value_enum)]
        provider: ProviderArg,
        /// Jev's model [default: jev-1.13.0]
        #[arg(long)]
        model: Option<String>,
        /// For the command judge: run by `sh -c` in the project's repo, it
        /// reads the change as JSON on stdin and prints the scores as JSON
        #[arg(long, value_name = "CMD")]
        command: Option<String>,
        /// The confidence, above 0 and at most 1, from which the judge pulls
        /// a trusted file back into review [default: 0.8]
        #[arg(long)]
        threshold: Option<f64>,
        /// What leads the order of the files to review: the guide's list,
        /// with the files the judge scores 80 or more ahead of it, or the
        /// judge's scores alone, the guide only breaking ties [default: guide]
        #[arg(long, value_enum)]
        order: Option<OrderArg>,
        /// [default: the project at the current directory]
        #[arg(long)]
        project: Option<String>,
    },
    /// The project's judge, and what it hasn't judged yet [default: the
    /// project at the current directory]
    Show { project: Option<String> },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ProviderArg {
    None,
    Jev,
    Command,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OrderArg {
    Guide,
    Judge,
}

pub fn run(command: JudgeCommand) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let judge: Judge = match command {
        JudgeCommand::Set {
            provider,
            model,
            command,
            threshold,
            order,
            project,
        } => {
            let project = named(&client, project)?;
            let request = SetJudge {
                provider: match provider {
                    ProviderArg::None => "none",
                    ProviderArg::Jev => "jev",
                    ProviderArg::Command => "command",
                }
                .into(),
                model,
                command,
                threshold,
                order: order.map(|order| {
                    match order {
                        OrderArg::Guide => "guide",
                        OrderArg::Judge => "judge",
                    }
                    .into()
                }),
            };
            client.put(&format!("{}/judge", route(&project)?), &request)?
        }
        JudgeCommand::Show { project } => {
            let project = named(&client, project)?;
            client.get(&format!("{}/judge", route(&project)?))?
        }
    };
    let text = describe(&judge);
    Ok(Outcome::done(&judge, text))
}

fn describe(judge: &Judge) -> String {
    let project = &judge.project;
    let by = match (judge.provider.as_str(), &judge.model, &judge.command) {
        ("jev", Some(model), _) => format!("Jev ({model})"),
        ("command", _, Some(command)) => format!("`{command}`"),
        _ => {
            return format!(
                "{project} has no judge: files rank by the guide and their paths and diffs"
            );
        }
    };
    let mut out = vec![format!("{project} is judged by {by}")];
    if let Some(threshold) = judge.threshold {
        out.push(format!(
            "  a trusted file goes back to review at {}% confidence",
            (threshold * 100.0).round()
        ));
    }
    match judge.order.as_deref() {
        Some("judge") => {
            out.push("  its scores order the files to review; the guide breaks ties".into())
        }
        Some(_) => out
            .push("  files to review follow the guide, after the ones it scores 80 or more".into()),
        None => {}
    }
    if let Some(problem) = &judge.problem {
        out.push(format!("  it can't run: {problem}"));
    }
    if judge.waiting > 0 {
        out.push(format!(
            "  {} open item{} not judged yet",
            judge.waiting,
            if judge.waiting == 1 { "" } else { "s" }
        ));
    }
    for failure in &judge.failures {
        out.push(format!(
            "  failed on {} revision {}: {}",
            failure.name, failure.revision, failure.error
        ));
    }
    if !judge.failures.is_empty() {
        out.push("  setting the judge again retries them".into());
    }
    out.join("\n")
}

//! `mg rules list | propose | show`.

use api::rules::{Kind, Preview, Proposal, ProposalState, ProposeRule, Rule, Rules, Tier};
use clap::{Subcommand, ValueEnum};

use crate::client::{Client, Reply, segment};
use crate::output::{CliError, Outcome};
use crate::project_cmd::{named, route};

#[derive(Subcommand)]
pub enum RulesCommand {
    /// The rules that sort a project's files into review and trusted, first
    /// match wins, and the changes waiting for the operator [default: the
    /// project at the current directory]
    List { project: Option<String> },
    /// Propose a rule, only when the operator asks for one: a review rule
    /// applies at once, a trusted one waits for their approval in the UI
    Propose {
        #[arg(long, value_enum)]
        tier: TierArg,
        /// Why, as the operator put it; shown beside each file it sorts
        #[arg(long)]
        reason: String,
        /// Glob over the file's path, as in `**/*.lock`
        #[arg(long)]
        path: Option<String>,
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
        /// Matches files with at most this many changed lines
        #[arg(long)]
        max_lines: Option<u64>,
        /// Regex over the changed lines: a trusted rule needs every one to
        /// match, a review rule any
        #[arg(long)]
        diff_matches: Option<String>,
        /// [default: the project at the current directory]
        #[arg(long)]
        project: Option<String>,
    },
    /// A proposed rule and whether the operator approved it
    Show {
        /// As in R4
        id: String,
        /// [default: the project at the current directory]
        project: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum TierArg {
    Review,
    Trusted,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum KindArg {
    Added,
    Deleted,
    Renamed,
    Modified,
    Generated,
}

pub fn run(command: RulesCommand) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    match command {
        RulesCommand::List { project } => {
            let project = named(&client, project)?;
            let rules: Rules = client.get(&format!("{}/rules", route(&project)?))?;
            let text = describe(&rules);
            Ok(Outcome::done(&rules, text))
        }
        RulesCommand::Propose {
            tier,
            reason,
            path,
            kind,
            max_lines,
            diff_matches,
            project,
        } => {
            let project = named(&client, project)?;
            let request = ProposeRule {
                tier: match tier {
                    TierArg::Review => Tier::Review,
                    TierArg::Trusted => Tier::Trusted,
                },
                path,
                kind: kind.map(|kind| match kind {
                    KindArg::Added => Kind::Added,
                    KindArg::Deleted => Kind::Deleted,
                    KindArg::Renamed => Kind::Renamed,
                    KindArg::Modified => Kind::Modified,
                    KindArg::Generated => Kind::Generated,
                }),
                max_lines,
                diff_matches,
                reason,
            };
            let path = format!("{}/rules/proposals", route(&project)?);
            match client.post::<Proposal>(&path, &request)? {
                Reply::Done(proposal) => {
                    let text = proposed(&proposal);
                    Ok(Outcome::done(&proposal, text))
                }
                Reply::Decide(decision) => Ok(Outcome::NeedsDecision(decision)),
            }
        }
        RulesCommand::Show { id, project } => {
            let project = named(&client, project)?;
            let path = format!("{}/rules/proposals/{}", route(&project)?, segment(&id));
            let proposal: Proposal = client.get(&path)?;
            let text = show(&proposal);
            Ok(Outcome::done(&proposal, text))
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
    } else {
        out.push(format!("{} ({})", rules.project, rules.location));
        out.extend(rules.rules.iter().map(line));
    }
    if !rules.pending.is_empty() || rules.edit.is_some() {
        out.push("Waiting for the operator's approval in the UI:".into());
    }
    for proposal in &rules.pending {
        out.push(line(&proposal.rule));
        if let Some(preview) = &proposal.preview {
            out.push(format!("      {}", would_hide(preview)));
        }
    }
    if let Some(edit) = &rules.edit {
        out.push(format!(
            "  an edit to {}: {}",
            rules.location,
            would_hide(&edit.preview)
        ));
    }
    out.join("\n")
}

fn proposed(proposal: &Proposal) -> String {
    let id = &proposal.rule.id;
    match &proposal.preview {
        Some(preview) if proposal.state == ProposalState::Pending => format!(
            "{id} waits for the operator's approval in the UI: {}",
            would_hide(preview)
        ),
        _ => format!("{id} applies now\n{}", line(&proposal.rule)),
    }
}

fn show(proposal: &Proposal) -> String {
    let state = match proposal.state {
        ProposalState::Pending => "waits for the operator's approval in the UI",
        ProposalState::Applied => "was approved by the operator",
        ProposalState::Rejected => "was rejected by the operator",
    };
    let mut out = vec![
        format!("{} {state}", proposal.rule.id),
        line(&proposal.rule),
    ];
    if let Some(preview) = &proposal.preview {
        out.push(format!("When proposed, {}", would_hide(preview)));
        for file in &preview.hidden {
            out.push(format!("  {}  {}  {}", file.name, file.path, file.title));
        }
    }
    out.join("\n")
}

fn line(rule: &Rule) -> String {
    let tier = match rule.tier {
        Tier::Review => "review ",
        Tier::Trusted => "trusted",
    };
    format!("  {}  {tier}  {} — {}", rule.id, rule.summary, rule.reason)
}

/// As in "it would have hidden 3 files in 2 of 14 items".
fn would_hide(preview: &Preview) -> String {
    let mut items: Vec<_> = preview.hidden.iter().map(|f| &f.item).collect();
    items.sort();
    items.dedup();
    let files = preview.hidden.len();
    format!(
        "it would have hidden {files} {} in {} of {} {}",
        if files == 1 { "file" } else { "files" },
        items.len(),
        preview.items,
        if preview.items == 1 { "item" } else { "items" },
    )
}

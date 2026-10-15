//! `mg`: the mergegate CLI, and the hub it starts. Composition root of the CLI.

mod abandon_cmd;
mod attach_cmd;
mod client;
mod connect_cmd;
mod doctor_cmd;
mod events_cmd;
mod feedback_cmd;
mod hub_cmd;
mod inbox_cmd;
mod init_cmd;
mod judge_cmd;
mod lang_cmd;
pub mod output;
mod prioritize_cmd;
mod project_cmd;
mod project_here;
mod reply_cmd;
mod repo;
mod resume_cmd;
mod rules_cmd;
mod skill_cmd;
mod submit_cmd;
mod sync_cmd;
mod wait_cmd;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use hub::Home;

use output::{CliError, EXIT_CODES_HELP, Outcome, exit};

/// Set to `1` to get `--json` output from every command.
pub const JSON_ENV: &str = "MG_JSON";

#[derive(Parser)]
#[command(
    name = "mg",
    version,
    about = "mergegate: local code review hub for coding agents",
    after_help = EXIT_CODES_HELP,
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// Print one JSON object on stdout (also: MG_JSON=1)
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run and inspect the hub
    #[command(subcommand)]
    Hub(hub_cmd::HubCommand),
    /// Work through the hub on another machine of the tailnet, which runs
    /// with listen tailnet; --local goes back to this machine's hub
    Connect {
        /// The hub's machine, as in mac-mini, mac-mini:7801 or its URL
        #[arg(required_unless_present = "local", conflicts_with = "local")]
        hub: Option<String>,
        #[arg(long)]
        local: bool,
    },
    /// Register repos as projects and change their settings
    #[command(subcommand)]
    Project(project_cmd::ProjectCommand),
    /// Install and remove the language packs that highlight diffs
    #[command(subcommand)]
    Lang(lang_cmd::LangCommand),
    /// The rules that sort a project's files into review and trusted
    #[command(subcommand)]
    Rules(rules_cmd::RulesCommand),
    /// The model that ranks a project's files by how much they need review
    #[command(subcommand)]
    Judge(judge_cmd::JudgeCommand),
    /// In a clone: add the `mergegate` remote, pointing at its project
    Init {
        /// Project to push to [default: the one registered for this repo]
        #[arg(long)]
        project: Option<String>,
    },
    /// Push the current branch to the hub and submit it for review; again
    /// from the same branch, add a revision
    Submit {
        /// Item title [default: the first commit's subject]
        #[arg(long)]
        title: Option<String>,
        /// Shell command the hub runs in this worktree when the item has
        /// events from the operator you haven't read; `{item}` becomes its
        /// ID, "" removes it; only on the hub's machine
        #[arg(long, value_name = "CMD")]
        wake: Option<String>,
        /// Screenshot or recording of the change for the operator: PNG,
        /// JPEG, GIF, WebP, MP4 or WebM, up to 100 MB; repeat for more
        #[arg(long, value_name = "FILE")]
        attach: Vec<PathBuf>,
        /// Markdown for the operator, read from FILE or `-` for stdin: what
        /// the change is for, and a `## Files` list of `` - `path` — reason ``
        /// bullets in the order to review them [default: the last one]
        #[arg(long, value_name = "FILE")]
        guide: Option<PathBuf>,
        /// How urgent the item is, from your brief; only the submit that
        /// opens the item sets it, mg prioritize changes it after
        #[arg(long, value_parser = submit_cmd::initial_priority())]
        priority: Option<api::item::Priority>,
        /// Why it has that priority, for the operator; as --priority
        #[arg(long)]
        note: Option<String>,
    },
    /// Add screenshots or recordings to the item's newest revision
    Attach {
        /// The item, as in app#42
        item: String,
        /// PNG, JPEG, GIF, WebP, MP4 or WebM, up to 100 MB each
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Wait until the item has feedback for its agent, print it and mark it
    /// delivered
    Wait {
        /// The item, as in app#42
        item: String,
        /// Which events end the wait
        #[arg(long = "for", value_enum, default_value = "any")]
        wake: wait_cmd::Wake,
        /// Seconds to wait before exiting with 75; 0 waits until there is
        /// feedback
        #[arg(long, default_value_t = 540)]
        timeout: u64,
        /// Replay the events after this cursor, even delivered ones, without
        /// marking them delivered
        #[arg(long)]
        since: Option<u64>,
    },
    /// Print the item's new feedback and mark it delivered, without waiting
    Feedback {
        /// The item, as in app#42
        item: String,
        /// Replay the events after this cursor, even delivered ones, without
        /// marking them delivered
        #[arg(long)]
        since: Option<u64>,
    },
    /// Answer a comment from the operator's review or from GitHub
    Reply {
        /// The comment, as in c_17
        comment: String,
        #[arg(long, value_enum)]
        status: reply_cmd::Status,
        /// Why, for the operator; required unless the status is addressed
        #[arg(long)]
        body: Option<String>,
    },
    /// Read the item's pull request comments on GitHub now and get those the
    /// hub holds at once; the hub polls on its own, so only when it missed
    /// some
    Sync {
        /// The item, as in app#42
        item: String,
    },
    /// Set how urgent an open item is, which orders the operator's inbox
    Prioritize {
        /// The item, as in app#42
        item: String,
        #[arg(long, value_enum)]
        priority: prioritize_cmd::Level,
        /// Why, for the operator; leaving it out clears the old one
        #[arg(long)]
        note: Option<String>,
    },
    /// Every item's events, hub-wide, without marking any delivered
    Events {
        /// Start after this cursor [default: the newest, so only what comes
        /// next]
        #[arg(long)]
        since: Option<u64>,
        /// Keep printing events as they come, one per line (one JSON object
        /// with --json), until interrupted
        #[arg(long)]
        follow: bool,
    },
    /// Close an item without merging it; its pinned revisions and diffs go
    Abandon {
        /// The item, as in app#42
        item: String,
        /// Why, for the operator
        #[arg(long)]
        reason: Option<String>,
    },
    /// Print a prompt that brings a new agent session up to date on an item,
    /// without marking anything delivered
    Resume {
        /// The item, as in app#42
        item: String,
    },
    /// Open items: those waiting for review, with their agents, merging
    Inbox,
    /// Check that the hub, git and every project are set up to work
    Doctor,
    /// The agent skill matching this mg
    #[command(subcommand)]
    Skill(skill_cmd::SkillCommand),
}

pub fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let json_env = std::env::var_os(JSON_ENV).is_some_and(|v| v == "1");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            let json = json_env || args.iter().any(|a| a == "--json");
            return usage(e, json, &args);
        }
    };
    let json = cli.json || json_env;
    output::emit(json, dispatch(cli.command, json))
}

/// `json` is for commands that print as they go; the rest leave it to
/// [`output::emit`].
fn dispatch(command: Command, json: bool) -> Result<Outcome, CliError> {
    match command {
        Command::Hub(command) => hub_cmd::run(command),
        Command::Connect { hub, local } => connect_cmd::run(hub, local),
        Command::Project(command) => project_cmd::run(command),
        Command::Lang(command) => lang_cmd::run(command),
        Command::Rules(command) => rules_cmd::run(command),
        Command::Judge(command) => judge_cmd::run(command),
        Command::Init { project } => init_cmd::run(project),
        Command::Submit {
            title,
            wake,
            attach,
            guide,
            priority,
            note,
        } => submit_cmd::run(title, wake, attach, guide, priority, note),
        Command::Attach { item, files } => attach_cmd::run(item, files),
        Command::Wait {
            item,
            wake,
            timeout,
            since,
        } => wait_cmd::run(item, wake, timeout, since),
        Command::Feedback { item, since } => feedback_cmd::run(item, since),
        Command::Reply {
            comment,
            status,
            body,
        } => reply_cmd::run(comment, status, body),
        Command::Sync { item } => sync_cmd::run(item),
        Command::Prioritize {
            item,
            priority,
            note,
        } => prioritize_cmd::run(item, priority, note),
        Command::Events { since, follow } => events_cmd::run(since, follow, json),
        Command::Abandon { item, reason } => abandon_cmd::run(item, reason),
        Command::Resume { item } => resume_cmd::run(item),
        Command::Inbox => inbox_cmd::run(),
        Command::Doctor => doctor_cmd::run(),
        Command::Skill(command) => skill_cmd::run(command),
    }
}

fn home() -> Result<Home, CliError> {
    Home::from_env().map_err(|e| {
        CliError::new("invalid_home", e.to_string())
            .hint(format!("set {} to an absolute path", hub::HOME_ENV))
    })
}

/// clap exits with 2 on usage errors, which the contract reserves for
/// `needs_decision`; usage errors exit with 64 instead.
fn usage(e: clap::Error, json: bool, args: &[OsString]) -> ExitCode {
    match e.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = e.print();
            ExitCode::from(exit::OK)
        }
        _ if json => {
            let message = match e.kind() {
                // clap renders help here, not an error line.
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                    "a subcommand is required".to_owned()
                }
                _ => error_line(&e.render().to_string()),
            };
            let err = CliError::new("usage", message)
                .hint(format!("{} --help", command_path(args)))
                .exit(exit::USAGE);
            output::emit(true, Err(err))
        }
        _ => {
            let _ = e.print();
            ExitCode::from(exit::USAGE)
        }
    }
}

/// The error clap rendered, on one line. Some errors continue on indented
/// lines before the first blank one, such as the missing arguments after
/// "the following required arguments were not provided:"; tips and usage
/// come after the blank line.
fn error_line(rendered: &str) -> String {
    let mut lines = rendered.lines();
    let mut line = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches("error: ")
        .to_owned();
    let more: Vec<&str> = lines
        .take_while(|l| !l.trim().is_empty())
        .map(str::trim)
        .collect();
    if !more.is_empty() {
        line.push(' ');
        line.push_str(&more.join(", "));
    }
    line
}

/// `mg` and the subcommands `args` name, so the hint points at the help of
/// the command that was misused.
fn command_path(args: &[OsString]) -> String {
    let mut path = String::from("mg");
    let mut command = Cli::command();
    for arg in args.iter().skip(1) {
        let Some(arg) = arg.to_str() else { break };
        if arg.starts_with('-') {
            continue;
        }
        let Some(sub) = command.find_subcommand(arg).cloned() else {
            break;
        };
        path.push(' ');
        path.push_str(sub.get_name());
        command = sub;
    }
    path
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

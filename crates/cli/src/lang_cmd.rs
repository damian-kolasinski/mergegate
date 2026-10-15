//! `mg lang list | add | update | remove`.

use std::path::PathBuf;

use api::lang::{AddPacks, Pack, Packs, Released};
use clap::Subcommand;

use crate::client::{Client, Reply, segment};
use crate::output::{CliError, Outcome};
use crate::project_cmd::local_path;
use crate::project_here::project_here;

#[derive(Subcommand)]
pub enum LangCommand {
    /// List the installed language packs and the files each highlights
    List {
        /// List the packs released with mg instead, which mg lang add
        /// installs by name
        #[arg(long)]
        available: bool,
    },
    /// Install language packs released with mg by name, the ones a
    /// project's files use, or packs from files
    #[command(group(
        clap::ArgGroup::new("source").required(true).args(["names", "from", "detect"])
    ))]
    Add {
        /// Packs from mg lang list --available
        names: Vec<String>,
        /// A .tar.gz, or a folder of them, instead of mg's release
        #[arg(long, value_name = "PATH")]
        from: Option<PathBuf>,
        /// The packs the project's files use that aren't installed
        /// [default: the project at the current directory]
        #[arg(long, value_name = "PROJECT", num_args = 0..=1, default_missing_value = "")]
        detect: Option<String>,
    },
    /// Download again the installed packs built for another mg
    Update,
    /// Remove an installed language pack
    Remove { name: String },
}

pub fn run(command: LangCommand) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    match command {
        LangCommand::List { available: false } => {
            let list: Packs = client.get("/api/languages")?;
            let text = if list.packs.is_empty() {
                "no language packs; install them with mg lang add --detect".to_owned()
            } else {
                table(&list.packs)
            };
            Ok(Outcome::done(&list, text))
        }
        LangCommand::List { available: true } => {
            let list: Released = client.get("/api/released-languages")?;
            let text = if list.packs.is_empty() {
                "this mg was built without its release's language packs; \
                 install them with mg lang add --from PATH"
                    .to_owned()
            } else {
                released(&list)
            };
            Ok(Outcome::done(&list, text))
        }
        LangCommand::Add {
            names,
            from,
            detect,
        } => {
            let request = match (from, detect) {
                (Some(from), _) => AddPacks {
                    from: Some(local_path(&client, &from)?),
                    ..AddPacks::default()
                },
                (None, Some(project)) => {
                    let project = if project.is_empty() {
                        project_here(&client)?.name
                    } else {
                        project
                    };
                    AddPacks {
                        detect: Some(project),
                        ..AddPacks::default()
                    }
                }
                (None, None) => AddPacks {
                    names,
                    ..AddPacks::default()
                },
            };
            add(&client, &request, "every pack its files use is installed")
        }
        LangCommand::Update => {
            let request = AddPacks {
                update: true,
                ..AddPacks::default()
            };
            add(&client, &request, "every pack is up to date")
        }
        LangCommand::Remove { name } => {
            // An empty name would address the list instead of a pack.
            if name.is_empty() {
                return Err(
                    CliError::new("pack_not_installed", "the pack name is empty")
                        .hint("mg lang list"),
                );
            }
            let removed: Pack = client.delete(&format!("/api/languages/{}", segment(&name)))?;
            let text = format!("removed {} {}", removed.name, removed.version);
            Ok(Outcome::done(&removed, text))
        }
    }
}

fn add(client: &Client, request: &AddPacks, nothing: &str) -> Result<Outcome, CliError> {
    match client.post::<Packs>("/api/languages", request)? {
        Reply::Done(added) if added.packs.is_empty() => Ok(Outcome::done(&added, nothing)),
        Reply::Done(added) => {
            let text = format!("installed\n{}", table(&added.packs));
            Ok(Outcome::done(&added, text))
        }
        Reply::Decide(decision) => Ok(Outcome::NeedsDecision(decision)),
    }
}

/// [`table`], with whether each pack is installed.
fn released(list: &Released) -> String {
    let packs: Vec<Pack> = list.packs.iter().map(|p| p.pack.clone()).collect();
    table(&packs)
        .lines()
        .zip(&list.packs)
        .map(|(row, p)| match (p.installed, p.outdated) {
            (true, true) => format!("{row}  (outdated: mg lang update)"),
            (true, false) => format!("{row}  (installed)"),
            (false, _) => row.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One row per pack: its name, version and the files its grammars claim.
fn table(packs: &[Pack]) -> String {
    let name = packs.iter().map(|p| p.name.len()).max().unwrap_or(0);
    let version = packs.iter().map(|p| p.version.len()).max().unwrap_or(0);
    packs
        .iter()
        .map(|p| format!("{:name$}  {:version$}  {}", p.name, p.version, files(p)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn files(pack: &Pack) -> String {
    pack.grammars
        .iter()
        .flat_map(|g| {
            let extensions = g.extensions.iter().map(|e| format!(".{e}"));
            extensions.chain(g.filenames.iter().cloned())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

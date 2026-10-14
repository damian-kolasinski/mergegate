//! `mg lang list | add | remove`.

use std::path::PathBuf;

use api::lang::{AddPacks, Pack, Packs};
use clap::Subcommand;

use crate::client::{Client, Reply, segment};
use crate::output::{CliError, Outcome};
use crate::project_cmd::absolute;

#[derive(Subcommand)]
pub enum LangCommand {
    /// List the installed language packs and the files each highlights
    List,
    /// Install language packs from a .tar.gz, or every .tar.gz in a folder
    Add {
        #[arg(long, value_name = "PATH")]
        from: PathBuf,
    },
    /// Remove an installed language pack
    Remove { name: String },
}

pub fn run(command: LangCommand) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    match command {
        LangCommand::List => {
            let list: Packs = client.get("/api/languages")?;
            let text = if list.packs.is_empty() {
                "no language packs; install them with mg lang add --from PATH".to_owned()
            } else {
                table(&list.packs)
            };
            Ok(Outcome::done(&list, text))
        }
        LangCommand::Add { from } => {
            let request = AddPacks {
                from: absolute(&from)?,
            };
            match client.post::<Packs>("/api/languages", &request)? {
                Reply::Done(added) => {
                    let text = format!("installed\n{}", table(&added.packs));
                    Ok(Outcome::done(&added, text))
                }
                Reply::Decide(decision) => Ok(Outcome::NeedsDecision(decision)),
            }
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

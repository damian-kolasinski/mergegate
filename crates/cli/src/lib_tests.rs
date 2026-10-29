use std::collections::BTreeSet;

use clap::CommandFactory;

use super::*;

/// One `mg ...` the skill names in code.
struct Invocation {
    words: Vec<String>,
    /// In inline code, where it can name a command group in prose, rather
    /// than on a line of a fenced block, which runs.
    inline: bool,
    /// The text it came from.
    source: String,
}

fn invocations() -> Vec<Invocation> {
    let mut found = Vec::new();
    for (file, text) in skill::FILES {
        for (code, inline) in code_spans(text) {
            // Shell lines can hold more than one command.
            for part in code.split(['|', ';', '&']) {
                let words: Vec<&str> = part.split_whitespace().collect();
                for (at, word) in words.iter().enumerate() {
                    // A bare `mg` names the binary, not a command.
                    if *word == "mg" && at + 1 < words.len() {
                        let rest = words[at + 1..].iter().take_while(|w| **w != "mg");
                        found.push(Invocation {
                            words: rest.map(|w| (*w).to_owned()).collect(),
                            inline,
                            source: format!("{file}: {code}"),
                        });
                    }
                }
            }
        }
    }
    found
}

/// Inline code and the lines of fenced blocks, each with whether it is
/// inline. A `text` block holds what `mg` prints, not commands.
fn code_spans(text: &str) -> Vec<(String, bool)> {
    let mut spans = Vec::new();
    let mut fenced = None;
    for line in text.lines() {
        if let Some(info) = line.strip_prefix("```") {
            fenced = match fenced {
                None => Some(info.trim() == "text"),
                Some(_) => None,
            };
        } else if let Some(output) = fenced {
            if !output {
                spans.push((line.to_owned(), false));
            }
        } else {
            let inline = line.split('`').skip(1).step_by(2);
            spans.extend(inline.map(|code| (code.to_owned(), true)));
        }
    }
    spans
}

/// The subcommand the words name, and the words after it.
fn resolve(words: &[String]) -> (clap::Command, Vec<String>, &[String]) {
    let mut command = Cli::command();
    let mut path = Vec::new();
    let mut rest = words;
    while let Some((word, tail)) = rest.split_first() {
        let Some(sub) = command.find_subcommand(word).cloned() else {
            break;
        };
        path.push(word.clone());
        command = sub;
        rest = tail;
    }
    (command, path, rest)
}

/// The words outside quoted arguments, whose flags, as in
/// `--wake "claude --resume ID"`, are another command's.
fn unquoted(words: &[String]) -> Vec<&String> {
    let mut outside = Vec::new();
    let mut quote = None;
    for word in words {
        match quote {
            None => match word.chars().next().filter(|c| matches!(c, '"' | '\'')) {
                // A word that opens and closes its quote is one argument.
                Some(q) if !(word.len() > 1 && word.ends_with(q)) => quote = Some(q),
                Some(_) => {}
                None => outside.push(word),
            },
            Some(q) if word.ends_with(q) => quote = None,
            Some(_) => {}
        }
    }
    outside
}

/// Whether the words after `command` make a command `mg` runs. Inline code
/// that ends at a group, as in "the `mg wakes` commands", names the group.
fn runs(command: &clap::Command, rest: &[String], inline: bool) -> bool {
    !command.has_subcommands() || (inline && rest.is_empty())
}

#[test]
fn a_group_named_in_prose_is_no_invocation() {
    let words = |text: &str| {
        text.split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let group = words("wakes");
    let (command, _, rest) = resolve(&group);
    assert!(runs(&command, rest, true));
    assert!(!runs(&command, rest, false), "a fenced line runs");
    let flagged = words("wakes --json");
    let (command, _, rest) = resolve(&flagged);
    assert!(!runs(&command, rest, true));
}

#[test]
fn flags_in_a_quoted_argument_are_not_mgs() {
    let words: Vec<String> = r#"--wake "claude -p --resume ID 'Run it.'" --title "x" --on 3"#
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let outside: Vec<&str> = unquoted(&words).into_iter().map(String::as_str).collect();
    assert_eq!(outside, ["--wake", "--title", "--on", "3"]);
}

#[test]
fn every_command_and_flag_the_skill_names_exists() {
    let global: Vec<String> = Cli::command()
        .get_arguments()
        .filter_map(|a| a.get_long().map(str::to_owned))
        .collect();
    let invocations = invocations();
    assert!(!invocations.is_empty());
    for Invocation {
        words,
        inline,
        source,
    } in invocations
    {
        let (command, path, rest) = resolve(&words);
        assert!(!path.is_empty(), "no such command in {source}");
        assert!(
            runs(&command, rest, inline),
            "`mg {}` needs a subcommand in {source}",
            path.join(" ")
        );
        for word in unquoted(rest) {
            let Some(flag) = word.strip_prefix("--") else {
                continue;
            };
            let flag = flag.split('=').next().unwrap();
            let known = global.iter().any(|g| g == flag)
                || command.get_arguments().any(|a| a.get_long() == Some(flag));
            assert!(
                known,
                "`mg {}` has no --{flag}, in {source}",
                path.join(" ")
            );
        }
    }
}

#[test]
fn the_skill_names_every_command() {
    let named: BTreeSet<String> = invocations()
        .into_iter()
        .map(|i| resolve(&i.words).1.join(" "))
        .collect();
    let mut missing = Vec::new();
    let mut stack = vec![(Cli::command(), String::new())];
    while let Some((command, path)) = stack.pop() {
        // A hidden command is a hook's, not an agent's.
        for sub in command.get_subcommands().filter(|sub| !sub.is_hide_set()) {
            let sub_path = format!("{path} {}", sub.get_name()).trim().to_owned();
            if sub.has_subcommands() {
                stack.push((sub.clone(), sub_path));
            } else if !named.contains(&sub_path) {
                missing.push(sub_path);
            }
        }
    }
    assert_eq!(missing, [] as [String; 0], "commands the skill never names");
}

use std::collections::BTreeSet;

use clap::CommandFactory;

use super::*;

/// Every `mg ...` the skill names in code, split into words, with the text
/// it came from.
fn invocations() -> Vec<(Vec<String>, String)> {
    let mut found = Vec::new();
    for (file, text) in skill::FILES {
        for code in code_spans(text) {
            // Shell lines can hold more than one command.
            for part in code.split(['|', ';', '&']) {
                let words: Vec<&str> = part.split_whitespace().collect();
                for (at, word) in words.iter().enumerate() {
                    // A bare `mg` names the binary, not a command.
                    if *word == "mg" && at + 1 < words.len() {
                        let rest = words[at + 1..].iter().take_while(|w| **w != "mg");
                        found.push((
                            rest.map(|w| (*w).to_owned()).collect(),
                            format!("{file}: {code}"),
                        ));
                    }
                }
            }
        }
    }
    found
}

/// Inline code and the lines of fenced blocks.
fn code_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if fenced {
            spans.push(line.to_owned());
        } else {
            spans.extend(line.split('`').skip(1).step_by(2).map(str::to_owned));
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

#[test]
fn every_command_and_flag_the_skill_names_exists() {
    let global: Vec<String> = Cli::command()
        .get_arguments()
        .filter_map(|a| a.get_long().map(str::to_owned))
        .collect();
    let invocations = invocations();
    assert!(!invocations.is_empty());
    for (words, source) in invocations {
        let (command, path, rest) = resolve(&words);
        assert!(!path.is_empty(), "no such command in {source}");
        assert!(
            !command.has_subcommands(),
            "`mg {}` needs a subcommand in {source}",
            path.join(" ")
        );
        for word in rest {
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
        .map(|(words, _)| resolve(&words).1.join(" "))
        .collect();
    let mut missing = Vec::new();
    let mut stack = vec![(Cli::command(), String::new())];
    while let Some((command, path)) = stack.pop() {
        for sub in command.get_subcommands() {
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

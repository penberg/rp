use crate::agent;
use crate::cmd::check;
use crate::config::test_integration_config;
use crate::issues::active_issue_dir;
use std::collections::BTreeSet;
use std::process::Command;
use std::process::ExitCode;

pub fn run() -> ExitCode {
    let issue_dir = match active_issue_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let issue_name = issue_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    println!("fix: issue {issue_name}");
    println!("fix: issue directory {}", issue_dir.display());

    let test_integration = match test_integration_config() {
        Ok(config) => config,
        Err(err) => {
            eprintln!(
                "error: failed to read {}: {err}",
                crate::config::CONFIG_PATH
            );
            return ExitCode::FAILURE;
        }
    };
    let baseline_test_changes = match changed_test_paths(&test_integration.roots) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("error: failed to inspect repo test changes: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("fix: invoking agent");

    let summary = match agent::fix(std::path::Path::new("."), &issue_dir) {
        Ok(summary) => summary,
        Err(err) => {
            eprintln!("error: fix failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    if !summary.trim().is_empty() {
        println!("fix: agent summary");
        println!("{summary}");
    }

    if !test_integration.roots.is_empty() {
        let current_test_changes = match changed_test_paths(&test_integration.roots) {
            Ok(paths) => paths,
            Err(err) => {
                eprintln!("error: failed to inspect repo test changes: {err}");
                return ExitCode::FAILURE;
            }
        };

        let new_test_changes = current_test_changes
            .difference(&baseline_test_changes)
            .cloned()
            .collect::<Vec<_>>();

        if new_test_changes.is_empty() {
            eprintln!(
                "fix: no repo-native regression test was added under configured roots: {}",
                test_integration.roots.join(", ")
            );
            return ExitCode::FAILURE;
        }

        println!("fix: regression test changes");
        for path in new_test_changes {
            println!("fix:   {path}");
        }
    }

    println!("fix: checking reproducer");
    let check = match check::run_check(&issue_dir) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("error: post-fix check failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    match check.verdict.as_str() {
        "not_reproduced" => {
            println!("fix: issue appears fixed");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("fix: issue still not fixed ({other})");
            ExitCode::FAILURE
        }
    }
}

fn changed_test_paths(roots: &[String]) -> Result<BTreeSet<String>, String> {
    if roots.is_empty() {
        return Ok(BTreeSet::new());
    }

    let mut paths = BTreeSet::new();

    let diff_output = git_paths(&["diff", "--name-only", "--"], roots)?;
    collect_lines(&mut paths, &diff_output);

    let untracked_output = git_paths(&["ls-files", "--others", "--exclude-standard", "--"], roots)?;
    collect_lines(&mut paths, &untracked_output);

    Ok(paths)
}

fn git_paths(prefix: &[&str], roots: &[String]) -> Result<String, String> {
    let mut command = Command::new("git");
    for arg in prefix {
        command.arg(arg);
    }
    for root in roots {
        command.arg(root);
    }

    let output = command
        .output()
        .map_err(|err| format!("failed to execute git: {err}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn collect_lines(paths: &mut BTreeSet<String>, output: &str) {
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        paths.insert(line.to_string());
    }
}

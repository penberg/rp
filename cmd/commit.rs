use crate::agent;
use crate::issues::active_issue_dir;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

pub fn run() -> ExitCode {
    let diff = match git_diff_against_head() {
        Ok(diff) => diff,
        Err(err) => {
            eprintln!("error: failed to inspect git diff: {err}");
            return ExitCode::FAILURE;
        }
    };

    if diff.trim().is_empty() {
        eprintln!("error: no tracked changes to commit");
        return ExitCode::FAILURE;
    }

    let issue_context = load_issue_context();

    println!("commit: generating commit message");
    let message = match agent::commit_message(Path::new("."), issue_context.as_ref(), &diff) {
        Ok(message) => message,
        Err(err) => {
            eprintln!("error: failed to generate commit message: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("commit: git commit -am");
    let status = match Command::new("git")
        .arg("commit")
        .arg("-am")
        .arg(&message)
        .status()
    {
        Ok(status) => status,
        Err(err) => {
            eprintln!("error: failed to execute git commit: {err}");
            return ExitCode::FAILURE;
        }
    };

    if !status.success() {
        eprintln!("error: git commit exited with status {status}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn load_issue_context() -> Option<agent::IssueContext> {
    let issue_dir = active_issue_dir().ok()?;
    let issue_name = issue_dir.file_name()?.to_str()?.to_string();
    let summary = fs::read_to_string(issue_dir.join("SUMMARY.txt")).ok()?;
    let explanation = fs::read_to_string(issue_dir.join("EXPLANATION.md")).ok()?;

    Some(agent::IssueContext {
        issue_name,
        summary,
        explanation,
    })
}

fn git_diff_against_head() -> Result<String, String> {
    let mut diff = git_output(&["diff", "--no-ext-diff", "--no-color", "HEAD"])?;

    for untracked in git_untracked_files()? {
        let patch = git_diff_output(&[
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--no-index",
            "--",
            "/dev/null",
            &untracked,
        ])?;
        diff.push_str(&patch);
        if !patch.ends_with('\n') {
            diff.push('\n');
        }
    }

    Ok(diff)
}

fn git_untracked_files() -> Result<Vec<String>, String> {
    let output = git_output(&["ls-files", "--others", "--exclude-standard"])?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn git_output(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|err| format!("failed to execute git: {err}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_diff_output(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|err| format!("failed to execute git: {err}"))?;

    match output.status.code() {
        Some(0) | Some(1) => Ok(String::from_utf8_lossy(&output.stdout).to_string()),
        _ => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

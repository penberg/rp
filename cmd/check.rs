use crate::issues::resolve_issue_dir;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

pub struct CheckResult {
    pub verdict: String,
}

pub fn run(issue: Option<&str>) -> ExitCode {
    let result = match run_check_for(issue) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    match result.verdict.as_str() {
        "reproduced" => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

pub fn run_check_for(issue: Option<&str>) -> Result<CheckResult, String> {
    let issue_dir = resolve_issue_dir(issue)?;
    run_check(&issue_dir)
}

pub fn run_check(issue_dir: &Path) -> Result<CheckResult, String> {
    let reproducer = issue_dir.join("reproducer.sh");
    if !reproducer.is_file() {
        return Err(format!("missing reproducer {}", reproducer.display()));
    }

    let issue_name = issue_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    println!("check: issue {issue_name}");
    println!("check: reproducer {}", reproducer.display());
    println!("check: running reproducer");

    let output = Command::new("sh")
        .arg(&reproducer)
        .output()
        .map_err(|err| format!("failed to run {}: {err}", reproducer.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code();
    let verdict = verdict_for(exit_code);
    let stdout_path = issue_dir.join("check.stdout");
    let stderr_path = issue_dir.join("check.stderr");
    let status_path = issue_dir.join("check.status");

    write_check_artifacts(issue_dir, &stdout, &stderr, exit_code, &verdict)
        .map_err(|err| format!("failed to write check artifacts: {err}"))?;

    print_summary(
        &verdict,
        exit_code,
        &stdout_path,
        &stderr_path,
        &status_path,
    );
    print_reproducer_output(&stdout, &stderr, &verdict);

    Ok(CheckResult { verdict })
}

fn verdict_for(exit_code: Option<i32>) -> String {
    match exit_code {
        Some(1) => String::from("reproduced"),
        Some(0) => String::from("not_reproduced"),
        Some(_) | None => String::from("broken_reproducer"),
    }
}

fn print_summary(
    verdict: &str,
    exit_code: Option<i32>,
    stdout_path: &Path,
    stderr_path: &Path,
    status_path: &Path,
) {
    let exit = exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| String::from("signal"));

    match verdict {
        "reproduced" => println!("check: reproduced (exit {exit})"),
        "not_reproduced" => println!("check: not reproduced (exit {exit})"),
        "broken_reproducer" => println!("check: broken reproducer (exit {exit})"),
        other => println!("check: verdict {other} (exit {exit})"),
    }

    println!("check: stdout {}", stdout_path.display());
    println!("check: stderr {}", stderr_path.display());
    println!("check: status {}", status_path.display());
}

fn print_reproducer_output(stdout: &str, stderr: &str, verdict: &str) {
    if verdict != "broken_reproducer" {
        return;
    }

    if !stderr.trim().is_empty() {
        eprintln!("check: reproducer stderr:");
        eprint!("{stderr}");
        if !stderr.ends_with('\n') {
            eprintln!();
        }
    }

    if !stdout.trim().is_empty() {
        eprintln!("check: reproducer stdout:");
        eprint!("{stdout}");
        if !stdout.ends_with('\n') {
            eprintln!();
        }
    }
}

fn write_check_artifacts(
    issue_dir: &Path,
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
    verdict: &str,
) -> Result<(), std::io::Error> {
    fs::write(issue_dir.join("check.stdout"), stdout)?;
    fs::write(issue_dir.join("check.stderr"), stderr)?;

    let status = match exit_code {
        Some(code) => format!("verdict={verdict}\nexit_code={code}\n"),
        None => format!("verdict={verdict}\nexit_code=signal\n"),
    };

    fs::write(issue_dir.join("check.status"), status)?;
    fs::write(issue_dir.join("status"), format!("{verdict}\n"))?;
    Ok(())
}

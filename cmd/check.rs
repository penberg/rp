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

    write_check_artifacts(issue_dir, &stdout, &stderr, exit_code, &verdict)
        .map_err(|err| format!("failed to write check artifacts: {err}"))?;

    println!("check: verdict {verdict}");
    if let Some(code) = exit_code {
        println!("check: exit code {code}");
    } else {
        println!("check: exit code signal");
    }
    println!("check: wrote {}", issue_dir.join("check.status").display());

    Ok(CheckResult { verdict })
}

fn verdict_for(exit_code: Option<i32>) -> String {
    match exit_code {
        Some(1) => String::from("reproduced"),
        Some(0) => String::from("not_reproduced"),
        Some(_) | None => String::from("broken_reproducer"),
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

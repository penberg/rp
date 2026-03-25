use std::fs;
use std::path::{Path, PathBuf};

pub const ISSUES_DIR: &str = ".rp/issues";

pub fn active_issue_dir() -> Result<PathBuf, String> {
    let issues_dir = Path::new(ISSUES_DIR);
    if !issues_dir.is_dir() {
        return Err(format!("missing issues directory {}", issues_dir.display()));
    }

    let mut issues = fs::read_dir(issues_dir)
        .map_err(|err| format!("failed to read {}: {err}", issues_dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();

    issues.sort();

    match issues.len() {
        0 => Err(format!("no issues found in {}", issues_dir.display())),
        1 => Ok(issues.remove(0)),
        _ => {
            let names = issues
                .iter()
                .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "multiple issues found in {}: {}",
                issues_dir.display(),
                names
            ))
        }
    }
}

pub fn resolve_issue_dir(issue: Option<&str>) -> Result<PathBuf, String> {
    match issue {
        Some(issue) => issue_dir(issue),
        None => active_issue_dir(),
    }
}

fn issue_dir(issue: &str) -> Result<PathBuf, String> {
    let issues_dir = Path::new(ISSUES_DIR);
    if !issues_dir.is_dir() {
        return Err(format!("missing issues directory {}", issues_dir.display()));
    }

    let issue_dir = issues_dir.join(issue);
    if !issue_dir.is_dir() {
        return Err(format!(
            "issue {} not found in {}",
            issue,
            issues_dir.display()
        ));
    }

    Ok(issue_dir)
}

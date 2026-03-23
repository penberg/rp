use crate::agent;
use crate::config::effective_agent;
use crate::issues::ISSUES_DIR;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub fn run(prompt: &str, verbose: bool) -> ExitCode {
    let repo_root = Path::new(".");
    let issue_dir = issue_dir(prompt);
    let agent = match effective_agent() {
        Ok(agent) => agent,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("inspect: source {prompt}");
    println!("inspect: issue directory {}", issue_dir.display());
    println!("inspect: agent {agent}");

    if issue_dir.exists() {
        eprintln!("error: {} already exists", issue_dir.display());
        return ExitCode::FAILURE;
    }

    if let Err(err) = fs::create_dir_all(&issue_dir) {
        eprintln!("error: failed to create {}: {err}", issue_dir.display());
        return ExitCode::FAILURE;
    }

    println!("inspect: generating reproducer");

    let inspect = match agent::inspect(repo_root, prompt, &agent::InspectOptions { verbose }) {
        Ok(inspect) => inspect,
        Err(err) => {
            eprintln!("error: inspect failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    let files = [
        ("SOURCE.txt", format!("{prompt}\n")),
        ("SUMMARY.txt", format!("{}\n", inspect.summary)),
        ("inspect.md", inspect.inspect_markdown),
        ("reproducer.sh", inspect.reproducer_script),
        ("status", String::from("inspected\n")),
    ];

    for (name, content) in files {
        let path = issue_dir.join(name);
        if let Err(err) = fs::write(&path, content) {
            eprintln!("error: failed to write {}: {err}", path.display());
            return ExitCode::FAILURE;
        }

        println!("inspect: wrote {}", path.display());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = issue_dir.join("reproducer.sh");
        let permissions = fs::Permissions::from_mode(0o755);
        if let Err(err) = fs::set_permissions(&path, permissions) {
            eprintln!(
                "error: failed to update {} permissions: {err}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    }

    println!("inspect: done");
    ExitCode::SUCCESS
}

fn issue_dir(prompt: &str) -> PathBuf {
    Path::new(ISSUES_DIR).join(issue_key(prompt))
}

fn issue_key(prompt: &str) -> String {
    if let Some(number) = github_issue_number(prompt) {
        return number;
    }

    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in prompt.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }

        if slug.len() >= 48 {
            break;
        }
    }

    let slug = slug.trim_end_matches('-').to_string();

    if slug.is_empty() {
        String::from("inspect")
    } else {
        slug
    }
}

fn github_issue_number(prompt: &str) -> Option<String> {
    if prompt.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(prompt.to_string());
    }

    prompt
        .split("/issues/")
        .nth(1)
        .and_then(|tail| tail.split('/').next())
        .filter(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        .map(ToOwned::to_owned)
}

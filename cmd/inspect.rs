use crate::agent;
use crate::config::effective_agent;
use crate::issues::ISSUES_DIR;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(prompt: &str, verbose: bool) -> ExitCode {
    let repo_root = Path::new(".");
    let issue_ref = issue_ref(prompt);
    let issue_dir = issue_dir(&issue_ref.key);
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

    if issue_dir.exists() && !issue_ref.reuse_existing {
        eprintln!("error: {} already exists", issue_dir.display());
        return ExitCode::FAILURE;
    }

    if let Err(err) = fs::create_dir_all(&issue_dir) {
        eprintln!("error: failed to create {}: {err}", issue_dir.display());
        return ExitCode::FAILURE;
    }

    if issue_ref.reuse_existing {
        println!("inspect: reusing existing issue directory");
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

fn issue_dir(key: &str) -> PathBuf {
    Path::new(ISSUES_DIR).join(key)
}

struct IssueRef {
    key: String,
    reuse_existing: bool,
}

fn issue_ref(prompt: &str) -> IssueRef {
    if let Some(source_id) = canonical_source_id(prompt) {
        return IssueRef {
            key: format!("{}-{}", short_hash(&source_id), slugify(&source_id)),
            reuse_existing: true,
        };
    }

    let timestamp = prompt_timestamp();
    let fingerprint_input = format!("prompt:{timestamp}:{prompt}");

    IssueRef {
        key: format!("{}-{}", short_hash(&fingerprint_input), slugify(prompt)),
        reuse_existing: false,
    }
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in input.chars().flat_map(|c| c.to_lowercase()) {
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

fn short_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;

    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("{hash:016x}")[..12].to_string()
}

fn prompt_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn canonical_source_id(prompt: &str) -> Option<String> {
    canonical_github_issue(prompt)
        .or_else(|| canonical_gitlab_issue(prompt))
        .or_else(|| canonical_linear_issue(prompt))
}

fn canonical_github_issue(prompt: &str) -> Option<String> {
    let rest = prompt
        .strip_prefix("https://github.com/")
        .or_else(|| prompt.strip_prefix("http://github.com/"))?;
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    let marker = parts.next()?;
    let number = parts.next()?;

    if marker != "issues" || !is_ascii_digits(number) {
        return None;
    }

    Some(format!("github:{owner}/{repo}#{number}"))
}

fn canonical_gitlab_issue(prompt: &str) -> Option<String> {
    let rest = prompt
        .strip_prefix("https://gitlab.com/")
        .or_else(|| prompt.strip_prefix("http://gitlab.com/"))?;
    let parts = rest.split('/').collect::<Vec<_>>();
    let marker_index = parts.iter().position(|part| *part == "-")?;
    if parts.len() <= marker_index + 2 || parts.get(marker_index + 1) != Some(&"issues") {
        return None;
    }

    let project = parts[..marker_index].join("/");
    let number = parts[marker_index + 2];
    if project.is_empty() || !is_ascii_digits(number) {
        return None;
    }

    Some(format!("gitlab:{project}#{number}"))
}

fn canonical_linear_issue(prompt: &str) -> Option<String> {
    let rest = prompt
        .strip_prefix("https://linear.app/")
        .or_else(|| prompt.strip_prefix("http://linear.app/"))?;
    let mut parts = rest.split('/');
    let workspace = parts.next()?;
    let marker = parts.next()?;
    let issue_key = parts.next()?;

    if marker != "issue" || workspace.is_empty() || issue_key.is_empty() {
        return None;
    }

    Some(format!("linear:{workspace}/{}", issue_key.to_ascii_uppercase()))
}

fn is_ascii_digits(input: &str) -> bool {
    !input.is_empty() && input.chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_issues_reuse_stable_key() {
        let issue = issue_ref("https://github.com/penberg/weave/issues/7");
        assert!(issue.reuse_existing);
        assert!(issue.key.ends_with("-github-penberg-weave-7"));
        assert_eq!(issue.key.split('-').next().unwrap().len(), 12);
    }

    #[test]
    fn gitlab_issues_reuse_stable_key() {
        let issue = issue_ref("https://gitlab.com/group/project/-/issues/91");
        assert!(issue.reuse_existing);
        assert!(issue.key.ends_with("-gitlab-group-project-91"));
        assert_eq!(issue.key.split('-').next().unwrap().len(), 12);
    }

    #[test]
    fn linear_issues_reuse_stable_key() {
        let issue = issue_ref("https://linear.app/acme/issue/eng-42/fix-parser");
        assert!(issue.reuse_existing);
        assert!(issue.key.ends_with("-linear-acme-eng-42"));
        assert_eq!(issue.key.split('-').next().unwrap().len(), 12);
    }

    #[test]
    fn prompts_get_readable_slug() {
        assert_eq!(
            slugify("Run ./testing/sqlite3/all.test and reproduce the first failure you see"),
            "run-testing-sqlite3-all-test-and-reproduce-the-f"
        );
    }
}

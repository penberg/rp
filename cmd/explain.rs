use crate::issues::resolve_issue_dir;
use std::{
    fs,
    io::{self, IsTerminal},
    path::Path,
    process::ExitCode,
};
use termimad::MadSkin;

const EXPLAIN_WIDTH: usize = 80;

pub fn run(issue: Option<&str>) -> ExitCode {
    let issue_dir = match resolve_issue_dir(issue) {
        Ok(issue_dir) => issue_dir,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let issue_name = issue_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let summary = match fs::read_to_string(issue_dir.join("SUMMARY.txt")) {
        Ok(summary) => summary,
        Err(err) => {
            eprintln!(
                "error: failed to read {}: {err}",
                issue_dir.join("SUMMARY.txt").display()
            );
            return ExitCode::FAILURE;
        }
    };

    let explanation = match read_explanation(&issue_dir) {
        Ok(explanation) => explanation,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("Issue: {issue_name}");
    println!();
    println!("Summary:");
    println!("{}", render_markdown(&summary));
    println!();
    println!("Explanation:");
    println!("{}", render_markdown(&explanation));

    ExitCode::SUCCESS
}

fn read_explanation(issue_dir: &Path) -> Result<String, String> {
    let path = issue_dir.join("EXPLANATION.md");
    fs::read_to_string(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))
}

fn render_markdown(markdown: &str) -> String {
    let skin = if io::stdout().is_terminal() {
        MadSkin::default()
    } else {
        MadSkin::no_style()
    };

    let rendered = skin.text(markdown, Some(EXPLAIN_WIDTH)).to_string();

    rendered
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

use crate::config::effective_agent;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

const TRIAGE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "summary": { "type": "string" },
    "explanation_markdown": { "type": "string" },
    "reproducer_script": { "type": "string" }
  },
  "required": ["summary", "explanation_markdown", "reproducer_script"],
  "additionalProperties": false
}"#;

#[derive(Deserialize)]
pub struct InspectResult {
    pub summary: String,
    pub explanation_markdown: String,
    pub reproducer_script: String,
}

pub struct InspectOptions {
    pub verbose: bool,
}

pub fn inspect(
    repo_root: &Path,
    source: &str,
    options: &InspectOptions,
) -> Result<InspectResult, String> {
    let agent = effective_agent().map_err(|err| err.to_string())?;
    let prompt = inspect_prompt(source);

    let output = match agent.as_str() {
        "claude" => run_claude_inspect(repo_root, &prompt, options)?,
        "codex" => run_codex_inspect(repo_root, &prompt, options)?,
        "opencode" => run_opencode_inspect(repo_root, &prompt, options)?,
        other => return Err(format!("unsupported agent: {other}")),
    };

    parse_inspect_result(&output).map_err(|err| {
        format!(
            "failed to parse {} inspect output as JSON: {err}\noutput:\n{}",
            agent, output
        )
    })
}

pub fn fix(repo_root: &Path, issue_dir: &Path) -> Result<String, String> {
    let agent = effective_agent().map_err(|err| err.to_string())?;
    let prompt = fix_prompt(issue_dir)?;

    match agent.as_str() {
        "claude" => run_claude_fix(repo_root, &prompt),
        "codex" => run_codex_fix(repo_root, &prompt),
        "opencode" => run_opencode_fix(repo_root, &prompt),
        other => Err(format!("unsupported agent: {other}")),
    }
}

fn inspect_prompt(source: &str) -> String {
    format!(
        "You are generating a deterministic local bug reproducer for a repository.\n\
Return JSON only. Do not wrap it in Markdown.\n\
The JSON must have exactly these string keys: summary, explanation_markdown, reproducer_script.\n\
The reproducer_script must be a complete POSIX sh script that exits non-zero while the bug is present.\n\
The explanation_markdown should explain the issue in human terms, including the reasoning and any assumptions.\n\
Use the repository in the current working directory as context.\n\
If the source is a GitHub issue URL, inspect it and derive a concrete local reproducer.\n\
If exact reproduction is impossible, produce the closest deterministic reproducer you can and explain the gap.\n\
\n\
Source:\n\
{source}\n"
    )
}

fn fix_prompt(issue_dir: &Path) -> Result<String, String> {
    let source = read_optional(issue_dir.join("SOURCE.txt"))?;
    let summary = read_optional(issue_dir.join("SUMMARY.txt"))?;
    let inspect = read_optional(issue_dir.join("EXPLANATION.md"))?;
    let verify_cmd = crate::config::verify_command()
        .map_err(|err| format!("failed to read {}: {err}", crate::config::CONFIG_PATH))?;
    let test_integration = crate::config::test_integration_config()
        .map_err(|err| format!("failed to read {}: {err}", crate::config::CONFIG_PATH))?;

    let validation_block = if let Some(command) = verify_cmd {
        format!("Project verification command:\n- {command}\n")
    } else {
        String::from("No project verification command was configured.\n")
    };

    let test_block = if test_integration.roots.is_empty() {
        String::from("No repo-native test integration was configured.\n")
    } else {
        let roots = test_integration
            .roots
            .iter()
            .map(|root| format!("- {root}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut block = format!("Repo-native test roots:\n{roots}\n");

        if let Some(guidance) = &test_integration.guidance {
            block.push_str("Test integration guidance:\n");
            block.push_str(guidance);
            block.push('\n');
        }

        block.push_str("You must add or update a regression test under the configured roots when fixing this issue.\n");
        block
    };

    Ok(format!(
        "You are fixing a repository-local bug using existing triage artifacts.\n\
Work in the current repository and modify files in place.\n\
The active issue directory is: {}\n\
The reproducer is: {}/reproducer.sh\n\
\n\
Your goal is to make `sh {}/reproducer.sh` exit 0.\n\
After the reproducer passes, run the project validation commands below if available.\n\
Return a concise plain-text summary of what you changed.\n\
\n\
{}\n\
{}\n\
Issue source:\n{}\n\
\n\
Issue summary:\n{}\n\
\n\
Triage notes:\n{}\n",
        issue_dir.display(),
        issue_dir.display(),
        issue_dir.display(),
        validation_block,
        test_block,
        source.trim(),
        summary.trim(),
        inspect.trim(),
    ))
}

fn read_optional(path: std::path::PathBuf) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))
}

fn run_claude_fix(repo_root: &Path, prompt: &str) -> Result<String, String> {
    run_command(
        Command::new("claude")
            .current_dir(repo_root)
            .arg("-p")
            .arg("--dangerously-skip-permissions")
            .arg(prompt),
        "claude",
        &InspectOptions { verbose: false },
    )
}

fn run_codex_fix(repo_root: &Path, prompt: &str) -> Result<String, String> {
    run_command(
        Command::new("codex")
            .current_dir(repo_root)
            .arg("exec")
            .arg("--dangerously-bypass-approvals-and-sandbox")
            .arg("--skip-git-repo-check")
            .arg(prompt),
        "codex",
        &InspectOptions { verbose: false },
    )
}

fn run_opencode_fix(repo_root: &Path, prompt: &str) -> Result<String, String> {
    run_command(
        Command::new("opencode")
            .current_dir(repo_root)
            .arg("run")
            .arg(prompt),
        "opencode",
        &InspectOptions { verbose: false },
    )
}

fn run_claude_inspect(
    repo_root: &Path,
    prompt: &str,
    options: &InspectOptions,
) -> Result<String, String> {
    let mut command = Command::new("claude");
    command
        .current_dir(repo_root)
        .arg("-p")
        .arg("--dangerously-skip-permissions");

    if options.verbose {
        command
            .arg("--output-format")
            .arg("stream-json")
            .arg("--include-partial-messages")
            .arg("--verbose")
            .arg(prompt);

        let output = run_command(&mut command, "claude", options)?;
        extract_claude_stream_result(&output)
    } else {
        command
            .arg("--output-format")
            .arg("json")
            .arg("--json-schema")
            .arg(TRIAGE_SCHEMA)
            .arg(prompt);

        run_command(&mut command, "claude", options)
    }
}

fn run_codex_inspect(
    repo_root: &Path,
    prompt: &str,
    options: &InspectOptions,
) -> Result<String, String> {
    let output_path = repo_root.join(".rp").join("inspect-output.json");
    let schema_path = repo_root.join(".rp").join("inspect-schema.json");

    fs::write(&schema_path, TRIAGE_SCHEMA)
        .map_err(|err| format!("failed to write {}: {err}", schema_path.display()))?;

    let mut command = Command::new("codex");
    command
        .current_dir(repo_root)
        .arg("exec")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("--skip-git-repo-check")
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("--output-schema")
        .arg(&schema_path)
        .arg(prompt);

    if options.verbose {
        command.arg("--json");
    }

    run_command(&mut command, "codex", options)?;

    fs::read_to_string(&output_path)
        .map_err(|err| format!("failed to read {}: {err}", output_path.display()))
}

fn run_opencode_inspect(
    repo_root: &Path,
    prompt: &str,
    options: &InspectOptions,
) -> Result<String, String> {
    let mut command = Command::new("opencode");
    command.current_dir(repo_root).arg("run").arg(prompt);

    if options.verbose {
        command.arg("--format").arg("json").arg("--print-logs");
    }

    run_command(&mut command, "opencode", options)
}

fn run_command(
    command: &mut Command,
    name: &str,
    options: &InspectOptions,
) -> Result<String, String> {
    if options.verbose {
        eprintln!("inspect: backend command {:?}", command);
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to execute {name}: {err}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture {name} stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture {name} stderr"))?;

    let stdout_buffer = Arc::new(Mutex::new(String::new()));
    let stderr_buffer = Arc::new(Mutex::new(String::new()));

    let stdout_handle = stream_pipe(
        name.to_string(),
        "stdout".to_string(),
        stdout,
        Arc::clone(&stdout_buffer),
        options.verbose,
    );
    let stderr_handle = stream_pipe(
        name.to_string(),
        "stderr".to_string(),
        stderr,
        Arc::clone(&stderr_buffer),
        options.verbose,
    );

    let status = child
        .wait()
        .map_err(|err| format!("failed to wait for {name}: {err}"))?;

    stdout_handle
        .join()
        .map_err(|_| format!("failed to join {name} stdout reader"))?;
    stderr_handle
        .join()
        .map_err(|_| format!("failed to join {name} stderr reader"))?;

    let stdout = Arc::into_inner(stdout_buffer)
        .ok_or_else(|| format!("failed to finalize {name} stdout buffer"))?
        .into_inner()
        .map_err(|_| format!("failed to unlock {name} stdout buffer"))?;
    let stderr = Arc::into_inner(stderr_buffer)
        .ok_or_else(|| format!("failed to finalize {name} stderr buffer"))?
        .into_inner()
        .map_err(|_| format!("failed to unlock {name} stderr buffer"))?;

    if !status.success() {
        return Err(format!("{name} exited with status {}:\n{}", status, stderr));
    }

    Ok(stdout)
}

fn stream_pipe<R: std::io::Read + Send + 'static>(
    name: String,
    stream_name: String,
    reader: R,
    buffer: Arc<Mutex<String>>,
    verbose: bool,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if verbose {
                        if let Some(rendered) = render_verbose_line(&name, &stream_name, &line) {
                            eprintln!("{rendered}");
                        }
                    }

                    if let Ok(mut output) = buffer.lock() {
                        output.push_str(&line);
                    }
                }
                Err(err) => {
                    if verbose {
                        eprintln!("inspect: {name} {stream_name}: read error: {err}");
                    }
                    break;
                }
            }
        }
    })
}

fn render_verbose_line(name: &str, stream_name: &str, line: &str) -> Option<String> {
    if name == "claude" && stream_name == "stdout" {
        return render_claude_stream_line(line);
    }

    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("inspect: {name} {stream_name}: {trimmed}"))
    }
}

fn render_claude_stream_line(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    let event_type = value.get("type").and_then(Value::as_str)?;

    match event_type {
        "system" => render_claude_system_line(&value),
        "assistant" => {
            extract_claude_text(&value).map(|text| format!("inspect: claude: {}", text.trim()))
        }
        "result" => value
            .get("result")
            .and_then(Value::as_str)
            .map(|_| "inspect: claude: completed".to_string()),
        "stream_event" => render_claude_stream_event(&value),
        "user" => render_claude_user_tool_result(&value),
        _ => None,
    }
}

fn render_claude_system_line(value: &Value) -> Option<String> {
    let subtype = value.get("subtype").and_then(Value::as_str)?;

    match subtype {
        "init" => value
            .get("model")
            .and_then(Value::as_str)
            .map(|model| format!("inspect: claude: session started ({model})")),
        "api_retry" => {
            let attempt = value.get("attempt").and_then(Value::as_u64)?;
            Some(format!("inspect: claude: API retry {attempt}"))
        }
        _ => None,
    }
}

fn render_claude_stream_event(value: &Value) -> Option<String> {
    let event = value.get("event")?;
    let event_type = event.get("type").and_then(Value::as_str)?;

    match event_type {
        "content_block_start" => {
            let block = event.get("content_block")?;
            match block.get("type").and_then(Value::as_str) {
                Some("thinking") => Some("inspect: claude: thinking".to_string()),
                Some("tool_use") => {
                    let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
                    Some(format!("inspect: claude: using tool {name}"))
                }
                _ => None,
            }
        }
        "content_block_delta" => {
            let delta = event.get("delta")?;
            match delta.get("type").and_then(Value::as_str) {
                Some("text_delta") => delta
                    .get("text")
                    .and_then(Value::as_str)
                    .map(|text| format!("inspect: claude: {}", text.trim_end())),
                Some("thinking_delta") => delta
                    .get("thinking")
                    .and_then(Value::as_str)
                    .map(|text| format!("inspect: claude: thinking {}", text.trim_end())),
                Some("input_json_delta") => None,
                _ => None,
            }
        }
        "message_delta" => event
            .get("delta")
            .and_then(|delta| delta.get("stop_reason"))
            .and_then(Value::as_str)
            .map(|reason| format!("inspect: claude: stop reason {reason}")),
        "message_stop" => Some("inspect: claude: message complete".to_string()),
        _ => None,
    }
}

fn render_claude_user_tool_result(value: &Value) -> Option<String> {
    let content = value.get("message")?.get("content")?.as_array()?;

    for item in content {
        if item.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }

        let tool_output = item.get("content").and_then(Value::as_str).unwrap_or("");
        let first_line = tool_output.lines().next().unwrap_or("").trim();

        if first_line.is_empty() {
            return Some("inspect: claude: tool returned output".to_string());
        }

        let summary = truncate_for_log(first_line, 120);
        return Some(format!("inspect: claude: tool output {summary}"));
    }

    None
}

fn truncate_for_log(text: &str, max_chars: usize) -> String {
    let mut result = String::new();

    for ch in text.chars().take(max_chars) {
        result.push(ch);
    }

    if text.chars().count() > max_chars {
        result.push_str("...");
    }

    result
}

fn parse_inspect_result(output: &str) -> Result<InspectResult, serde_json::Error> {
    if let Ok(result) = serde_json::from_str(output) {
        return Ok(result);
    }

    let trimmed = output.trim();

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(structured_output) = value.get("structured_output") {
            if let Ok(result) = serde_json::from_value::<InspectResult>(structured_output.clone()) {
                return Ok(result);
            }
        }

        if let Some(result_text) = value.get("result").and_then(Value::as_str) {
            if let Ok(result) = serde_json::from_str::<InspectResult>(result_text) {
                return Ok(result);
            }
        }
    }

    if let Some(stripped) = trimmed
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
    {
        return serde_json::from_str(stripped.trim());
    }

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        return serde_json::from_str(&trimmed[start..=end]);
    }

    serde_json::from_str(trimmed)
}

fn extract_claude_stream_result(output: &str) -> Result<String, String> {
    let mut last_text: Option<String> = None;

    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(result) = value.get("result").and_then(Value::as_str) {
            return Ok(result.to_string());
        }

        if let Some(text) = extract_claude_text(&value) {
            last_text = Some(text);
        }
    }

    last_text.ok_or_else(|| "failed to extract final result from Claude stream output".to_string())
}

fn extract_claude_text(value: &Value) -> Option<String> {
    if let Some(message) = value.get("message") {
        if let Some(text) = message.get("text").and_then(Value::as_str) {
            return Some(text.to_string());
        }

        if let Some(content) = message.get("content").and_then(Value::as_array) {
            let text = content
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("");

            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    if let Some(content) = value.get("content").and_then(Value::as_array) {
        let text = content
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("");

        if !text.is_empty() {
            return Some(text);
        }
    }

    if let Some(delta) = value.get("delta").and_then(Value::as_str) {
        return Some(delta.to_string());
    }

    None
}

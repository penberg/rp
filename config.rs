use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_PATH: &str = ".rp.yml";
pub const LOCAL_CONFIG_DIR: &str = ".rp";
pub const LOCAL_CONFIG_PATH: &str = ".rp/config";
pub const ISSUES_DIR: &str = ".rp/issues";

const DEFAULT_AGENTS: [&str; 3] = ["claude", "codex", "opencode"];

#[derive(Clone, Default)]
pub struct TestIntegrationConfig {
    pub roots: Vec<String>,
    pub guidance: Option<String>,
}

pub fn local_config_dir() -> PathBuf {
    PathBuf::from(LOCAL_CONFIG_DIR)
}

pub fn read_local_config(path: &Path) -> Result<BTreeMap<String, String>, std::io::Error> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let content = fs::read_to_string(path)?;
    let mut entries = BTreeMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            entries.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    Ok(entries)
}

pub fn write_local_config(
    path: &Path,
    entries: &BTreeMap<String, String>,
) -> Result<(), std::io::Error> {
    let mut content = String::new();

    for (key, value) in entries {
        content.push_str(key);
        content.push_str(" = ");
        content.push_str(value);
        content.push('\n');
    }

    fs::write(path, content)
}

pub fn default_agent() -> Option<&'static str> {
    DEFAULT_AGENTS
        .iter()
        .copied()
        .find(|agent| command_exists(agent))
}

pub fn effective_agent() -> Result<String, &'static str> {
    let path = Path::new(LOCAL_CONFIG_PATH);
    let entries = read_local_config(path).map_err(|_| "failed to read local config")?;

    if let Some(agent) = entries.get("agent") {
        return Ok(agent.clone());
    }

    default_agent()
        .map(str::to_string)
        .ok_or("agent is not configured and no default agent was found in PATH")
}

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

pub fn verify_command() -> Result<Option<String>, std::io::Error> {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(command) = trimmed.strip_prefix("verify-cmd: ") {
            return Ok(Some(command.to_string()));
        }
    }

    // Backward compatibility with the older project_validation list.
    let mut commands = Vec::new();
    let mut in_project_validation = false;

    for line in content.lines() {
        let trimmed = line.trim_end();

        if trimmed == "project_validation:" {
            in_project_validation = true;
            continue;
        }

        if in_project_validation {
            let list_item = trimmed.trim_start();
            if let Some(command) = list_item.strip_prefix("- ") {
                commands.push(command.to_string());
                continue;
            }

            if !trimmed.starts_with(' ') && !trimmed.is_empty() {
                break;
            }
        }
    }

    Ok(commands.into_iter().next())
}

pub fn test_integration_config() -> Result<TestIntegrationConfig, std::io::Error> {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Ok(TestIntegrationConfig::default());
    }

    let content = fs::read_to_string(path)?;
    let mut config = TestIntegrationConfig::default();
    let mut in_section = false;
    let mut in_tests = false;
    let mut in_guidance = false;

    for line in content.lines() {
        let trimmed = line.trim_end();

        if trimmed == "tests:" {
            in_tests = true;
            in_section = false;
            in_guidance = false;
            continue;
        }

        if trimmed == "guidance: |" {
            in_guidance = true;
            in_tests = false;
            in_section = false;
            config.guidance = Some(String::new());
            continue;
        }

        if trimmed == "test_integration:" {
            in_section = true;
            in_tests = false;
            in_guidance = false;
            continue;
        }

        if in_guidance {
            if let Some(rest) = trimmed.strip_prefix("  ") {
                let guidance = config.guidance.get_or_insert_with(String::new);
                if !guidance.is_empty() {
                    guidance.push('\n');
                }
                guidance.push_str(rest);
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("    ") {
                let guidance = config.guidance.get_or_insert_with(String::new);
                if !guidance.is_empty() {
                    guidance.push('\n');
                }
                guidance.push_str(rest);
                continue;
            }
            in_guidance = false;
        }

        if in_tests {
            let entry = trimmed.trim_start();
            if let Some(value) = entry.strip_prefix("- ") {
                config.roots.push(value.to_string());
                continue;
            }
            if !trimmed.starts_with(' ') && !trimmed.is_empty() {
                in_tests = false;
            }
        }

        if !in_section {
            continue;
        }

        if !trimmed.starts_with(' ') && !trimmed.is_empty() {
            break;
        }

        let entry = trimmed.trim_start();

        if let Some(value) = entry.strip_prefix("- ") {
            config.roots.push(value.to_string());
        } else if entry == "guidance: |" {
            in_guidance = true;
            config.guidance = Some(String::new());
        }
    }

    Ok(config)
}

fn command_exists(command: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&paths).any(|dir| dir.join(command).is_file())
}

pub fn detect_project_config() -> String {
    let verify_cmd = match detect_project_type() {
        ProjectType::Rust => Some("cargo test"),
        ProjectType::Node => Some("npm test"),
        ProjectType::Python => Some("pytest"),
        ProjectType::Go => Some("go test ./..."),
        ProjectType::Make => Some("make test"),
        ProjectType::Unknown => None,
    };

    let test_integration = detect_test_integration();

    render_config(verify_cmd, &test_integration)
}

fn render_config(
    verify_cmd: Option<&str>,
    test_integration: &Option<TestIntegrationConfig>,
) -> String {
    let mut config = String::from("version: 1\n\n");

    if let Some(verify_cmd) = verify_cmd {
        config.push_str("verify-cmd: ");
        config.push_str(verify_cmd);
        config.push('\n');
    }

    if let Some(test_integration) = test_integration {
        if !test_integration.roots.is_empty() {
            config.push_str("\ntests:\n");
            for root in &test_integration.roots {
                config.push_str("  - ");
                config.push_str(root);
                config.push('\n');
            }
        }

        if let Some(guidance) = &test_integration.guidance {
            config.push_str("\nguidance: |\n");
            for line in guidance.lines() {
                config.push_str("  ");
                config.push_str(line);
                config.push('\n');
            }
        }
    }

    config
}

fn detect_test_integration() -> Option<TestIntegrationConfig> {
    if Path::new("testing/conformance").is_dir() {
        Some(TestIntegrationConfig {
            roots: vec![String::from("testing/conformance")],
            guidance: Some(String::from(
                "Prefer adding a minimal regression test under testing/conformance.\n\
Choose the most specific suite for the bug, and update the suite Makefile when needed.",
            )),
        })
    } else {
        None
    }
}

fn detect_project_type() -> ProjectType {
    if Path::new("Cargo.toml").exists() {
        ProjectType::Rust
    } else if Path::new("package.json").exists() {
        ProjectType::Node
    } else if Path::new("pyproject.toml").exists()
        || Path::new("setup.py").exists()
        || Path::new("requirements.txt").exists()
    {
        ProjectType::Python
    } else if Path::new("go.mod").exists() {
        ProjectType::Go
    } else if Path::new("Makefile").exists() {
        ProjectType::Make
    } else {
        ProjectType::Unknown
    }
}

#[derive(Clone, Copy)]
enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Make,
    Unknown,
}

impl ProjectType {}

use crate::config::{
    LOCAL_CONFIG_DIR, LOCAL_CONFIG_PATH, default_agent, local_config_dir, read_local_config,
    write_local_config,
};
use clap::Subcommand;
use std::path::Path;
use std::process::ExitCode;

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// List all options in config file
    List,
    /// Set value for a config option
    Set { key: String, value: String },
    /// Print value for a config option
    Get { key: String },
}

pub fn run(command: Option<ConfigCommand>) -> ExitCode {
    match command {
        None | Some(ConfigCommand::List) => list(),
        Some(ConfigCommand::Set { key, value }) => set(&key, &value),
        Some(ConfigCommand::Get { key }) => get(&key),
    }
}

fn list() -> ExitCode {
    let path = Path::new(LOCAL_CONFIG_PATH);
    let mut entries = match read_local_config(path) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("error: failed to read {}: {err}", LOCAL_CONFIG_PATH);
            return ExitCode::FAILURE;
        }
    };

    if !entries.contains_key("agent") {
        if let Some(agent) = default_agent() {
            entries.insert("agent".to_string(), agent.to_string());
        }
    }

    for (key, value) in entries {
        println!("{key}={value}");
    }

    ExitCode::SUCCESS
}

fn set(key: &str, value: &str) -> ExitCode {
    let path = Path::new(LOCAL_CONFIG_PATH);

    if let Err(err) = std::fs::create_dir_all(local_config_dir()) {
        eprintln!("error: failed to create {}: {err}", LOCAL_CONFIG_DIR);
        return ExitCode::FAILURE;
    }

    let mut entries = match read_local_config(path) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("error: failed to read {}: {err}", LOCAL_CONFIG_PATH);
            return ExitCode::FAILURE;
        }
    };

    entries.insert(key.to_string(), value.to_string());

    if let Err(err) = write_local_config(path, &entries) {
        eprintln!("error: failed to write {}: {err}", LOCAL_CONFIG_PATH);
        return ExitCode::FAILURE;
    }

    println!("set {} = {} in {}", key, value, LOCAL_CONFIG_PATH);
    ExitCode::SUCCESS
}

fn get(key: &str) -> ExitCode {
    let path = Path::new(LOCAL_CONFIG_PATH);
    let entries = match read_local_config(path) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("error: failed to read {}: {err}", LOCAL_CONFIG_PATH);
            return ExitCode::FAILURE;
        }
    };

    match entries.get(key) {
        Some(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        None if key == "agent" => match default_agent() {
            Some(agent) => {
                println!("{agent}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!(
                    "error: agent is not set in {} and no default agent was found in PATH",
                    LOCAL_CONFIG_PATH
                );
                ExitCode::FAILURE
            }
        },
        None => {
            eprintln!("error: {key} is not set in {}", LOCAL_CONFIG_PATH);
            ExitCode::FAILURE
        }
    }
}

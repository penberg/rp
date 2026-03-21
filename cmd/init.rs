use crate::config::{CONFIG_PATH, detect_project_config};
use std::fs;
use std::path::Path;
use std::process::ExitCode;

pub fn run() -> ExitCode {
    let path = Path::new(CONFIG_PATH);

    if path.exists() {
        eprintln!("error: {} already exists", CONFIG_PATH);
        return ExitCode::FAILURE;
    }

    let config = detect_project_config();

    if let Err(err) = fs::write(path, config) {
        eprintln!("error: failed to write {}: {err}", CONFIG_PATH);
        return ExitCode::FAILURE;
    }

    println!("created {}", CONFIG_PATH);
    ExitCode::SUCCESS
}

//! Helpers shared by the tests that generate a workspace and build it.

use std::{path::Path, process::Command};

/// Runs `program` with `arguments` in `directory`, panicking with its output
/// when it fails, and returns its standard output.
pub fn run(program: &str, arguments: &[&str], directory: &Path) -> String {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap_or_else(|error| panic!("Unable to run {program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {arguments:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 output")
}

/// Returns the sorted directory names of the crates of the workspace at
/// `path`.
pub fn crate_directories(path: &Path) -> Vec<String> {
    let mut directories: Vec<String> = std::fs::read_dir(path)
        .expect("Unable to list the workspace")
        .map(|entry| entry.expect("Unable to read a workspace entry"))
        .filter(|entry| entry.path().join("Cargo.toml").exists())
        .map(|entry| entry.file_name().into_string().expect("UTF-8 name"))
        .collect();
    directories.sort_unstable();
    directories
}

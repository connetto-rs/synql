//! Test module to verify workspace generation in a `tmp` directory.
//!
//! The test simply verifies that `synql` is able to process successfully
//! the EMI database schema and generate the workspace without errors,
//! cleaning up the temporary directory after the tests complete.

use std::process::Command;

use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

#[test]
fn test_aps() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let workspace_path = temp_dir.path();
    let db: ParserDB = ParserDB::from_git_url::<PostgreSqlDialect>(
        "https://github.com/earth-metabolome-initiative/asset-procedure-schema.git",
    )?;

    let synql: SynQL<ParserDB> = SynQL::new(&db, workspace_path)
        .name("synql")
        .clear_existing()
        .generate_workspace_toml()
        .generate_rustfmt()
        .sink_crate("sink")
        .into();

    synql.generate().expect("Unable to generate workspace");

    // Verify that Cargo.toml exists
    let cargo_toml = workspace_path.join("Cargo.toml");
    assert!(cargo_toml.exists(), "Cargo.toml should be created");

    // Runs the `cargo fmt` command in the specified directory.
    let output = Command::new("cargo").arg("fmt").current_dir(workspace_path).output()?;

    if !output.status.success() {
        eprintln!("cargo fmt stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cargo fmt stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cargo fmt failed for generated workspace");
    }
    // Verify that the generated workspace can be checked
    let output = Command::new("cargo").arg("check").current_dir(workspace_path).output()?;

    if !output.status.success() {
        eprintln!("cargo check stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cargo check stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cargo check failed for generated workspace");
    }

    Ok(())
}

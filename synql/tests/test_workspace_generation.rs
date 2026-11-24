//! Test module to verify workspace generation in a `tmp` directory.
//!
//! The test includes generating a SQL-based workspace and checking
//! that the expected files pass a workspace-level `cargo check` and
//! `cargo test`, as some of the generated code may tests, and finally
//! cleaning up the temporary directory after the tests complete.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

#[test]
fn generated_workspace_takes_diesel_from_crates_io() {
    let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT PRIMARY KEY);")
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("diesel_workspace");
    let synql: SynQL<ParserDB> =
        SynQL::new(&db, &workspace_path).name("diesel-workspace").generate_workspace_toml().into();
    synql.generate().expect("Unable to generate workspace");

    let manifest = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    let diesel = manifest
        .lines()
        .find(|line| line.starts_with("diesel = "))
        .expect("The workspace declares diesel");
    assert!(diesel.contains("version = "), "diesel has no version: {diesel}");
    assert!(!diesel.contains("git = "), "diesel comes from git: {diesel}");
    assert!(
        !manifest.contains("[patch.crates-io]"),
        "the workspace patches crates.io:\n{manifest}"
    );
}

#[test]
fn a_registered_crate_followed_on_git_is_patched_for_crates_io() {
    let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT PRIMARY KEY);")
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("patched_workspace");
    let followed = ExternalCrate::new("followed")
        .expect("A valid crate name")
        .version("1.0")
        .expect("A valid version")
        .git("https://github.com/connetto-rs/followed", "main")
        .expect("A valid git source")
        .into();
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("patched-workspace")
        .external_crate(followed)
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    let manifest = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    let patches: Vec<&str> = manifest
        .lines()
        .skip_while(|line| *line != "[patch.crates-io]")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(
        patches,
        ["followed = { git = \"https://github.com/connetto-rs/followed\", branch = \"main\" }"],
        "{manifest}"
    );
}

#[test]
fn caller_patches_follow_the_derived_ones_and_replace_those_they_name() {
    let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT PRIMARY KEY);")
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("caller_patched_workspace");
    let followed = |name: &str| -> ExternalCrate {
        ExternalCrate::new(name)
            .expect("A valid crate name")
            .version("1.0")
            .expect("A valid version")
            .git(format!("https://github.com/connetto-rs/{name}").as_str(), "main")
            .expect("A valid git source")
            .into()
    };
    let fork = |name: &str| {
        TomlDependency::new(name)
            .git("https://github.com/connetto-rs/fork", Some("fix"))
            .expect("A valid git source")
    };
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("caller-patched-workspace")
        .external_crate(followed("replaced"))
        .external_crate(followed("kept"))
        .crates_io_patch(TomlDependency::new("sqlstate").version("9").expect("A valid version"))
        .crates_io_patch(fork("sqlstate"))
        .crates_io_patch(fork("replaced"))
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    let manifest = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    let patches: Vec<&str> = manifest
        .lines()
        .skip_while(|line| *line != "[patch.crates-io]")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(
        patches,
        [
            "kept = { git = \"https://github.com/connetto-rs/kept\", branch = \"main\" }",
            "sqlstate = { git = \"https://github.com/connetto-rs/fork\", branch = \"fix\" }",
            "replaced = { git = \"https://github.com/connetto-rs/fork\", branch = \"fix\" }",
        ],
        "{manifest}"
    );
}

#[test]
fn caller_patches_alone_write_the_patch_section() {
    let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT PRIMARY KEY);")
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("diesel_patched_workspace");
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("diesel-patched-workspace")
        .crates_io_patch(
            TomlDependency::new("diesel")
                .git("https://github.com/diesel-rs/diesel", Some("main"))
                .expect("A valid git source"),
        )
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    let manifest = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    let patches: Vec<&str> = manifest
        .lines()
        .skip_while(|line| *line != "[patch.crates-io]")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(
        patches,
        ["diesel = { git = \"https://github.com/diesel-rs/diesel\", branch = \"main\" }"],
        "{manifest}"
    );
}

#[test]
fn test_workspace_generation() -> Result<(), Box<dyn std::error::Error>> {
    let db = ParserDB::parse::<GenericDialect>(
        "
		CREATE TABLE users (
		    id SERIAL PRIMARY KEY,
		    name TEXT NOT NULL CHECK (name <> ''),
            description TEXT,
		    email TEXT UNIQUE NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CHECK (name <> description),
            CHECK (created_at <= updated_at)
		);
        CREATE TABLE comments (
		    id SERIAL PRIMARY KEY,
		    comment TEXT NOT NULL CHECK (comment <> ''),
            user_id INT REFERENCES users(id)
		);
        CREATE TABLE extended_comments (
            id INT PRIMARY KEY REFERENCES comments(id),
            extra_info TEXT
        );
",
    )?;
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_workspace");
    // let workspace_path = std::path::PathBuf::from("../../../../local");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-workspace")
        .generate_workspace_toml()
        .generate_rustfmt()
        .into();
    synql.generate().expect("Unable to generate workspace");

    // Verify that the workspace directory was created
    assert!(workspace_path.exists(), "Workspace directory should be created");

    // Verify that Cargo.toml exists
    let cargo_toml = workspace_path.join("Cargo.toml");
    assert!(cargo_toml.exists(), "Cargo.toml should be created");

    // Runs the `cargo fmt` command in the specified directory.
    let output = Command::new("cargo").arg("fmt").current_dir(&workspace_path).output()?;

    if !output.status.success() {
        eprintln!("cargo fmt stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cargo fmt stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cargo fmt failed for generated workspace");
    }

    // Verify that the generated workspace can be checked
    let output = Command::new("cargo").arg("check").current_dir(&workspace_path).output()?;

    if !output.status.success() {
        eprintln!("cargo check stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cargo check stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cargo check failed for generated workspace");
    }

    // Verify that the generated workspace can be tested
    let output = Command::new("cargo").arg("test").current_dir(&workspace_path).output()?;

    if !output.status.success() {
        eprintln!("cargo test stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cargo test stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cargo test failed for generated workspace");
    }

    // Verify that the generated documentation can be built without errors or
    // warnings
    let output = Command::new("cargo")
        .args(["doc", "--no-deps", "--document-private-items"])
        .current_dir(&workspace_path)
        .output()?;

    if !output.status.success() {
        eprintln!("cargo doc stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cargo doc stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cargo doc failed for generated workspace");
    }

    Ok(())
}

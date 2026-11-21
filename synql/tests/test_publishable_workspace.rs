//! A workspace generated with a license and a repository packages for
//! crates.io, every crate it generates included.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TYPE mood AS ENUM ('happy', 'sad');
CREATE TABLE users (id INT PRIMARY KEY, mood mood NOT NULL);
CREATE TABLE comments (id INT PRIMARY KEY, user_id INT NOT NULL REFERENCES users (id));
CREATE TABLE extended_comments (id INT PRIMARY KEY REFERENCES comments (id), extra TEXT);
";

#[test]
fn generated_workspace_packages_for_crates_io() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("publishable_workspace");
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("publishable")
        .version(0, 3, 1)
        .license("MIT")
        .repository("https://github.com/connetto-rs/publishable")
        .sql(SCHEMA)
        .sink_crate("publishable-sink")
        .dag_sink_crate_prefix("publishable-dag-")
        .one_crate_per_table()
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    let output = Command::new("cargo")
        .args(["package", "--workspace", "--no-verify", "--allow-dirty"])
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo package");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "cargo package failed:\n{stderr}");
    assert!(!stderr.contains("warning: manifest has no"), "a crate lacks metadata:\n{stderr}");

    let comments = std::fs::read_to_string(workspace_path.join("publishable-comments/Cargo.toml"))
        .expect("Unable to read the manifest of the comments crate");
    assert!(
        comments
            .lines()
            .any(|line| line.starts_with("description = ") && line.contains("comments")),
        "the comments crate does not describe its table:\n{comments}"
    );
    let manifest = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    assert!(
        manifest.contains(
            "publishable-users = { version = \"=0.3.1\", path = \"./publishable-users\" }"
        ),
        "the users crate is not pinned at the workspace version:\n{manifest}"
    );
}

//! A table whose composite key to its ancestor also holds its inherited
//! primary key inserts through its builder, and the other columns of that key
//! still take their value from the descendant.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test` on SQLite.

use std::{path::Path, process::Command};

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE parents (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL,
    UNIQUE (id, project_id)
);
CREATE TABLE children (
    id UUID PRIMARY KEY REFERENCES parents (id),
    project_id UUID NOT NULL,
    FOREIGN KEY (id, project_id) REFERENCES parents (id, project_id)
);
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel = { workspace = true, features = ["sqlite", "returning_clauses_for_sqlite_3_35"] }
diesel-builders.workspace = true
rosetta-uuid.workspace = true
demo-children.workspace = true
demo-parents.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over an inherited key on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use demo_children::*;
use demo_parents::*;

#[test]
fn the_child_and_its_parent_share_the_key_and_the_project() {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    connection
        .batch_execute(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE parents (
                id BLOB PRIMARY KEY,
                project_id BLOB NOT NULL,
                UNIQUE (id, project_id)
            );
            CREATE TABLE children (
                id BLOB PRIMARY KEY REFERENCES parents (id),
                project_id BLOB NOT NULL,
                FOREIGN KEY (id, project_id) REFERENCES parents (id, project_id)
            );",
        )
        .expect("create the tables");
    let id = rosetta_uuid::Uuid::new_v4();
    let project_id = rosetta_uuid::Uuid::new_v4();
    // Both tables have setters named after the shared columns.
    let builder = SetParentId::id(children::table::builder(), id);
    let child = SetChildProjectId::project_id(builder, project_id)
        .insert(&mut connection)
        .expect("insert the child");
    let parent: Parent = child.ancestor(&mut connection).expect("load the parent");
    assert_eq!(parent.get_column_ref::<parents::id>(), &id);
    assert_eq!(parent.get_column_ref::<parents::project_id>(), &project_id);
    assert_eq!(child.get_column_ref::<children::project_id>(), &project_id);
}
"#;

#[test]
fn a_child_keyed_to_its_parent_by_a_composite_key_inserts() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("demo");

    let synql: SynQL<ParserDB> =
        SynQL::new_with_crate_base_path(&db, &workspace_path, Path::new("models"))
            .name("demo")
            .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
            .generate_workspace_toml()
            .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/inherited_key.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over an inherited key on SQLite.\n"),
    ] {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    let output = Command::new("cargo")
        .args(["test", "-p", "assertions"])
        .env("RUSTFLAGS", "-D warnings")
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo test");
    assert!(
        output.status.success(),
        "The inherited key failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

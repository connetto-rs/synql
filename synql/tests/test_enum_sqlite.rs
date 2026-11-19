//! A model with an enum column loads from and round trips through the SQLite
//! table pg2sqlite emits, where the enum column holds its label in `TEXT`
//! under a `CHECK` on the labels.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test`.

use std::{path::Path, process::Command};

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TYPE role AS ENUM ('read', 'write', 'admin');
CREATE TABLE grants (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), role role NOT NULL);
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel = { workspace = true, features = ["sqlite", "returning_clauses_for_sqlite_3_35"] }
diesel-builders.workspace = true
rosetta-uuid.workspace = true
demo-grants.workspace = true
demo-public-role.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over the generated models on SQLite.

use diesel::prelude::*;
use diesel_builders::prelude::*;
use demo_grants::*;
use demo_public_role::{Role, sql_types};

/// Opens an in-memory SQLite database holding the `grants` table pg2sqlite
/// emits.
fn connection() -> SqliteConnection {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    diesel::sql_query(
        "CREATE TABLE grants (id BLOB PRIMARY KEY, role TEXT NOT NULL CHECK (role IN ('read', 'write', 'admin')))",
    )
    .execute(&mut connection)
    .expect("create the table pg2sqlite emits");
    connection
}

#[test]
fn a_grant_loads_from_sqlite() {
    let mut connection = connection();
    let grants: Vec<Grant> =
        grants::table.select(Grant::as_select()).load(&mut connection).expect("load the grants");
    assert!(grants.is_empty());
}

#[test]
fn a_grant_round_trips_through_sqlite() {
    let mut connection = connection();
    let id = rosetta_uuid::Uuid::new_v4();
    let inserted = grants::table::builder()
        .id(id)
        .role(Role::Admin)
        .insert(&mut connection)
        .expect("insert the grant");
    let loaded: Grant = grants::table
        .find(id)
        .select(Grant::as_select())
        .first(&mut connection)
        .expect("load the grant");
    assert_eq!(loaded.get_column_ref::<grants::role>(), &Role::Admin);
    assert_eq!(loaded, inserted);
}

#[test]
fn sqlite_refuses_an_unknown_label() {
    let mut connection = connection();
    // A label outside the type, which the `CHECK` keeps out of the table.
    let unknown = diesel::dsl::sql::<sql_types::Role>("'owner'");
    let error = diesel::select(unknown)
        .get_result::<Role>(&mut connection)
        .expect_err("`owner` is no label of the type");
    assert!(error.to_string().contains("owner"), "{error}");
}
"#;

#[test]
fn enum_columns_load_from_sqlite() {
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
        ("assertions/tests/sqlite_load.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over the generated models on SQLite.\n"),
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
        "The generated models failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

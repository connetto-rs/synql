//! A column with a SQL default stays unset in the builder until its caller
//! sets it, and an insert leaves an unset one out, so the database applies the
//! default and the inserted model holds the stored value. A primary key
//! column and a column tied to another builder keep the Rust value of their
//! default, since the builder must hold their value before the insert.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test` against SQLite
//! tables standing in for the PostgreSQL ones.

use std::{path::Path, process::Command};

use quote::quote;
use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE FUNCTION next_priority() RETURNS INT AS 'SELECT 3' LANGUAGE SQL;
CREATE TABLE rules (
    id INT PRIMARY KEY,
    name TEXT NOT NULL,
    in_use BOOLEAN NOT NULL DEFAULT FALSE,
    note TEXT DEFAULT 'none',
    priority INT NOT NULL DEFAULT next_priority()
);
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel = { workspace = true, features = ["sqlite", "returning_clauses_for_sqlite_3_35"] }
diesel-builders.workspace = true
defaults-rules.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over columns with a SQL default, on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use defaults_rules::*;

/// Opens an in-memory SQLite database whose table stands in for the
/// PostgreSQL one.
///
/// Each SQLite default differs from the PostgreSQL default of its column, so
/// a model holding the SQLite value shows that the insert left the column
/// out.
fn connection() -> SqliteConnection {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    connection
        .batch_execute(
            "CREATE TABLE rules (
                id INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                in_use INTEGER NOT NULL DEFAULT 1,
                note TEXT DEFAULT 'kept',
                priority INTEGER NOT NULL DEFAULT 7
            );",
        )
        .expect("create the stand-in table");
    connection
}

#[test]
fn unset_columns_take_the_database_default() {
    let mut connection = connection();
    let builder = rules::table::builder().id(1).name("fridge".to_owned());
    assert_eq!(builder.may_get_column_ref::<rules::in_use>(), None);
    assert_eq!(builder.may_get_column_ref::<rules::note>(), None);
    assert_eq!(builder.may_get_column_ref::<rules::priority>(), None);

    let rule = builder.insert(&mut connection).expect("insert the rule without its defaulted columns");
    assert_eq!(rule.get_column_ref::<rules::in_use>(), &true);
    assert_eq!(rule.get_column_ref::<rules::note>(), &Some("kept".to_owned()));
    assert_eq!(rule.get_column_ref::<rules::priority>(), &7);
}

#[test]
fn set_columns_are_written() {
    let mut connection = connection();
    let rule = rules::table::builder()
        .id(1)
        .name("freezer".to_owned())
        .in_use(false)
        .note(None::<String>)
        .priority(9)
        .insert(&mut connection)
        .expect("insert the rule with every column set");
    assert_eq!(rule.get_column_ref::<rules::in_use>(), &false);
    assert_eq!(rule.get_column_ref::<rules::note>(), &None);
    assert_eq!(rule.get_column_ref::<rules::priority>(), &9);
}
"#;

/// Returns the default decorator of `column_name` in `table_name` of
/// `schema`.
fn default_decorator(schema: &str, table_name: &str, column_name: &str) -> String {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let workspace: Workspace =
        Workspace::new().name("defaults").expect("A valid workspace name").core().std().into();
    let table = db
        .table_by_target(TargetName::new(table_name, false), IdentifierCase::AsWritten)
        .expect("Unable to look up the table")
        .expect("The table exists");
    let column = table
        .column(column_name, &db, IdentifierCase::AsWritten)
        .expect("Unable to look up the column")
        .expect("The column exists");
    column
        .generate_default_decorator(&workspace, &db)
        .expect("The default has a decorator")
        .to_string()
}

#[test]
fn a_primary_key_column_keeps_the_rust_value_of_its_default() {
    let schema = "CREATE TABLE t (id INT PRIMARY KEY DEFAULT 5, x INT NOT NULL DEFAULT 5);";
    assert_eq!(
        default_decorator(schema, "t", "id"),
        quote! { #[table_model(default = 5i32)] }.to_string()
    );
    assert_eq!(
        default_decorator(schema, "t", "x"),
        quote! { #[table_model(database_default)] }.to_string()
    );
}

#[test]
fn a_mandatory_column_keeps_the_rust_value_of_its_default() {
    let schema = "
    CREATE TABLE grandparent (id INT PRIMARY KEY);
    CREATE TABLE parent (id INT PRIMARY KEY REFERENCES grandparent(id));
    CREATE TABLE sibling (id INT PRIMARY KEY, grandparent_id INT REFERENCES grandparent(id), UNIQUE(id, grandparent_id));
    CREATE TABLE child (
        id INT PRIMARY KEY REFERENCES parent(id),
        sibling_id INT DEFAULT 5 REFERENCES sibling(id),
        FOREIGN KEY (sibling_id, id) REFERENCES sibling(id, grandparent_id)
    );";
    assert_eq!(
        default_decorator(schema, "child", "sibling_id"),
        quote! { #[table_model(default = 5i32)] }.to_string()
    );
}

#[test]
fn defaulted_columns_are_left_to_the_database() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("defaults");

    let synql: SynQL<ParserDB> =
        SynQL::new_with_crate_base_path(&db, &workspace_path, Path::new("models"))
            .name("defaults")
            .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
            .generate_workspace_toml()
            .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/database_default_columns.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over columns with a SQL default, on SQLite.\n"),
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
        "The defaulted columns failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

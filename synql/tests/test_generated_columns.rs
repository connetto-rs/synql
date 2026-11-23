//! A stored generated column stays out of the builder, out of the `INSERT`
//! and out of the checks the builder runs, and the model reads back the
//! value the database computed.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test` against the
//! SQLite tables pg2sqlite emits for the schema.

use std::{path::Path, process::Command};

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TYPE grantee_kind AS ENUM ('user', 'link');
CREATE TABLE grantees (id UUID PRIMARY KEY, kind grantee_kind NOT NULL, UNIQUE (id, kind));
CREATE TABLE share_links (
    id UUID PRIMARY KEY,
    kind grantee_kind GENERATED ALWAYS AS ('link'::grantee_kind) STORED NOT NULL,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    FOREIGN KEY (id, kind) REFERENCES grantees (id, kind) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE prices (
    id UUID PRIMARY KEY,
    price INT NOT NULL,
    doubled INT GENERATED ALWAYS AS (price * 2) STORED NOT NULL CHECK (doubled > 0),
    CHECK (doubled >= price)
);
CREATE TABLE doublings (
    price INT NOT NULL,
    doubled INT GENERATED ALWAYS AS (price * 2) STORED PRIMARY KEY
);
";

const MIXED_KEY_SCHEMA: &str = "
CREATE TABLE codes (
    price INT NOT NULL,
    doubled INT GENERATED ALWAYS AS (price * 2) STORED,
    PRIMARY KEY (price, doubled)
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
demo-doublings.workspace = true
demo-grantees.workspace = true
demo-prices.workspace = true
demo-share_links.workspace = true
demo-public-grantee_kind.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over a generated column on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use demo_doublings::*;
use demo_grantees::*;
use demo_prices::*;
use demo_public_grantee_kind::GranteeKind;
use demo_share_links::*;

/// Opens an in-memory SQLite database holding the tables pg2sqlite emits.
fn connection() -> SqliteConnection {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    connection
        .batch_execute(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE grantees (
                id BLOB PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('user', 'link')),
                UNIQUE (id, kind)
            );
            CREATE TABLE share_links (
                id BLOB PRIMARY KEY,
                kind TEXT GENERATED ALWAYS AS ('link') STORED NOT NULL,
                token_hash BLOB NOT NULL UNIQUE CHECK (length(token_hash) = 32),
                FOREIGN KEY (id, kind) REFERENCES grantees (id, kind) DEFERRABLE INITIALLY DEFERRED
            );
            CREATE TABLE prices (
                id BLOB PRIMARY KEY,
                price INTEGER NOT NULL,
                doubled INTEGER GENERATED ALWAYS AS (price * 2) STORED NOT NULL CHECK (doubled > 0),
                CHECK (doubled >= price)
            );
            -- SQLite refuses a generated primary key, so a rowid key stands in for it.
            CREATE TABLE doublings (price INTEGER NOT NULL, doubled INTEGER PRIMARY KEY);",
        )
        .expect("create the tables pg2sqlite emits");
    connection
}

#[test]
fn the_builder_leaves_the_generated_column_to_the_database() {
    let mut connection = connection();
    let share_link = SetGranteeId::id(share_links::table::builder(), rosetta_uuid::Uuid::new_v4())
        .try_token_hash(vec![0; 32])
        .expect("a 32 byte hash")
        .insert(&mut connection)
        .expect("insert the grantee and the share link without writing either kind");
    assert_eq!(share_link.get_column_ref::<share_links::kind>(), &GranteeKind::Link);
}

#[test]
fn the_database_checks_the_generated_column() {
    let mut connection = connection();
    let price = prices::table::builder()
        .id(rosetta_uuid::Uuid::new_v4())
        .price(21)
        .insert(&mut connection)
        .expect("insert the price without writing or checking its doubled value");
    assert_eq!(price.get_column_ref::<prices::doubled>(), &42);
    prices::table::builder()
        .id(rosetta_uuid::Uuid::new_v4())
        .price(-1)
        .insert(&mut connection)
        .expect_err("SQLite refuses a negative doubled value");
}

#[test]
fn the_database_assigns_a_generated_key() {
    let mut connection = connection();
    let doubling = doublings::table::builder()
        .price(21)
        .insert(&mut connection)
        .expect("insert the doubling without writing its key");
    let loaded: Doubling = doublings::table
        .find(doubling.get_column_ref::<doublings::doubled>())
        .select(Doubling::as_select())
        .first(&mut connection)
        .expect("load the doubling by the key the database assigned");
    assert_eq!(loaded, doubling);
}
"#;

#[test]
fn generated_columns_stay_out_of_the_builder() {
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
        ("assertions/tests/generated_columns.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over a generated column on SQLite.\n"),
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
        "The generated column failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_generated_column_in_a_key_the_builder_sets_is_refused() {
    let db =
        ParserDB::parse::<PostgreSqlDialect>(MIXED_KEY_SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("demo");
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path).name("demo").into();
    let error = synql.generate().expect_err("A builder cannot set part of a key");
    assert!(
        matches!(
            &error,
            Error::GeneratedKeyColumn { table_name, column_name }
                if table_name == "codes" && column_name == "doubled"
        ),
        "{error}"
    );
}

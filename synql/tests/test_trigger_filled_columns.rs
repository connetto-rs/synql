//! A column a `BEFORE INSERT` trigger always overwrites stays out of the
//! builder and out of the `INSERT`, as a generated column does, while a
//! column the trigger reads keeps its setter, and a primary key the trigger
//! assigns in full is a surrogate key.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test` against SQLite
//! tables standing in for the PostgreSQL ones.

use std::{path::Path, process::Command};

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE nodes (
    id INT PRIMARY KEY,
    parent_id INT,
    name TEXT NOT NULL,
    parent_archived BOOLEAN NOT NULL DEFAULT FALSE,
    inherits_archive BOOLEAN NOT NULL DEFAULT FALSE,
    CHECK (inherits_archive = parent_archived)
);
CREATE FUNCTION copy_archive() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    SELECT parent.parent_archived INTO NEW.parent_archived FROM nodes parent WHERE parent.id = NEW.parent_id;
    NEW.parent_archived := coalesce(NEW.parent_archived, FALSE);
    NEW.inherits_archive := NEW.parent_archived;
    RETURN NEW;
END
$$;
CREATE TRIGGER copy_archive BEFORE INSERT ON nodes FOR EACH ROW EXECUTE FUNCTION copy_archive();
CREATE FUNCTION lower_name() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.name := lower(NEW.name);
    RETURN NEW;
END
$$;
CREATE TRIGGER lower_name BEFORE INSERT ON nodes FOR EACH ROW EXECUTE FUNCTION lower_name();
CREATE TABLE tickets (number INT PRIMARY KEY, title TEXT NOT NULL);
CREATE FUNCTION number_ticket() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.number := 7;
    RETURN NEW;
END
$$;
CREATE TRIGGER number_ticket BEFORE INSERT ON tickets FOR EACH ROW EXECUTE FUNCTION number_ticket();
";

const MIXED_KEY_SCHEMA: &str = "
CREATE TABLE codes (prefix TEXT, number INT, PRIMARY KEY (prefix, number));
CREATE FUNCTION number_code() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.number := 1;
    RETURN NEW;
END
$$;
CREATE TRIGGER number_code BEFORE INSERT ON codes FOR EACH ROW EXECUTE FUNCTION number_code();
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel = { workspace = true, features = ["sqlite", "returning_clauses_for_sqlite_3_35"] }
diesel-builders.workspace = true
demo-nodes.workspace = true
demo-tickets.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over columns a trigger fills, on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use demo_nodes::*;
use demo_tickets::*;

/// Opens an in-memory SQLite database whose tables stand in for the
/// PostgreSQL ones.
///
/// A SQLite trigger cannot change the row before it is written, so a column
/// default stands in for each value a PostgreSQL trigger fills. The defaults
/// differ from the PostgreSQL `DEFAULT FALSE` a builder would copy, and a row
/// writing anything else is refused, as PostgreSQL refuses a column the
/// inserting role may not write.
fn connection() -> SqliteConnection {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    connection
        .batch_execute(
            "CREATE TABLE nodes (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER,
                name TEXT NOT NULL,
                parent_archived INTEGER NOT NULL DEFAULT 1,
                inherits_archive INTEGER NOT NULL DEFAULT 1,
                CHECK (inherits_archive = parent_archived)
            );
            CREATE TRIGGER refuse_filled_columns BEFORE INSERT ON nodes
            WHEN NEW.parent_archived IS NOT 1 OR NEW.inherits_archive IS NOT 1
            BEGIN SELECT RAISE(ABORT, 'permission denied for table nodes'); END;
            CREATE TABLE tickets (number INTEGER PRIMARY KEY, title TEXT NOT NULL);",
        )
        .expect("create the stand-in tables");
    connection
}

#[test]
fn the_builder_leaves_trigger_filled_columns_to_the_database() {
    let mut connection = connection();
    let node = nodes::table::builder()
        .id(1)
        .name("Ada".to_owned())
        .insert(&mut connection)
        .expect("insert the node without writing either archive flag");
    assert_eq!(node.get_column_ref::<nodes::parent_archived>(), &true);
    assert_eq!(node.get_column_ref::<nodes::inherits_archive>(), &true);
    assert_eq!(node.get_column_ref::<nodes::name>(), "Ada");
}

#[test]
fn the_database_assigns_a_trigger_filled_key() {
    let mut connection = connection();
    let ticket = tickets::table::builder()
        .title("Broken build".to_owned())
        .insert(&mut connection)
        .expect("insert the ticket without writing its number");
    let loaded: Ticket = tickets::table
        .find(ticket.get_column_ref::<tickets::number>())
        .select(Ticket::as_select())
        .first(&mut connection)
        .expect("load the ticket by the number the database assigned");
    assert_eq!(loaded, ticket);
}
"#;

#[test]
fn trigger_filled_columns_stay_out_of_the_builder() {
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
        ("assertions/tests/trigger_filled_columns.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over columns a trigger fills, on SQLite.\n"),
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
        "The trigger-filled columns failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_trigger_filled_column_in_a_key_the_builder_sets_is_refused() {
    let db =
        ParserDB::parse::<PostgreSqlDialect>(MIXED_KEY_SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("demo");
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path).name("demo").into();
    let error = synql.generate().expect_err("A builder cannot set part of a key");
    assert!(
        matches!(
            &error,
            Error::FilledKeyColumn { table_name, column_name }
                if table_name == "codes" && column_name == "number"
        ),
        "{error}"
    );
}

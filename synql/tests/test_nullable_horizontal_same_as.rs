//! `NOT NULL` columns keyed to a nullable column of a triangle's satellite
//! insert through their builder, whether the key is wider than two columns or
//! not.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test` on SQLite.

use std::{path::Path, process::Command};

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE templates (id INT PRIMARY KEY);
CREATE TABLE slots (
    id INT PRIMARY KEY,
    template_id INT NOT NULL REFERENCES templates (id),
    placed_into_slot_id INT,
    UNIQUE (id, template_id),
    UNIQUE (id, placed_into_slot_id),
    UNIQUE (id, template_id, placed_into_slot_id),
    FOREIGN KEY (placed_into_slot_id, template_id) REFERENCES slots (id, template_id)
);
CREATE TABLE storages (
    id INT PRIMARY KEY REFERENCES templates (id),
    stored_into_slot_id INT NOT NULL,
    stored_asset_slot_id INT NOT NULL,
    FOREIGN KEY (stored_into_slot_id, id) REFERENCES slots (id, template_id),
    FOREIGN KEY (stored_asset_slot_id, id) REFERENCES slots (id, template_id),
    FOREIGN KEY (stored_asset_slot_id, id, stored_into_slot_id)
        REFERENCES slots (id, template_id, placed_into_slot_id)
);
CREATE TABLE retrievals (
    id INT PRIMARY KEY REFERENCES templates (id),
    retrieved_from_slot_id INT NOT NULL,
    retrieved_asset_slot_id INT NOT NULL,
    FOREIGN KEY (retrieved_from_slot_id, id) REFERENCES slots (id, template_id),
    FOREIGN KEY (retrieved_asset_slot_id, id) REFERENCES slots (id, template_id),
    FOREIGN KEY (retrieved_asset_slot_id, retrieved_from_slot_id)
        REFERENCES slots (id, placed_into_slot_id)
);
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel = { workspace = true, features = ["sqlite", "returning_clauses_for_sqlite_3_35"] }
diesel-builders.workspace = true
demo-retrievals.workspace = true
demo-slots.workspace = true
demo-storages.workspace = true
demo-templates.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over slots linked through a nullable column on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use demo_retrievals::*;
use demo_slots::*;
use demo_storages::*;
use demo_templates::*;

/// Opens an in-memory SQLite database holding the schema.
fn connection() -> SqliteConnection {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    connection
        .batch_execute(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE templates (id INTEGER PRIMARY KEY);
            CREATE TABLE slots (
                id INTEGER PRIMARY KEY,
                template_id INTEGER NOT NULL REFERENCES templates (id),
                placed_into_slot_id INTEGER,
                UNIQUE (id, template_id),
                UNIQUE (id, placed_into_slot_id),
                UNIQUE (id, template_id, placed_into_slot_id),
                FOREIGN KEY (placed_into_slot_id, template_id) REFERENCES slots (id, template_id)
            );
            CREATE TABLE storages (
                id INTEGER PRIMARY KEY REFERENCES templates (id),
                stored_into_slot_id INTEGER NOT NULL,
                stored_asset_slot_id INTEGER NOT NULL,
                FOREIGN KEY (stored_into_slot_id, id) REFERENCES slots (id, template_id),
                FOREIGN KEY (stored_asset_slot_id, id) REFERENCES slots (id, template_id),
                FOREIGN KEY (stored_asset_slot_id, id, stored_into_slot_id)
                    REFERENCES slots (id, template_id, placed_into_slot_id)
            );
            CREATE TABLE retrievals (
                id INTEGER PRIMARY KEY REFERENCES templates (id),
                retrieved_from_slot_id INTEGER NOT NULL,
                retrieved_asset_slot_id INTEGER NOT NULL,
                FOREIGN KEY (retrieved_from_slot_id, id) REFERENCES slots (id, template_id),
                FOREIGN KEY (retrieved_asset_slot_id, id) REFERENCES slots (id, template_id),
                FOREIGN KEY (retrieved_asset_slot_id, retrieved_from_slot_id)
                    REFERENCES slots (id, placed_into_slot_id)
            );",
        )
        .expect("create the tables");
    connection
}

#[test]
fn the_storage_inserts_into_the_slot_its_asset_is_placed_into() {
    let mut connection = connection();
    let storage = SetTemplateId::id(storages::table::builder(), 1)
        .stored_into_slot(slots::table::builder().id(10))
        .stored_asset_slot(slots::table::builder().id(11).placed_into_slot_id(Some(10)))
        .insert(&mut connection)
        .expect("insert the storage");
    let asset: Slot = storage.stored_asset_slot(&mut connection).expect("load the asset slot");
    assert_eq!(asset.get_column_ref::<slots::placed_into_slot_id>(), &Some(10));
    assert_eq!(storage.get_column_ref::<storages::stored_into_slot_id>(), &10);
}

#[test]
fn the_retrieval_keeps_its_two_column_key_to_the_slot_its_asset_is_placed_into() {
    let mut connection = connection();
    let retrieval = SetTemplateId::id(retrievals::table::builder(), 2)
        .retrieved_from_slot(slots::table::builder().id(20))
        .retrieved_asset_slot(slots::table::builder().id(21).placed_into_slot_id(Some(20)))
        .insert(&mut connection)
        .expect("insert the retrieval");
    let keys: Vec<_> =
        retrieval.iter_match_full::<(slots::id, slots::placed_into_slot_id)>().collect();
    assert_eq!(keys, [(&21, (&20,))]);
}
"#;

#[test]
fn not_null_columns_keyed_to_a_nullable_satellite_column_insert() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("demo");

    let synql: SynQL<ParserDB> =
        SynQL::new_with_crate_base_path(&db, &workspace_path, Path::new("models"))
            .name("demo")
            .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
            .generate_workspace_toml()
            .one_crate_per_table()
            .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/nullable_satellite_column.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over slots linked through a nullable column.\n"),
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
        "The nullable slot link failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

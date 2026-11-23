//! A table four extensions below the root, whose every level keys the same
//! column into the root, inserts through its builder with that column set
//! once, and every ancestor stores the value.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test` on SQLite.

use std::{path::Path, process::Command};

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE resources (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    UNIQUE (organization_id, id)
);
CREATE TABLE ownables (
    id UUID PRIMARY KEY REFERENCES resources (id),
    organization_id UUID NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE asset_models (
    id UUID PRIMARY KEY REFERENCES ownables (id),
    organization_id UUID NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE physical_asset_models (
    id UUID PRIMARY KEY REFERENCES asset_models (id),
    organization_id UUID NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE container_models (
    id UUID PRIMARY KEY REFERENCES physical_asset_models (id),
    organization_id UUID NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
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
demo-resources.workspace = true
demo-ownables.workspace = true
demo-asset_models.workspace = true
demo-physical_asset_models.workspace = true
demo-container_models.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over a column shared down a chain of extensions on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use demo_asset_models::*;
use demo_container_models::*;
use demo_ownables::*;
use demo_physical_asset_models::*;
use demo_resources::*;

#[test]
fn every_ancestor_of_a_container_model_stores_its_organization() {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    connection
        .batch_execute(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE resources (
                id BLOB PRIMARY KEY,
                organization_id BLOB NOT NULL,
                UNIQUE (organization_id, id)
            );
            CREATE TABLE ownables (
                id BLOB PRIMARY KEY REFERENCES resources (id),
                organization_id BLOB NOT NULL,
                FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
            );
            CREATE TABLE asset_models (
                id BLOB PRIMARY KEY REFERENCES ownables (id),
                organization_id BLOB NOT NULL,
                FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
            );
            CREATE TABLE physical_asset_models (
                id BLOB PRIMARY KEY REFERENCES asset_models (id),
                organization_id BLOB NOT NULL,
                FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
            );
            CREATE TABLE container_models (
                id BLOB PRIMARY KEY REFERENCES physical_asset_models (id),
                organization_id BLOB NOT NULL,
                FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
            );",
        )
        .expect("create the tables");
    let id = rosetta_uuid::Uuid::new_v4();
    let organization_id = rosetta_uuid::Uuid::new_v4();
    let builder = SetResourceId::id(container_models::table::builder(), id);
    let container_model = SetContainerModelOrganizationId::organization_id(builder, organization_id)
        .insert(&mut connection)
        .expect("insert the container model");
    let physical_asset_model: PhysicalAssetModel =
        container_model.ancestor(&mut connection).expect("load the physical asset model");
    let asset_model: AssetModel =
        container_model.ancestor(&mut connection).expect("load the asset model");
    let ownable: Ownable = container_model.ancestor(&mut connection).expect("load the ownable");
    let resource: Resource = container_model.ancestor(&mut connection).expect("load the resource");
    assert_eq!(
        container_model.get_column_ref::<container_models::organization_id>(),
        &organization_id
    );
    assert_eq!(
        physical_asset_model.get_column_ref::<physical_asset_models::organization_id>(),
        &organization_id
    );
    assert_eq!(asset_model.get_column_ref::<asset_models::organization_id>(), &organization_id);
    assert_eq!(ownable.get_column_ref::<ownables::organization_id>(), &organization_id);
    assert_eq!(resource.get_column_ref::<resources::organization_id>(), &organization_id);
}
"#;

#[test]
fn a_column_shared_down_a_chain_of_extensions_inserts() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("demo");

    let synql: SynQL<ParserDB> =
        SynQL::new_with_crate_base_path(&db, &workspace_path, Path::new("models"))
            .name("demo")
            .one_crate_per_table()
            .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
            .generate_workspace_toml()
            .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/vertical_same_as_chain.rs", ASSERTIONS_TEST),
        (
            "assertions/src/lib.rs",
            "//! Assertions over a column shared down a chain of extensions.\n",
        ),
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
        "The shared column failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

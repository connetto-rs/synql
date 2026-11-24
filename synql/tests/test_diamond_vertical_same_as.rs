//! A table extending two tables that share an ancestor declares its column
//! equal to the matching column of both parents, and inserts through its
//! builder at the lowest `diesel-builders` and `validation-errors` the
//! generated manifest permits.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, locks both crates at the version the workspace manifest
//! requires, and runs the assertions with `cargo test` on SQLite. The check
//! on the team name makes the workspace depend on `validation-errors`.

#[expect(dead_code, reason = "this test lists no crate directories")]
mod common;

use std::path::Path;

use common::run;
use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE resources (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    UNIQUE (organization_id, id)
);
CREATE TABLE owners (
    id UUID PRIMARY KEY REFERENCES resources (id),
    organization_id UUID NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE ownables (
    id UUID PRIMARY KEY REFERENCES resources (id),
    organization_id UUID NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE teams (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    name TEXT NOT NULL CHECK (name <> ''),
    FOREIGN KEY (id) REFERENCES owners (id),
    FOREIGN KEY (id) REFERENCES ownables (id),
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
demo-owners.workspace = true
demo-ownables.workspace = true
demo-teams.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over a column shared by both parents of a diamond on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use demo_ownables::*;
use demo_owners::*;
use demo_resources::*;
use demo_teams::*;

#[test]
fn every_ancestor_of_a_team_stores_its_organization() {
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
            CREATE TABLE owners (
                id BLOB PRIMARY KEY REFERENCES resources (id),
                organization_id BLOB NOT NULL,
                FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
            );
            CREATE TABLE ownables (
                id BLOB PRIMARY KEY REFERENCES resources (id),
                organization_id BLOB NOT NULL,
                FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
            );
            CREATE TABLE teams (
                id BLOB PRIMARY KEY,
                organization_id BLOB NOT NULL,
                name TEXT NOT NULL CHECK (name <> ''),
                FOREIGN KEY (id) REFERENCES owners (id),
                FOREIGN KEY (id) REFERENCES ownables (id),
                FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
            );",
        )
        .expect("create the tables");
    let id = rosetta_uuid::Uuid::new_v4();
    let organization_id = rosetta_uuid::Uuid::new_v4();
    let builder = SetResourceId::id(teams::table::builder(), id);
    let team = SetTeamOrganizationId::organization_id(builder, organization_id)
        .try_name("Platform".to_owned())
        .expect("a non-empty name")
        .insert(&mut connection)
        .expect("insert the team");
    let owner: Owner = team.ancestor(&mut connection).expect("load the owner");
    let ownable: Ownable = team.ancestor(&mut connection).expect("load the ownable");
    let resource: Resource = team.ancestor(&mut connection).expect("load the resource");
    assert_eq!(team.get_column_ref::<teams::organization_id>(), &organization_id);
    assert_eq!(owner.get_column_ref::<owners::organization_id>(), &organization_id);
    assert_eq!(ownable.get_column_ref::<ownables::organization_id>(), &organization_id);
    assert_eq!(resource.get_column_ref::<resources::organization_id>(), &organization_id);
}
"#;

/// Returns the version the workspace manifest at `workspace_path` requires
/// of `dependency`.
fn required_version(workspace_path: &Path, dependency: &str) -> String {
    let manifest: toml::Table = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest")
        .parse()
        .expect("Unable to parse the workspace manifest");
    manifest["workspace"]["dependencies"]
        .get(dependency)
        .and_then(|declaration| declaration.get("version"))
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("The workspace manifest requires no version of {dependency}"))
        .to_owned()
}

#[test]
fn a_column_shared_by_both_parents_of_a_diamond_inserts_at_the_required_versions() {
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
        ("assertions/tests/diamond_vertical_same_as.rs", ASSERTIONS_TEST),
        (
            "assertions/src/lib.rs",
            "//! Assertions over a column shared by both parents of a diamond.\n",
        ),
    ] {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    run("cargo", &["generate-lockfile"], &workspace_path);
    // `validation-errors` depends on `diesel-builders`, so it steps down first.
    for dependency in ["validation-errors", "diesel-builders"] {
        let version = required_version(&workspace_path, dependency);
        run("cargo", &["update", "-p", dependency, "--precise", &version], &workspace_path);
    }
    run("cargo", &["test", "-p", "assertions"], &workspace_path);
}

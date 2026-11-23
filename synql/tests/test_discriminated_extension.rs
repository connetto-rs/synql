//! A subtype keyed to its supertype through the primary key and a generated
//! discriminator writes the discriminator into the supertype's row, so a
//! nested builder of any descendant inserts the whole hierarchy without its
//! caller naming the discriminator.
//!
//! The test generates a workspace with one crate per table, so the leaf's
//! crate writes a discriminator whose type lives in neither its crate nor its
//! parent's, adds a crate of assertions over the generated code, and runs the
//! assertions with `cargo test` against the SQLite tables pg2sqlite emits for
//! the schema.

use std::{path::Path, process::Command};

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TYPE resource_kind AS ENUM ('folder', 'ownable');
CREATE TABLE resources (id UUID PRIMARY KEY, kind resource_kind NOT NULL, UNIQUE (id, kind));
CREATE TABLE ownables (
    id UUID PRIMARY KEY,
    kind resource_kind GENERATED ALWAYS AS ('ownable'::resource_kind) STORED NOT NULL,
    FOREIGN KEY (id, kind) REFERENCES resources (id, kind) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE asset_models (
    id UUID PRIMARY KEY REFERENCES ownables (id) ON DELETE CASCADE,
    name TEXT NOT NULL
);
CREATE TABLE shapes (id UUID PRIMARY KEY, sides SMALLINT NOT NULL, UNIQUE (id, sides));
CREATE TABLE triangles (
    id UUID PRIMARY KEY,
    sides SMALLINT GENERATED ALWAYS AS (3) STORED NOT NULL,
    FOREIGN KEY (id, sides) REFERENCES shapes (id, sides)
);
";

const UNTRANSLATABLE_SCHEMA: &str = "
CREATE TABLE tokens (id INT PRIMARY KEY, issuer UUID NOT NULL, UNIQUE (id, issuer));
CREATE TABLE root_tokens (
    id INT PRIMARY KEY,
    issuer UUID GENERATED ALWAYS AS ('00000000-0000-0000-0000-000000000000'::uuid) STORED,
    FOREIGN KEY (id, issuer) REFERENCES tokens (id, issuer)
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
demo-asset_models.workspace = true
demo-resources.workspace = true
demo-shapes.workspace = true
demo-triangles.workspace = true
demo-public-resource_kind.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over discriminated extensions on SQLite.

use diesel::{connection::SimpleConnection, prelude::*};
use diesel_builders::prelude::*;
use demo_asset_models::*;
use demo_public_resource_kind::ResourceKind;
use demo_resources::*;
use demo_shapes::*;
use demo_triangles::*;

/// Opens an in-memory SQLite database holding the tables pg2sqlite emits.
fn connection() -> SqliteConnection {
    let mut connection = SqliteConnection::establish(":memory:").expect("open SQLite");
    // The typed DSL has no form for DDL.
    connection
        .batch_execute(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE resources (
                id BLOB PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('folder', 'ownable')),
                UNIQUE (id, kind)
            );
            CREATE TABLE ownables (
                id BLOB PRIMARY KEY,
                kind TEXT GENERATED ALWAYS AS ('ownable') STORED NOT NULL,
                FOREIGN KEY (id, kind) REFERENCES resources (id, kind) DEFERRABLE INITIALLY DEFERRED
            );
            CREATE TABLE asset_models (
                id BLOB PRIMARY KEY REFERENCES ownables (id) ON DELETE CASCADE,
                name TEXT NOT NULL
            );
            CREATE TABLE shapes (id BLOB PRIMARY KEY, sides INTEGER NOT NULL, UNIQUE (id, sides));
            CREATE TABLE triangles (
                id BLOB PRIMARY KEY,
                sides INTEGER GENERATED ALWAYS AS (3) STORED NOT NULL,
                FOREIGN KEY (id, sides) REFERENCES shapes (id, sides)
            );",
        )
        .expect("create the tables pg2sqlite emits");
    connection
}

#[test]
fn a_leaf_writes_the_discriminator_of_its_grandparent() {
    let mut connection = connection();
    let id = rosetta_uuid::Uuid::new_v4();
    // Each table of the hierarchy has an `id` setter, and the root's fills them all.
    SetResourceId::id(asset_models::table::builder(), id)
        .name("Pizza oven".to_owned())
        .insert(&mut connection)
        .expect("insert the resource, the ownable and the asset model without naming a kind");
    let resource: Resource = resources::table
        .find(id)
        .select(Resource::as_select())
        .first(&mut connection)
        .expect("load the resource the asset model inserted");
    assert_eq!(resource.get_column_ref::<resources::kind>(), &ResourceKind::Ownable);
}

#[test]
fn a_scalar_discriminator_is_written_as_its_literal() {
    let mut connection = connection();
    let id = rosetta_uuid::Uuid::new_v4();
    SetShapeId::id(triangles::table::builder(), id)
        .insert(&mut connection)
        .expect("insert the shape and the triangle without naming the sides");
    let shape: Shape = shapes::table
        .find(id)
        .select(Shape::as_select())
        .first(&mut connection)
        .expect("load the shape the triangle inserted");
    assert_eq!(shape.get_column_ref::<shapes::sides>(), &3);
}
"#;

#[test]
fn extensions_write_the_discriminators_of_their_ancestors() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("demo");

    let synql: SynQL<ParserDB> =
        SynQL::new_with_crate_base_path(&db, &workspace_path, Path::new("models"))
            .name("demo")
            .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
            .one_crate_per_table()
            .generate_workspace_toml()
            .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/discriminated_extension.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over discriminated extensions on SQLite.\n"),
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
        "The discriminated extensions failed on SQLite:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_discriminator_without_a_rust_value_is_refused() {
    let db = ParserDB::parse::<PostgreSqlDialect>(UNTRANSLATABLE_SCHEMA)
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("demo");
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path).name("demo").into();
    let error = synql.generate().expect_err("No builder can write the issuer");
    assert!(
        matches!(
            &error,
            Error::UnsupportedDiscriminator { foreign_key, column_name, constant }
                if foreign_key == "root_tokens (id, issuer) REFERENCES tokens (id, issuer)"
                    && column_name == "issuer"
                    && constant == "'00000000-0000-0000-0000-000000000000'::UUID"
        ),
        "{error}"
    );
}

//! The generated models insert through any diesel connection to PostgreSQL,
//! including those whose columns map to rosetta-uuid and rosetta-utc.
//!
//! The test generates a workspace, adds a crate inserting a builder through a
//! connection generic over the PostgreSQL backend, and checks it with
//! `cargo check`, which needs no libpq.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE probes (
    id UUID PRIMARY KEY,
    received_at TIMESTAMPTZ NOT NULL
);
";

const INSERTER_MANIFEST: &str = r#"[package]
name = "inserter"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel = { workspace = true, features = ["postgres_backend"] }
diesel-builders.workspace = true
synql-postgres-probes.workspace = true
"#;

const INSERTER_LIB: &str = r"//! Inserts the generated builder through a PostgreSQL connection.

use diesel::{connection::LoadConnection, pg::Pg};
use diesel_builders::prelude::*;
use synql_postgres_probes::probes;

/// Requires `Builder` to insert through `Conn`.
fn inserts_through<Builder: Insert<Conn>, Conn>() {}

/// Requires the builder of `probes` to insert through every PostgreSQL
/// connection.
pub fn probes_insert_through_postgres<Conn: LoadConnection<Backend = Pg>>() {
    inserts_through::<TableBuilder<probes::table>, Conn>();
}
";

#[test]
fn rosetta_columns_insert_through_postgres() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_postgres");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-postgres")
        .member(TomlDependency::new("inserter").path("inserter").expect("A valid path"))
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in
        [("inserter/Cargo.toml", INSERTER_MANIFEST), ("inserter/src/lib.rs", INSERTER_LIB)]
    {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    let output = Command::new("cargo")
        .args(["check", "-p", "inserter"])
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo check");
    assert!(
        output.status.success(),
        "The generated builder does not insert through PostgreSQL:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

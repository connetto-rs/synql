//! The generated `[workspace.dependencies]` declares a built-in crate only
//! when a generated crate depends on it, while a crate registered with
//! `SynQLBuilder::external_crate` stays declared for hand-written members.

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

/// Generates the workspace of `schema` with an unused registered crate and
/// returns the names `[workspace.dependencies]` declares.
fn workspace_dependencies(schema: &str) -> Vec<String> {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_workspace_dependencies");
    let registered = ExternalCrate::new("registered")
        .expect("A valid crate name")
        .version("1.0")
        .expect("A valid version")
        .into();
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-workspace-dependencies")
        .external_crate(registered)
        .generate_workspace_toml()
        .sink_crate("sink")
        .into();
    synql.generate().expect("Unable to generate workspace");

    let manifest = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    manifest
        .lines()
        .skip_while(|line| *line != "[workspace.dependencies]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split([' ', '.', '=']).next())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn unused_built_in_crates_are_not_declared() {
    let dependencies = workspace_dependencies(
        "CREATE TABLE notes (id INT PRIMARY KEY, body TEXT NOT NULL CHECK (body <> ''));",
    );
    for unused in ["sqlitegis", "rosetta-time", "rosetta-utc", "rosetta-uuid", "serde_json"] {
        assert!(
            !dependencies.iter().any(|name| name == unused),
            "`{unused}` is declared but unused: {dependencies:?}"
        );
    }
    for used in ["serde", "diesel", "diesel-builders", "validation-errors", "registered"] {
        assert!(
            dependencies.iter().any(|name| name == used),
            "`{used}` is not declared: {dependencies:?}"
        );
    }
}

#[test]
fn built_in_crates_of_column_types_are_declared() {
    let dependencies = workspace_dependencies(
        "CREATE TABLE visits (id UUID PRIMARY KEY, day DATE NOT NULL, at TIMESTAMPTZ NOT NULL);",
    );
    for used in ["rosetta-time", "rosetta-utc", "rosetta-uuid"] {
        assert!(
            dependencies.iter().any(|name| name == used),
            "`{used}` is not declared: {dependencies:?}"
        );
    }
    assert!(
        !dependencies.iter().any(|name| name == "sqlitegis"),
        "`sqlitegis` is declared but unused: {dependencies:?}"
    );
}

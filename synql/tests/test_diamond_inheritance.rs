//! Test to verify that a table extending two spouses, one referencing the
//! other, generates a workspace whose nested ancestor join compiles.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

#[test]
fn test_diamond_inheritance() {
    let db = ParserDB::parse::<GenericDialect>(
        "
    CREATE TABLE entities (
        id INT PRIMARY KEY,
        label TEXT NOT NULL
    );
    CREATE TABLE owners (
        id INT PRIMARY KEY REFERENCES entities(id)
    );
    CREATE TABLE ownables (
        id INT PRIMARY KEY REFERENCES entities(id),
        owner_id INT NOT NULL REFERENCES owners(id)
    );
    CREATE TABLE teams (
        id INT PRIMARY KEY,
        name TEXT NOT NULL,
        FOREIGN KEY (id) REFERENCES owners(id),
        FOREIGN KEY (id) REFERENCES ownables(id)
    );
",
    )
    .expect("Unable to parse the schema");

    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_diamond");

    let synql: SynQL<ParserDB> =
        SynQL::new(&db, &workspace_path).name("synql-diamond").generate_workspace_toml().into();
    synql.generate().expect("Unable to generate workspace");

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo check");
    assert!(
        output.status.success(),
        "cargo check failed for the generated workspace:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

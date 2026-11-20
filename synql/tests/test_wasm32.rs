//! The generated workspace builds for `wasm32-unknown-unknown`, which browser
//! clients compile the models to, including its enum crates, whose diesel
//! enables `postgres_backend`.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TYPE message_kind AS ENUM ('text', 'photo');
CREATE TABLE probes (
    id UUID PRIMARY KEY,
    kind message_kind NOT NULL,
    payload JSONB NOT NULL,
    opens_at TIME NOT NULL,
    received_at TIMESTAMPTZ,
    sent_on DATE,
    delay INTERVAL
);
";

#[test]
fn test_wasm32() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_wasm32");

    let synql: SynQL<ParserDB> =
        SynQL::new(&db, &workspace_path).name("synql-wasm32").generate_workspace_toml().into();
    synql.generate().expect("Unable to generate workspace");

    let output = Command::new("cargo")
        .args(["check", "--target", "wasm32-unknown-unknown"])
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo check");
    assert!(
        output.status.success(),
        "cargo check failed for the generated workspace on wasm32:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

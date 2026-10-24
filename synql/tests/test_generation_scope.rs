//! Tests for which tables and members end up in a generated workspace.

use sql_traits::prelude::*;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

const SCHEMA: &str = "
    CREATE TABLE users (id INT PRIMARY KEY, name TEXT NOT NULL);
    CREATE TABLE comments (id INT PRIMARY KEY, user_id INT NOT NULL REFERENCES users(id));
    CREATE TABLE isolated (id INT PRIMARY KEY, data TEXT);
";

#[test]
fn denied_tables_and_their_dependents_are_not_generated() {
    let db = ParserDB::parse::<GenericDialect>(SCHEMA).expect("Unable to parse the schema");
    let users = db
        .table_by_target(TargetName::new("users", false), IdentifierCase::AsWritten)
        .expect("Unable to look up users")
        .expect("users exists");

    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_deny");
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-deny")
        .deny(users)
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    assert!(!workspace_path.join("synql-deny-users").exists());
    assert!(!workspace_path.join("synql-deny-comments").exists());
    assert!(workspace_path.join("synql-deny-isolated").exists());

    let workspace_toml = std::fs::read_to_string(workspace_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    assert!(workspace_toml.contains("synql-deny-isolated"));
    assert!(!workspace_toml.contains("synql-deny-users"));
    assert!(!workspace_toml.contains("synql-deny-comments"));
}

#[test]
fn a_workspace_member_without_a_path_is_rejected() {
    let db = ParserDB::parse::<GenericDialect>(SCHEMA).expect("Unable to parse the schema");

    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_member");
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-member")
        .member(TomlDependency::new("pathless"))
        .generate_workspace_toml()
        .into();

    let error = synql.generate().expect_err("A path-less member must be rejected");
    assert!(
        matches!(&error, synql::Error::WorkspaceMemberWithoutPath(name) if name == "pathless"),
        "Unexpected error: {error}"
    );
}

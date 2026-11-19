//! Test module verifying how tables are grouped into crates, and that tables
//! grouped into shared crates generate a workspace which compiles and
//! documents.

use std::{path::Path, process::Command};

use quote::quote;
use sql_traits::prelude::*;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE user_states (
    id SERIAL PRIMARY KEY,
    label TEXT NOT NULL
);
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    state_id INT NOT NULL REFERENCES user_states(id),
    name TEXT NOT NULL CHECK (name <> '')
);
CREATE TABLE posts (
    id SERIAL PRIMARY KEY,
    author_id INT NOT NULL REFERENCES users(id),
    body TEXT NOT NULL
);
CREATE TABLE comments (
    id SERIAL PRIMARY KEY,
    post_id INT NOT NULL REFERENCES posts(id),
    author_id INT NOT NULL REFERENCES users(id),
    body TEXT NOT NULL
);
CREATE TABLE pinned_comments (
    id INT PRIMARY KEY REFERENCES comments(id),
    pinned_by INT NOT NULL REFERENCES users(id)
);
CREATE TABLE likes (
    id SERIAL PRIMARY KEY,
    post_id INT NOT NULL REFERENCES posts(id),
    user_id INT NOT NULL REFERENCES users(id)
);
CREATE TABLE projects (
    id SERIAL PRIMARY KEY,
    owner_id INT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL
);
CREATE TABLE milestones (
    id SERIAL PRIMARY KEY,
    project_id INT NOT NULL REFERENCES projects(id),
    name TEXT NOT NULL
);
CREATE TABLE tasks (
    id SERIAL PRIMARY KEY,
    project_id INT NOT NULL REFERENCES projects(id),
    milestone_id INT NOT NULL REFERENCES milestones(id),
    title TEXT NOT NULL CHECK (title <> '')
);
CREATE TABLE task_assignments (
    id SERIAL PRIMARY KEY,
    task_id INT NOT NULL REFERENCES tasks(id),
    milestone_id INT NOT NULL REFERENCES milestones(id),
    project_id INT NOT NULL REFERENCES projects(id)
);
CREATE TABLE settings (
    id SERIAL PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE \"UserBadges\" (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id)
);
CREATE TABLE archived_settings (
    id SERIAL PRIMARY KEY,
    setting_id INT NOT NULL REFERENCES settings(id)
);
";

/// Runs `cargo` with `arguments` in `workspace_path`, panicking with its
/// output when it fails.
fn run_cargo(workspace_path: &Path, arguments: &[&str]) {
    let output = Command::new("cargo")
        .args(arguments)
        .current_dir(workspace_path)
        .output()
        .expect("Unable to run cargo");
    assert!(
        output.status.success(),
        "cargo {arguments:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Returns the sorted names of the crates of the workspace at
/// `workspace_path` whose directory starts with `prefix`, without the prefix.
fn crate_names(workspace_path: &Path, prefix: &str) -> Vec<String> {
    let mut crates: Vec<String> = std::fs::read_dir(workspace_path)
        .expect("Unable to list the workspace")
        .map(|entry| entry.expect("Unable to read a workspace entry").file_name())
        .filter_map(|name| name.to_str()?.strip_prefix(prefix).map(str::to_owned))
        .collect();
    crates.sort_unstable();
    crates
}

#[test]
fn test_tables_grouped_by_default() {
    let db = ParserDB::parse::<GenericDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("default_workspace");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path).name("default").into();
    synql.generate().expect("Unable to generate workspace");

    assert_eq!(crate_names(&workspace_path, "default-"), ["projects", "settings", "users"]);
}

#[test]
fn test_one_crate_per_table() {
    let db = ParserDB::parse::<GenericDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("ungrouped_workspace");

    let synql: SynQL<ParserDB> =
        SynQL::new(&db, &workspace_path).name("ungrouped").one_crate_per_table().into();
    synql.generate().expect("Unable to generate workspace");

    assert_eq!(
        crate_names(&workspace_path, "ungrouped-"),
        [
            "archived_settings",
            "comments",
            "likes",
            "milestones",
            "pinned_comments",
            "posts",
            "projects",
            "settings",
            "task_assignments",
            "tasks",
            "user_badges",
            "user_states",
            "users",
        ]
    );
}

#[test]
fn test_grouped_tables_generation() {
    let db = ParserDB::parse::<GenericDialect>(SCHEMA).expect("Unable to parse the schema");
    let archived_settings = db
        .table_by_target(TargetName::new("archived_settings", false), IdentifierCase::AsWritten)
        .expect("Unable to look up archived_settings")
        .expect("archived_settings exists");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("grouped_workspace");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("grouped")
        .generate_workspace_toml()
        .sink_crate("sink")
        .dag_sink_crate_prefix("sink_")
        .deny(archived_settings)
        .toml_callback(|_table, _db| Ok(Some(TomlDependency::new("serde_json").version("1.0")?)))
        .sink_callback(|_tables, _db, _workspace| {
            Ok(Some(quote! {
                pub use grouped_users::user_states_model::SetUserStateId as SetUserStatesId;
                pub use grouped_users::users_model::SetUserStateId as SetUsersStateId;
            }))
        })
        .into();
    synql.generate().expect("Unable to generate workspace");

    assert_eq!(crate_names(&workspace_path, "grouped-"), ["projects", "settings", "users"]);

    let users_lib = std::fs::read_to_string(workspace_path.join("grouped-users/src/lib.rs"))
        .expect("Unable to read the users crate");
    assert!(users_lib.contains("extern crate self as grouped_users"), "{users_lib}");
    assert!(users_lib.contains("pub struct PinnedComment"), "{users_lib}");
    assert!(users_lib.contains("# ! [allow (non_snake_case)]"), "{users_lib}");

    let users_toml = std::fs::read_to_string(workspace_path.join("grouped-users/Cargo.toml"))
        .expect("Unable to read the users manifest");
    assert_eq!(users_toml.matches("serde_json = ").count(), 1, "{users_toml}");

    let projects_toml = std::fs::read_to_string(workspace_path.join("grouped-projects/Cargo.toml"))
        .expect("Unable to read the projects manifest");
    assert_eq!(projects_toml.matches("grouped-users.workspace = true").count(), 1);
    assert!(!projects_toml.contains("grouped-projects.workspace"), "{projects_toml}");

    run_cargo(&workspace_path, &["check"]);
    run_cargo(&workspace_path, &["doc", "--no-deps", "--document-private-items"]);
}

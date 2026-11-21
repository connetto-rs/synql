//! Tests for generating a schema on top of an already generated base
//! workspace, which describes itself in its description crate.

use std::{path::Path, process::Command, str::FromStr};

use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const BASE_SQL: &str = "
CREATE TYPE mood AS ENUM ('happy', 'sad');
CREATE TABLE users (id UUID PRIMARY KEY, name TEXT NOT NULL, mood mood NOT NULL);
CREATE TABLE resources (id UUID PRIMARY KEY, owner_id UUID NOT NULL REFERENCES users (id));
";

const DOWNSTREAM_SQL: &str = "
CREATE TABLE notes (
    id UUID PRIMARY KEY,
    resource_id UUID NOT NULL REFERENCES resources (id),
    mood mood NOT NULL,
    body TEXT NOT NULL
);
CREATE TABLE documents (id UUID PRIMARY KEY REFERENCES resources (id), title TEXT NOT NULL);
";

/// Runs `program` with `arguments` in `directory`, panicking with its output
/// when it fails, and returns its standard output.
fn run(program: &str, arguments: &[&str], directory: &Path) -> String {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap_or_else(|error| panic!("Unable to run {program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {arguments:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 output")
}

/// Generates the workspace `name` of `sql` at `path` on top of `base`, and
/// returns the base description it writes.
fn generate(name: &str, sql: &str, base: Option<BaseWorkspace>, path: &Path) -> BaseWorkspace {
    let full_sql = base.as_ref().map_or_else(|| sql.to_owned(), |base| base.sql() + sql);
    let database =
        ParserDB::parse::<PostgreSqlDialect>(&full_sql).expect("Unable to parse the schema");
    let mut builder = SynQL::new(&database, path)
        .name(name)
        .sql(sql)
        .generate_workspace_toml()
        .sink_crate(&format!("{name}-sink"));
    if let Some(base) = base {
        builder = builder.base(base);
    }
    let synql: SynQL<ParserDB> = builder.into();
    synql.generate().expect("Unable to generate the workspace");

    let description = std::fs::read_to_string(path.join(format!("{name}-synql/synql.toml")))
        .expect("The workspace writes its description crate");
    BaseWorkspace::from_str(&description).expect("The description parses")
}

/// Returns the sorted directory names of the crates of the workspace at
/// `path`.
fn crate_directories(path: &Path) -> Vec<String> {
    let mut directories: Vec<String> = std::fs::read_dir(path)
        .expect("Unable to list the workspace")
        .map(|entry| entry.expect("Unable to read a workspace entry"))
        .filter(|entry| entry.path().join("Cargo.toml").exists())
        .map(|entry| entry.file_name().into_string().expect("UTF-8 name"))
        .collect();
    directories.sort_unstable();
    directories
}

#[test]
fn a_downstream_workspace_builds_on_the_crates_of_its_base() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base_path = temp_dir.path().join("base");
    let base = generate("base", BASE_SQL, None, &base_path);
    assert_eq!(base.sql(), BASE_SQL);

    run("git", &["init", "--quiet"], &base_path);
    run("git", &["add", "."], &base_path);
    run(
        "git",
        &[
            "-c",
            "user.name=synql",
            "-c",
            "user.email=synql@example.com",
            "commit",
            "--quiet",
            "-m",
            "base",
        ],
        &base_path,
    );
    let revision = run("git", &["rev-parse", "HEAD"], &base_path);
    let url = format!("file://{}", base_path.display());

    let downstream_path = temp_dir.path().join("downstream");
    generate("downstream", DOWNSTREAM_SQL, Some(base.git(&url, revision.trim())), &downstream_path);

    assert_eq!(
        crate_directories(&downstream_path),
        ["downstream-documents", "downstream-notes", "downstream-sink", "downstream-synql"]
    );
    let manifest = std::fs::read_to_string(downstream_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    assert!(
        manifest.contains(&format!(
            "base-public-mood = {{ version = \"=0.1.0\", git = \"{url}\", rev = \"{}\", features = [\"sqlite\"] }}",
            revision.trim()
        )),
        "{manifest}"
    );
    run("cargo", &["check", "--workspace", "--all-targets", "--quiet"], &downstream_path);
}

#[test]
fn a_base_published_on_crates_io_is_required_at_its_exact_version() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base = generate("base", BASE_SQL, None, &temp_dir.path().join("base"));
    let downstream_path = temp_dir.path().join("downstream");
    generate("downstream", DOWNSTREAM_SQL, Some(base), &downstream_path);

    let manifest = std::fs::read_to_string(downstream_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    let base_dependencies: Vec<&str> =
        manifest.lines().filter(|line| line.starts_with("base-")).collect();
    assert_eq!(
        base_dependencies,
        [
            "base-resources = { version = \"=0.1.0\" }",
            "base-public-mood = { version = \"=0.1.0\", features = [\"sqlite\"] }",
        ]
    );
}

#[test]
fn a_workspace_built_on_a_base_is_itself_a_base() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base = generate("base", BASE_SQL, None, &temp_dir.path().join("base"));
    let middle = generate("middle", DOWNSTREAM_SQL, Some(base), &temp_dir.path().join("middle"));
    assert_eq!(middle.sql(), format!("{BASE_SQL}{DOWNSTREAM_SQL}"));

    let top_path = temp_dir.path().join("top");
    generate(
        "top",
        "CREATE TABLE reviews (id UUID PRIMARY KEY, note_id UUID NOT NULL REFERENCES notes (id), reviewer_id UUID NOT NULL REFERENCES users (id));",
        Some(middle),
        &top_path,
    );

    assert_eq!(crate_directories(&top_path), ["top-reviews", "top-sink", "top-synql"]);
    let manifest = std::fs::read_to_string(top_path.join("top-reviews/Cargo.toml"))
        .expect("Unable to read the crate manifest");
    assert!(manifest.contains("base-users.workspace = true"), "{manifest}");
    assert!(manifest.contains("middle-notes.workspace = true"), "{manifest}");
}

#[test]
fn a_base_table_missing_from_the_schema_is_refused() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base = generate("base", BASE_SQL, None, &temp_dir.path().join("base"));
    let database =
        ParserDB::parse::<PostgreSqlDialect>("CREATE TABLE notes (id UUID PRIMARY KEY);")
            .expect("Unable to parse the schema");
    let downstream_path = temp_dir.path().join("downstream");
    let synql: SynQL<ParserDB> = SynQL::new(&database, &downstream_path).base(base).into();

    let error = synql.generate().expect_err("The base tables are missing");
    assert!(
        matches!(&error, synql::Error::MissingBaseTable(table) if table == "resources"),
        "Unexpected error: {error}"
    );
}

#[test]
fn a_base_enum_type_missing_from_the_schema_is_refused() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base = generate(
        "base",
        "CREATE TYPE mood AS ENUM ('happy', 'sad');\nCREATE TABLE users (id UUID PRIMARY KEY);",
        None,
        &temp_dir.path().join("base"),
    );
    let database =
        ParserDB::parse::<PostgreSqlDialect>("CREATE TABLE users (id UUID PRIMARY KEY);")
            .expect("Unable to parse the schema");
    let downstream_path = temp_dir.path().join("downstream");
    let synql: SynQL<ParserDB> = SynQL::new(&database, &downstream_path).base(base).into();

    let error = synql.generate().expect_err("The base enum type is missing");
    assert!(
        matches!(&error, synql::Error::MissingBaseType(enum_type) if enum_type == "public.mood"),
        "Unexpected error: {error}"
    );
}

#[test]
fn a_downstream_table_depends_on_its_spouse_in_the_base() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base = generate(
        "base",
        "CREATE TABLE entities (id UUID PRIMARY KEY);\nCREATE TABLE accounts (id UUID PRIMARY KEY REFERENCES entities (id), login TEXT NOT NULL);",
        None,
        &temp_dir.path().join("base"),
    );
    let downstream_path = temp_dir.path().join("downstream");
    generate(
        "downstream",
        "CREATE TABLE members (id UUID PRIMARY KEY REFERENCES entities (id), nickname TEXT NOT NULL);
CREATE TABLE member_accounts (
    id UUID PRIMARY KEY,
    FOREIGN KEY (id) REFERENCES members (id),
    FOREIGN KEY (id) REFERENCES accounts (id)
);",
        Some(base),
        &downstream_path,
    );

    let manifest = std::fs::read_to_string(downstream_path.join("downstream-members/Cargo.toml"))
        .expect("Unable to read the crate manifest");
    assert!(manifest.contains("base-accounts.workspace = true"), "{manifest}");
}

#[test]
fn a_base_table_altered_to_depend_on_a_downstream_table_is_refused() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base = generate("base", BASE_SQL, None, &temp_dir.path().join("base"));
    let database = ParserDB::parse::<PostgreSqlDialect>(&format!(
        "{}{DOWNSTREAM_SQL}ALTER TABLE users ADD COLUMN pinned_note_id UUID REFERENCES notes (id);",
        base.sql()
    ))
    .expect("Unable to parse the schema");
    let downstream_path = temp_dir.path().join("downstream");
    let synql: SynQL<ParserDB> = SynQL::new(&database, &downstream_path).base(base).into();

    let error = synql.generate().expect_err("The base table now depends on the downstream");
    assert!(
        matches!(
            &error,
            synql::Error::BaseTableDependsOnGeneratedTable { base_table, generated_table }
                if base_table == "users" && generated_table == "notes"
        ),
        "Unexpected error: {error}"
    );
}

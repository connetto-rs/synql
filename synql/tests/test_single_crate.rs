//! Tests for writing a workspace as a single crate, which holds every table
//! and enum type in its own module, carries the description of the workspace
//! and puts its models behind its default `models` feature.

mod common;

use std::{path::Path, str::FromStr};

use common::{crate_directories, run};
use quote::quote;
use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE SCHEMA authz;
CREATE TYPE authz.team_role AS ENUM ('member', 'admin');
CREATE TYPE mood AS ENUM ('happy', 'sad');
CREATE TABLE users (id UUID PRIMARY KEY, name TEXT NOT NULL CHECK (name <> ''), mood mood NOT NULL);
CREATE TABLE admins (id UUID PRIMARY KEY REFERENCES users (id), since DATE NOT NULL);
CREATE TABLE teams (id UUID PRIMARY KEY, owner_id UUID NOT NULL REFERENCES users (id));
CREATE TABLE memberships (
    id UUID PRIMARY KEY,
    team_id UUID NOT NULL REFERENCES teams (id),
    user_id UUID NOT NULL REFERENCES users (id),
    role authz.team_role NOT NULL
);
CREATE TABLE \"Tags\" (id UUID PRIMARY KEY, label TEXT NOT NULL);
";

const DOWNSTREAM_SQL: &str = "
CREATE TABLE notes (
    id UUID PRIMARY KEY,
    team_id UUID NOT NULL REFERENCES teams (id),
    role authz.team_role NOT NULL,
    body TEXT NOT NULL
);
";

const TOP_SQL: &str = "
CREATE TABLE remarks (
    id UUID PRIMARY KEY,
    note_id UUID NOT NULL REFERENCES notes (id),
    mood mood NOT NULL
);
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
single.workspace = true
"#;

const ASSERTIONS_LIB: &str = r"//! Reaches the items of the single crate through its modules.

pub use single::authz_team_role::{TeamRole, sql_types::TeamRole as TeamRoleSqlType};
pub use single::public_mood::Mood;
pub use single::users_model::User as UserModel;
pub use single::{Admin, Membership, Owner, admins, memberships, teams, users};

pub const ADMIN: TeamRole = TeamRole::Admin;
pub const DESCRIPTION: &str = single::DESCRIPTION;
";

const MUTUAL_SQL: &str = "
CREATE TABLE alpha (id INT PRIMARY KEY, beta_id INT);
CREATE TABLE beta (id INT PRIMARY KEY, alpha_id INT REFERENCES alpha (id));
ALTER TABLE alpha ADD FOREIGN KEY (beta_id) REFERENCES beta (id);
";

const JOINS_MANIFEST: &str = r#"[package]
name = "joins"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel.workspace = true
mutual.workspace = true
"#;

const JOINS_LIB: &str = r"//! Joins the tables of the single crate, which reference each other.

use diesel::prelude::*;
use mutual::{alpha, beta};

/// Builds a join along each of the two foreign keys.
pub fn joins() {
    let _alpha_with_beta = alpha::table
        .inner_join(beta::table.on(alpha::beta_id.eq(beta::id.nullable())))
        .select((alpha::id, beta::id));
    let _beta_with_alpha = beta::table
        .inner_join(alpha::table.on(beta::alpha_id.eq(alpha::id.nullable())))
        .select((beta::id, alpha::id));
}
";

/// Returns the packages the crate `package` of the workspace at `path`
/// builds on with `features`, one per line.
fn dependency_tree(path: &Path, package: &str, features: &[&str]) -> String {
    let mut arguments = vec!["tree", "-p", package, "-e", "normal", "--prefix", "none"];
    arguments.extend_from_slice(features);
    run("cargo", &arguments, path)
}

/// Generates the single-crate workspace `name` of `sql` at `path` on top of
/// `base`, and returns the base description its crate carries.
fn generate_single(
    name: &str,
    sql: &str,
    base: Option<BaseWorkspace>,
    path: &Path,
) -> BaseWorkspace {
    let full_sql = base.as_ref().map_or_else(|| sql.to_owned(), |base| base.sql() + sql);
    let database =
        ParserDB::parse::<PostgreSqlDialect>(&full_sql).expect("Unable to parse the schema");
    let mut builder =
        SynQL::new(&database, path).name(name).sql(sql).single_crate().generate_workspace_toml();
    if let Some(base) = base {
        builder = builder.base(base);
    }
    let synql: SynQL<ParserDB> = builder.into();
    synql.generate().expect("Unable to generate the workspace");

    let description = std::fs::read_to_string(path.join(name).join("synql.toml"))
        .expect("The single crate carries the description");
    BaseWorkspace::from_str(&description).expect("The description parses")
}

/// Commits the workspace at `path` to a new git repository, and returns its
/// URL and revision.
fn commit(path: &Path) -> (String, String) {
    run("git", &["init", "--quiet"], path);
    run("git", &["add", "."], path);
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
        path,
    );
    let revision = run("git", &["rev-parse", "HEAD"], path);
    (format!("file://{}", path.display()), revision.trim().to_owned())
}

#[test]
fn a_single_crate_holds_the_whole_workspace() {
    let database =
        ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("single");
    let synql: SynQL<ParserDB> = SynQL::new(&database, &workspace_path)
        .name("single")
        .sql(SCHEMA)
        .single_crate()
        .generate_workspace_toml()
        .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
        .toml_callback(|_table, _db| Ok(Some(TomlDependency::new("serde_json").version("1.0")?)))
        .sink_callback(|_tables, _db, _workspace| {
            Ok(Some(quote! {
                pub use crate::users_model::User as Owner;
                pub fn owner_name(owner: &Owner) -> &str {
                    owner.name()
                }
            }))
        })
        .sink_callback(|_tables, _db, _workspace| Ok(None))
        .sink_toml_callback(|_db| Ok(None))
        .sink_toml_callback(|_db| Ok(Some(TomlDependency::new("serde_json").version("1.0")?)))
        .sink_toml_callback(|_db| Ok(Some(TomlDependency::new("hex").version("0.4")?)))
        .into();
    synql.generate().expect("Unable to generate the workspace");

    for (path, contents) in
        [("assertions/Cargo.toml", ASSERTIONS_MANIFEST), ("assertions/src/lib.rs", ASSERTIONS_LIB)]
    {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    assert_eq!(crate_directories(&workspace_path), ["assertions", "single"]);
    let description = std::fs::read_to_string(workspace_path.join("single/synql.toml"))
        .expect("The single crate carries the description");
    assert_eq!(
        BaseWorkspace::from_str(&description).expect("The description parses").sql(),
        SCHEMA
    );

    run("cargo", &["check", "--workspace", "--quiet"], &workspace_path);
    run("cargo", &["check", "-p", "single", "--no-default-features", "--quiet"], &workspace_path);
    run(
        "cargo",
        &["check", "-p", "single", "--no-default-features", "--features", "models", "--quiet"],
        &workspace_path,
    );

    let description_only = dependency_tree(&workspace_path, "single", &["--no-default-features"]);
    assert_eq!(description_only.lines().count(), 1, "{description_only}");
    let postgres_models = dependency_tree(
        &workspace_path,
        "single",
        &["--no-default-features", "--features", "models"],
    );
    assert!(postgres_models.contains("diesel v"), "{postgres_models}");
    assert!(postgres_models.contains("serde_json v"), "{postgres_models}");
    assert!(postgres_models.contains("hex v"), "{postgres_models}");
    assert!(!postgres_models.contains("libsqlite3-sys"), "{postgres_models}");
    let default_models = dependency_tree(&workspace_path, "single", &[]);
    assert!(default_models.contains("libsqlite3-sys"), "{default_models}");
}

#[test]
fn single_crates_and_grouped_workspaces_build_on_each_other() {
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let base_path = temp_dir.path().join("base");
    let base = generate_single("base", SCHEMA, None, &base_path);
    let (url, revision) = commit(&base_path);
    let base = base.git(&url, &revision);

    let grouped_path = temp_dir.path().join("grouped");
    let database = ParserDB::parse::<PostgreSqlDialect>(&(base.sql() + DOWNSTREAM_SQL))
        .expect("Unable to parse the schema");
    let synql: SynQL<ParserDB> = SynQL::new(&database, &grouped_path)
        .name("grouped")
        .sql(DOWNSTREAM_SQL)
        .base(base.clone())
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate the workspace");
    assert_eq!(crate_directories(&grouped_path), ["grouped-notes", "grouped-synql"]);
    let grouped_description =
        std::fs::read_to_string(grouped_path.join("grouped-synql/synql.toml"))
            .expect("The grouped workspace writes its description crate");
    let grouped = BaseWorkspace::from_str(&grouped_description).expect("The description parses");
    let (grouped_url, grouped_revision) = commit(&grouped_path);
    let manifest = std::fs::read_to_string(grouped_path.join("Cargo.toml"))
        .expect("Unable to read the workspace manifest");
    let base_dependencies: Vec<&str> =
        manifest.lines().filter(|line| line.starts_with("base")).collect();
    assert_eq!(
        base_dependencies,
        [format!(
            "base = {{ version = \"=0.1.0\", git = \"{url}\", rev = \"{revision}\", features = [\"sqlite\"] }}"
        )]
    );
    run("cargo", &["check", "--workspace", "--quiet"], &grouped_path);

    let single_path = temp_dir.path().join("downstream");
    let downstream = generate_single("downstream", DOWNSTREAM_SQL, Some(base), &single_path);
    assert_eq!(downstream.sql(), format!("{SCHEMA}{DOWNSTREAM_SQL}"));
    assert_eq!(crate_directories(&single_path), ["downstream"]);
    run("cargo", &["check", "--workspace", "--quiet"], &single_path);
    run(
        "cargo",
        &["check", "-p", "downstream", "--no-default-features", "--features", "models", "--quiet"],
        &single_path,
    );
    let postgres_models = dependency_tree(
        &single_path,
        "downstream",
        &["--no-default-features", "--features", "models"],
    );
    assert!(postgres_models.contains("base v0.1.0"), "{postgres_models}");
    assert!(!postgres_models.contains("libsqlite3-sys"), "{postgres_models}");
    let description_only = dependency_tree(&single_path, "downstream", &["--no-default-features"]);
    assert!(!description_only.contains("base v0.1.0"), "{description_only}");

    let top_path = temp_dir.path().join("top");
    let top = generate_single(
        "top",
        TOP_SQL,
        Some(grouped.git(&grouped_url, &grouped_revision)),
        &top_path,
    );
    assert_eq!(top.sql(), format!("{SCHEMA}{DOWNSTREAM_SQL}{TOP_SQL}"));
    run("cargo", &["check", "--workspace", "--quiet"], &top_path);
    let top_models = dependency_tree(&top_path, "top", &[]);
    assert!(top_models.contains("grouped-notes v0.1.0"), "{top_models}");
    assert!(top_models.contains("base v0.1.0"), "{top_models}");
}

#[test]
fn sink_crates_are_refused_in_a_single_crate() {
    let database =
        ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    for synql in [
        SynQL::new(&database, temp_dir.path()).single_crate().sink_crate("sink"),
        SynQL::new(&database, temp_dir.path()).single_crate().dag_sink_crate_prefix("sink-"),
    ] {
        let synql: SynQL<ParserDB> = synql.into();
        let error = synql.generate().expect_err("A single crate has no sink crate");
        assert!(matches!(error, synql::Error::SinkCrateInSingleCrate), "Unexpected error: {error}");
    }
}

#[test]
fn a_sink_callback_returning_statements_is_refused_in_a_single_crate() {
    let database = ParserDB::parse::<PostgreSqlDialect>("CREATE TABLE users (id INT PRIMARY KEY);")
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let synql: SynQL<ParserDB> = SynQL::new(&database, temp_dir.path())
        .name("single")
        .single_crate()
        .sink_callback(|_tables, _db, _workspace| Ok(Some(quote! { let answer = 42; })))
        .into();
    match synql.generate() {
        Err(synql::Error::UnparsableCode { path, .. }) => {
            assert!(path.ends_with("single/src/lib.rs"), "{}", path.display());
        }
        other => panic!("Expected unparsable code, got {other:?}"),
    }
}

#[test]
fn a_single_crate_of_enum_types_alone_builds_without_a_description() {
    let database =
        ParserDB::parse::<PostgreSqlDialect>("CREATE TYPE mood AS ENUM ('happy', 'sad');")
            .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("moods");
    let synql: SynQL<ParserDB> = SynQL::new(&database, &workspace_path)
        .name("moods")
        .single_crate()
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate the workspace");

    assert_eq!(crate_directories(&workspace_path), ["moods"]);
    assert!(!workspace_path.join("moods/synql.toml").exists());
    run("cargo", &["check", "--workspace", "--quiet"], &workspace_path);
    let models = dependency_tree(&workspace_path, "moods", &[]);
    assert!(models.contains("diesel v"), "{models}");
    assert!(!models.contains("diesel-builders"), "{models}");
}

#[test]
fn the_last_layout_call_decides_the_layout() {
    let database =
        ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let directories = |path: &Path, builder: SynQLBuilder<'_, ParserDB>| {
        let synql: SynQL<ParserDB> = builder.into();
        synql.generate().expect("Unable to generate the workspace");
        crate_directories(path)
    };

    let grouped_path = temp_dir.path().join("grouped");
    let grouped = directories(&grouped_path, SynQL::new(&database, &grouped_path).name("layout"));
    assert!(grouped.len() > 1, "{grouped:?}");
    let regrouped_path = temp_dir.path().join("regrouped");
    let regrouped = directories(
        &regrouped_path,
        SynQL::new(&database, &regrouped_path).name("layout").single_crate().group_tables(1.0),
    );
    assert_eq!(regrouped, grouped);

    let single_path = temp_dir.path().join("single");
    let single = directories(
        &single_path,
        SynQL::new(&database, &single_path).name("layout").group_tables(1.0).single_crate(),
    );
    assert_eq!(single, ["layout"]);
}

#[test]
fn an_enum_module_named_as_a_table_module_is_refused() {
    let database = ParserDB::parse::<PostgreSqlDialect>(
        "CREATE TYPE mood AS ENUM ('happy', 'sad');\nCREATE TABLE public_mood (id INT PRIMARY KEY);",
    )
    .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let synql: SynQL<ParserDB> =
        SynQL::new(&database, temp_dir.path()).name("clash").single_crate().into();

    let error = synql.generate().expect_err("The enum module shadows the table module");
    assert!(
        matches!(
            &error,
            synql::Error::ModuleNameClash { module_ident, .. } if module_ident == "public_mood"
        ),
        "Unexpected error: {error}"
    );
}

#[test]
fn tables_referencing_each_other_build_in_a_single_crate() {
    let database =
        ParserDB::parse::<PostgreSqlDialect>(MUTUAL_SQL).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("mutual");
    let synql: SynQL<ParserDB> = SynQL::new(&database, &workspace_path)
        .name("mutual")
        .single_crate()
        .generate_workspace_toml()
        .member(TomlDependency::new("joins").path("joins").expect("A valid path"))
        .into();
    synql.generate().expect("Unable to generate the workspace");

    for (path, contents) in [("joins/Cargo.toml", JOINS_MANIFEST), ("joins/src/lib.rs", JOINS_LIB)]
    {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    run("cargo", &["check", "--workspace", "--quiet"], &workspace_path);
}

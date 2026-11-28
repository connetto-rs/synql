//! Test module to verify that the generated workspace passes `cargo clippy`
//! with warnings denied, as written by `synql` and before any `cargo fmt`.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

/// Generates the workspace of `db` at `workspace_path`, writing `files` into
/// it, and asserts that `cargo clippy --all-targets -- -D warnings` accepts
/// it.
fn assert_clippy_accepts_with<'db>(
    db: &'db ParserDB,
    workspace_path: &'db std::path::Path,
    configure: impl FnOnce(SynQLBuilder<'db, ParserDB>) -> SynQLBuilder<'db, ParserDB>,
    files: &[(&str, &str)],
) {
    let synql: SynQL<ParserDB> =
        configure(SynQL::new(db, workspace_path).name("clippy").generate_workspace_toml()).into();
    synql.generate().expect("Unable to generate workspace");
    for (path, contents) in files {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    let output = Command::new("cargo")
        .args(["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])
        .current_dir(workspace_path)
        .output()
        .expect("Unable to run cargo clippy");
    assert!(
        output.status.success(),
        "cargo clippy rejected the generated workspace:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Generates the workspace of `schema` and asserts that `cargo clippy
/// --all-targets -- -D warnings` accepts it.
fn assert_clippy_accepts(schema: &str) {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    assert_clippy_accepts_with(&db, &temp_dir.path().join("clippy_workspace"), |synql| synql, &[]);
}

#[test]
fn negated_comparisons_are_written_without_negation() {
    assert_clippy_accepts(
        "
CREATE TABLE samples (
    id INT PRIMARY KEY,
    temperature_k DOUBLE PRECISION NOT NULL CHECK (temperature_k < 'Infinity'::double precision),
    minimum_k DOUBLE PRECISION NOT NULL,
    maximum_k DOUBLE PRECISION NOT NULL,
    first_count INT NOT NULL,
    second_count INT NOT NULL,
    CHECK (minimum_k + 1.0 <= maximum_k),
    CHECK (first_count + 1 < second_count),
    CHECK (first_count + 1 <> second_count)
);",
    );
}

#[test]
fn a_column_tied_by_several_keys_to_one_column_carries_one_same_as() {
    assert_clippy_accepts(
        "
CREATE TABLE templates (id INT PRIMARY KEY);
CREATE TABLE slots (
    id INT PRIMARY KEY,
    template_id INT NOT NULL REFERENCES templates (id),
    UNIQUE (id, template_id)
);
CREATE TABLE aliquotings (
    id INT PRIMARY KEY REFERENCES templates (id),
    from_slot_id INT NOT NULL REFERENCES slots (id),
    into_slot_id INT NOT NULL REFERENCES slots (id),
    FOREIGN KEY (from_slot_id, id) REFERENCES slots (id, template_id),
    FOREIGN KEY (into_slot_id, id) REFERENCES slots (id, template_id)
);",
    );
}

#[test]
fn mandatory_slot_builders_wider_than_any_hierarchy_compile() {
    assert_clippy_accepts(
        "
CREATE TABLE templates (id INT PRIMARY KEY);
CREATE TABLE models (id INT PRIMARY KEY);
CREATE TABLE slots (
    id INT PRIMARY KEY,
    template_id INT NOT NULL REFERENCES templates (id),
    model_id INT NOT NULL REFERENCES models (id),
    extra_0 INT, extra_1 INT, extra_2 INT, extra_3 INT, extra_4 INT, extra_5 INT,
    UNIQUE (id, template_id, model_id)
);
CREATE TABLE disposals (
    id INT PRIMARY KEY REFERENCES templates (id),
    slot_0_id INT NOT NULL, model_0_id INT NOT NULL REFERENCES models (id),
    slot_1_id INT NOT NULL, model_1_id INT NOT NULL REFERENCES models (id),
    slot_2_id INT NOT NULL, model_2_id INT NOT NULL REFERENCES models (id),
    slot_3_id INT NOT NULL, model_3_id INT NOT NULL REFERENCES models (id),
    FOREIGN KEY (slot_0_id, id, model_0_id) REFERENCES slots (id, template_id, model_id),
    FOREIGN KEY (slot_1_id, id, model_1_id) REFERENCES slots (id, template_id, model_id),
    FOREIGN KEY (slot_2_id, id, model_2_id) REFERENCES slots (id, template_id, model_id),
    FOREIGN KEY (slot_3_id, id, model_3_id) REFERENCES slots (id, template_id, model_id)
);",
    );
}

#[test]
fn discretionary_slot_builders_wider_than_any_hierarchy_compile() {
    assert_clippy_accepts(
        "
CREATE TABLE templates (id INT PRIMARY KEY);
CREATE TABLE models (id INT PRIMARY KEY);
CREATE TABLE slots (
    id INT PRIMARY KEY,
    template_id INT NOT NULL REFERENCES templates (id),
    model_id INT NOT NULL REFERENCES models (id),
    extra_0 INT, extra_1 INT, extra_2 INT, extra_3 INT, extra_4 INT, extra_5 INT,
    UNIQUE (id, model_id)
);
CREATE TABLE disposals (
    id INT PRIMARY KEY REFERENCES templates (id),
    slot_0_id INT NOT NULL, model_0_id INT NOT NULL REFERENCES models (id),
    slot_1_id INT NOT NULL, model_1_id INT NOT NULL REFERENCES models (id),
    slot_2_id INT NOT NULL, model_2_id INT NOT NULL REFERENCES models (id),
    slot_3_id INT NOT NULL, model_3_id INT NOT NULL REFERENCES models (id),
    FOREIGN KEY (slot_0_id, model_0_id) REFERENCES slots (id, model_id),
    FOREIGN KEY (slot_1_id, model_1_id) REFERENCES slots (id, model_id),
    FOREIGN KEY (slot_2_id, model_2_id) REFERENCES slots (id, model_id),
    FOREIGN KEY (slot_3_id, model_3_id) REFERENCES slots (id, model_id)
);",
    );
}

#[test]
fn consecutive_record_checks_are_not_mistaken_for_a_missing_else() {
    assert_clippy_accepts(
        "
CREATE TABLE events (
    id INT PRIMARY KEY,
    first_label TEXT,
    second_label TEXT,
    first_count INT NOT NULL,
    second_count INT NOT NULL,
    CHECK (lower(first_label) <> lower(second_label) AND length(first_label) < length(second_label)),
    CHECK (first_count + 1 < second_count AND first_count + 2 < second_count)
);",
    );
}

#[test]
fn comparisons_with_boolean_literals_read_the_boolean() {
    assert_clippy_accepts(
        "
CREATE TABLE switches (
    id INT PRIMARY KEY,
    enabled BOOLEAN NOT NULL,
    archived BOOLEAN NOT NULL CHECK (archived = false),
    flagged BOOLEAN CHECK (flagged <> true),
    label TEXT,
    first_count INT NOT NULL,
    second_count INT NOT NULL,
    CHECK (enabled = true OR label IS NOT NULL),
    CHECK (false = archived OR label IS NULL),
    CHECK (flagged <> true OR label IS NOT NULL),
    CHECK ((first_count < second_count) = true)
);",
    );
}

#[test]
fn boolean_negations_read_the_boolean() {
    assert_clippy_accepts(
        "
CREATE TABLE namespaces (
    id INT PRIMARY KEY,
    owner INT,
    reserved BOOLEAN NOT NULL,
    archived BOOLEAN NOT NULL CHECK (NOT archived),
    retired BOOLEAN CHECK (NOT retired),
    code TEXT NOT NULL CHECK (NOT (code LIKE 'a%')),
    first_count INT NOT NULL,
    second_count INT NOT NULL,
    CHECK (NOT reserved OR owner IS NULL),
    CHECK (NOT (first_count < second_count)),
    CHECK (NOT (first_count < second_count) OR owner IS NULL),
    CHECK (NOT NOT reserved OR owner IS NOT NULL),
    CHECK (NOT (reserved AND retired))
);",
    );
}

#[test]
fn row_comparisons_compare_member_by_member() {
    assert_clippy_accepts(
        "
CREATE TABLE teams (
    namespace_id INT NOT NULL,
    id INT NOT NULL,
    parent_namespace_id INT,
    parent_id INT,
    slug TEXT NOT NULL,
    parent_slug TEXT,
    PRIMARY KEY (namespace_id, id),
    CHECK ((parent_namespace_id, parent_id) <> (namespace_id, id)),
    CHECK ((namespace_id, slug) <> (id, 'root')),
    CHECK ((slug, namespace_id) = ('main', id)),
    CHECK (NOT ((namespace_id, parent_slug) = (id, slug)))
);",
    );
}

#[test]
fn validation_functions_borrow_copied_columns() {
    let db = ParserDB::parse::<PostgreSqlDialect>(
        "
CREATE FUNCTION is_positive(INT) RETURNS BOOLEAN AS 'SELECT $1 > 0' LANGUAGE SQL;
CREATE FUNCTION is_ordered(INT, INT) RETURNS BOOLEAN AS 'SELECT $1 < $2' LANGUAGE SQL;
CREATE TABLE ranges (
    id INT PRIMARY KEY,
    low INT NOT NULL CHECK (is_positive(low)),
    high INT,
    CHECK (is_ordered(low, high))
);",
    )
    .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let checks = ExternalCrate::new("checks")
        .expect("A valid crate name")
        .functions([
            ExternalFunction::new("is_positive", syn::parse_quote!(::checks::is_positive)),
            ExternalFunction::new("is_ordered", syn::parse_quote!(::checks::is_ordered)),
        ])
        .into();
    assert_clippy_accepts_with(
        &db,
        &temp_dir.path().join("clippy_workspace"),
        |synql| {
            synql
                .external_crate(checks)
                .member(TomlDependency::new("checks").path("checks").expect("A valid path"))
        },
        &[
            (
                "checks/Cargo.toml",
                "[package]\nname = \"checks\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            ),
            (
                "checks/src/lib.rs",
                "//! Validation functions the schema's check constraints call.

/// Accepts a positive number.
///
/// # Errors
///
/// Rejects zero and negative numbers.
pub fn is_positive(value: &i32) -> Result<(), std::fmt::Error> {
    if *value > 0 { Ok(()) } else { Err(std::fmt::Error) }
}

/// Accepts a first number smaller than the second.
///
/// # Errors
///
/// Rejects a first number at least as large as the second.
pub fn is_ordered(first: &i32, second: &i32) -> Result<(), std::fmt::Error> {
    if first < second { Ok(()) } else { Err(std::fmt::Error) }
}
",
            ),
        ],
    );
}

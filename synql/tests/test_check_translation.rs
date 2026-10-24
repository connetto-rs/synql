//! Generated check constraints accept a row when the SQL expression is TRUE
//! or NULL and reject it when it is FALSE, as PostgreSQL does.
//!
//! The test generates a workspace, adds a crate of validation functions and a
//! crate of assertions over the generated validation code, and runs the
//! assertions with `cargo test`.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE FUNCTION is_lowercase(TEXT) RETURNS BOOLEAN AS 'SELECT $1 = lower($1)' LANGUAGE SQL;
CREATE FUNCTION is_shorter(TEXT, TEXT) RETURNS BOOLEAN AS 'SELECT length($1) < length($2)' LANGUAGE SQL;
CREATE FUNCTION is_positive(INT) RETURNS BOOLEAN AS 'SELECT $1 > 0' LANGUAGE SQL;
CREATE TABLE ranges (
    id INT PRIMARY KEY,
    low INT NOT NULL,
    high INT,
    CHECK (low + 1 < high),
    CHECK (high > 0 AND low >= 0)
);
CREATE TABLE labels (
    id INT PRIMARY KEY,
    label TEXT
        CHECK (label IS NULL OR length(label) > 2)
        CHECK (label <> 'none' OR label IS NULL)
        CHECK (label IS NOT NULL OR label = 'x')
);
CREATE TABLE scores (
    id INT PRIMARY KEY,
    score INT NOT NULL CHECK ((score * 2)::INT >= 0),
    bonus INT NOT NULL CHECK (100 > bonus)
);
CREATE TABLE overrides (
    id INT PRIMARY KEY,
    low INT NOT NULL,
    high INT,
    note TEXT,
    CHECK (high IS NULL OR high > low OR note = 'override')
);
CREATE TABLE windows (
    id INT PRIMARY KEY,
    opens INT,
    closes INT,
    CHECK (opens IS NULL OR (closes IS NOT NULL AND opens < closes))
);
CREATE TABLE codes (
    id INT PRIMARY KEY,
    code TEXT CHECK (is_lowercase(code)),
    alias TEXT,
    CHECK (is_lowercase(alias) OR code IS NOT NULL),
    CHECK (is_shorter(code, alias))
);
CREATE TABLE budgets (
    id INT PRIMARY KEY,
    spent INT NOT NULL,
    cap INT NOT NULL,
    CHECK ((10) + spent <= cap),
    CHECK ((cap - spent) * 2 / 2 % 1000 >= 0),
    CHECK (cap > spent),
    CHECK (cap >= spent)
);
CREATE TABLE flags (
    id INT PRIMARY KEY,
    enabled BOOLEAN NOT NULL,
    label TEXT,
    CHECK (enabled = true OR label IS NOT NULL)
);
CREATE TABLE switches (
    id INT PRIMARY KEY,
    on_off BOOLEAN NOT NULL
        CHECK (false OR on_off)
        CHECK (on_off OR (on_off = false AND on_off = false))
);
CREATE TABLE memos (
    id INT PRIMARY KEY,
    memo TEXT,
    priority INT NOT NULL CHECK (is_positive(priority)),
    CHECK (length(memo) > 2 OR priority > 0),
    CHECK (length(memo) <= 50 AND priority < 100)
);
CREATE TABLE deadlines (
    id INT PRIMARY KEY,
    due TIMESTAMPTZ,
    opened TIMESTAMPTZ NOT NULL CHECK (opened <= NOW()) CHECK (opened < NOW()),
    closed TIMESTAMPTZ NOT NULL CHECK (closed > NOW()),
    CHECK (NOW() < due OR opened < due),
    CHECK (due < NOW() AND opened < due)
);
CREATE TABLE docs (
    id INT PRIMARY KEY,
    body JSONB,
    meta JSONB NOT NULL,
    CHECK (body IS NOT NULL OR meta = meta)
);
";

const CHECKS_MANIFEST: &str = r#"[package]
name = "checks"
version = "0.1.0"
edition = "2024"
"#;

const CHECKS_LIB: &str = r#"//! Validation functions the schema's check constraints call.

/// A text that breaks a validation function.
#[derive(Debug)]
pub struct Rejected(&'static str);

impl core::fmt::Display for Rejected {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

impl core::error::Error for Rejected {}

/// Accepts a text without uppercase letters.
///
/// # Errors
///
/// Rejects a text with an uppercase letter.
pub fn is_lowercase(value: &str) -> Result<(), Rejected> {
    if value.chars().any(char::is_uppercase) { Err(Rejected("not lowercase")) } else { Ok(()) }
}

/// Accepts a first text shorter than the second.
///
/// # Errors
///
/// Rejects a first text at least as long as the second.
pub fn is_shorter(first: &str, second: &str) -> Result<(), Rejected> {
    if first.chars().count() < second.chars().count() { Ok(()) } else { Err(Rejected("not shorter")) }
}

/// Accepts a positive number.
///
/// # Errors
///
/// Rejects zero and negative numbers.
pub fn is_positive(value: &i32) -> Result<(), Rejected> {
    if *value > 0 { Ok(()) } else { Err(Rejected("not positive")) }
}
"#;

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel-builders.workspace = true
validation-errors.workspace = true
synql-check-translation-ranges.workspace = true
synql-check-translation-labels.workspace = true
synql-check-translation-scores.workspace = true
synql-check-translation-overrides.workspace = true
synql-check-translation-windows.workspace = true
synql-check-translation-codes.workspace = true
synql-check-translation-budgets.workspace = true
synql-check-translation-flags.workspace = true
synql-check-translation-switches.workspace = true
synql-check-translation-memos.workspace = true
synql-check-translation-deadlines.workspace = true
synql-check-translation-docs.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over the generated validation code.

use diesel_builders::{SetColumn, TableExt, ValidateColumn, ValidateRecord};
use validation_errors::{ValidationError, ValidationErrorKind};

fn is_check_violation(result: Result<(), ValidationError>) -> bool {
    matches!(result, Err(error) if matches!(error.kind(), ValidationErrorKind::CheckViolated { .. }))
}

fn is_function_rejection(result: Result<(), ValidationError>) -> bool {
    matches!(result, Err(error) if matches!(error.kind(), ValidationErrorKind::Generic { .. }))
}

#[test]
fn arithmetic_comparisons_pass_on_null_and_fail_on_false() {
    use synql_check_translation_ranges::ranges;
    let validate = |low: i32, high: Option<i32>| {
        let mut values = ranges::table::empty_new_values();
        SetColumn::<ranges::low>::set_column(&mut values, low);
        SetColumn::<ranges::high>::set_column(&mut values, high);
        ValidateRecord::<ranges::table>::validate_record(&values)
    };
    assert!(validate(1, Some(5)).is_ok());
    assert!(validate(1, None).is_ok());
    assert!(is_check_violation(validate(4, Some(5))));
}

#[test]
fn single_column_checks_validate_the_value() {
    use synql_check_translation_labels::labels;
    use synql_check_translation_scores::scores;
    type Labels = <labels::table as TableExt>::NewValues;
    type Scores = <scores::table as TableExt>::NewValues;

    assert!(<Labels as ValidateColumn<labels::label>>::validate_column(&"abc".to_owned()).is_ok());
    assert!(is_check_violation(<Labels as ValidateColumn<labels::label>>::validate_column(
        &"ab".to_owned()
    )));
    assert!(is_check_violation(<Labels as ValidateColumn<labels::label>>::validate_column(
        &"none".to_owned()
    )));
    assert!(<Scores as ValidateColumn<scores::score>>::validate_column(&3).is_ok());
    assert!(is_check_violation(<Scores as ValidateColumn<scores::score>>::validate_column(&-1)));
    assert!(<Scores as ValidateColumn<scores::bonus>>::validate_column(&99).is_ok());
    assert!(matches!(
        <Scores as ValidateColumn<scores::bonus>>::validate_column(&100),
        Err(error) if matches!(error.kind(), ValidationErrorKind::MustBeStrictlySmallerThanScalar(..))
    ));
}

#[test]
fn or_follows_three_valued_logic() {
    use synql_check_translation_overrides::overrides;
    let validate = |low: i32, high: Option<i32>, note: Option<&str>| {
        let mut values = overrides::table::empty_new_values();
        SetColumn::<overrides::low>::set_column(&mut values, low);
        SetColumn::<overrides::high>::set_column(&mut values, high);
        SetColumn::<overrides::note>::set_column(&mut values, note.map(str::to_owned));
        ValidateRecord::<overrides::table>::validate_record(&values)
    };
    assert!(validate(5, None, None).is_ok());
    assert!(validate(5, Some(9), None).is_ok());
    assert!(validate(5, Some(1), None).is_ok());
    assert!(validate(5, Some(1), Some("override")).is_ok());
    assert!(is_check_violation(validate(5, Some(1), Some("locked"))));
}

#[test]
fn and_follows_three_valued_logic() {
    use synql_check_translation_windows::windows;
    let validate = |opens: Option<i32>, closes: Option<i32>| {
        let mut values = windows::table::empty_new_values();
        SetColumn::<windows::opens>::set_column(&mut values, opens);
        SetColumn::<windows::closes>::set_column(&mut values, closes);
        ValidateRecord::<windows::table>::validate_record(&values)
    };
    assert!(validate(None, None).is_ok());
    assert!(validate(Some(1), Some(2)).is_ok());
    assert!(is_check_violation(validate(Some(1), None)));
    assert!(is_check_violation(validate(Some(3), Some(2))));
}

#[test]
fn validation_functions_reject_through_their_own_error_or_the_check() {
    use synql_check_translation_codes::codes;
    type Codes = <codes::table as TableExt>::NewValues;
    let validate = |code: Option<&str>, alias: Option<&str>| {
        let mut values = codes::table::empty_new_values();
        SetColumn::<codes::code>::set_column(&mut values, code.map(str::to_owned));
        SetColumn::<codes::alias>::set_column(&mut values, alias.map(str::to_owned));
        ValidateRecord::<codes::table>::validate_record(&values)
    };

    assert!(<Codes as ValidateColumn<codes::code>>::validate_column(&"abc".to_owned()).is_ok());
    assert!(is_function_rejection(<Codes as ValidateColumn<codes::code>>::validate_column(
        &"Abc".to_owned()
    )));
    assert!(validate(Some("ab"), Some("abc")).is_ok());
    assert!(validate(None, Some("abc")).is_ok());
    assert!(validate(None, None).is_ok());
    assert!(is_check_violation(validate(None, Some("Abc"))));
    assert!(is_function_rejection(validate(Some("abcd"), Some("abc"))));
}

#[test]
fn arithmetic_with_leading_literals_and_every_operator() {
    use synql_check_translation_budgets::budgets;
    let validate = |spent: i32, cap: i32| {
        let mut values = budgets::table::empty_new_values();
        SetColumn::<budgets::spent>::set_column(&mut values, spent);
        SetColumn::<budgets::cap>::set_column(&mut values, cap);
        ValidateRecord::<budgets::table>::validate_record(&values)
    };
    assert!(validate(5, 20).is_ok());
    assert!(is_check_violation(validate(15, 20)));
}

#[test]
fn boolean_columns_and_literals() {
    use synql_check_translation_flags::flags;
    use synql_check_translation_switches::switches;
    type Switches = <switches::table as TableExt>::NewValues;
    let validate = |enabled: bool, label: Option<&str>| {
        let mut values = flags::table::empty_new_values();
        SetColumn::<flags::enabled>::set_column(&mut values, enabled);
        SetColumn::<flags::label>::set_column(&mut values, label.map(str::to_owned));
        ValidateRecord::<flags::table>::validate_record(&values)
    };
    assert!(validate(true, None).is_ok());
    assert!(validate(false, Some("x")).is_ok());
    assert!(is_check_violation(validate(false, None)));
    assert!(<Switches as ValidateColumn<switches::on_off>>::validate_column(&true).is_ok());
    assert!(is_check_violation(<Switches as ValidateColumn<switches::on_off>>::validate_column(
        &false
    )));
}

#[test]
fn lengths_of_null_text_and_functions_of_numbers() {
    use synql_check_translation_memos::memos;
    type Memos = <memos::table as TableExt>::NewValues;
    let validate = |memo: Option<&str>, priority: i32| {
        let mut values = memos::table::empty_new_values();
        SetColumn::<memos::memo>::set_column(&mut values, memo.map(str::to_owned));
        SetColumn::<memos::priority>::set_column(&mut values, priority);
        ValidateRecord::<memos::table>::validate_record(&values)
    };
    assert!(validate(None, 0).is_ok());
    assert!(validate(Some("abc"), 0).is_ok());
    assert!(is_check_violation(validate(Some("ab"), 0)));
    assert!(<Memos as ValidateColumn<memos::priority>>::validate_column(&1).is_ok());
    assert!(is_function_rejection(<Memos as ValidateColumn<memos::priority>>::validate_column(&0)));
}
"#;

#[test]
fn test_check_translation() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_check_translation");

    let checks = ExternalCrate::new("checks")
        .expect("A valid crate name")
        .functions([
            ExternalFunction::new("is_lowercase", syn::parse_quote!(::checks::is_lowercase)),
            ExternalFunction::new("is_shorter", syn::parse_quote!(::checks::is_shorter)),
            ExternalFunction::new("is_positive", syn::parse_quote!(::checks::is_positive)),
        ])
        .into();
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-check-translation")
        .external_crate(checks)
        .member(TomlDependency::new("checks").path("checks").expect("A valid path"))
        .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("checks/Cargo.toml", CHECKS_MANIFEST),
        ("checks/src/lib.rs", CHECKS_LIB),
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/checks.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over the generated validation code.\n"),
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
        "The generated validation code failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

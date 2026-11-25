//! Check constraints with SQL that has no Rust translation fail the
//! generation with an error naming the construct.

use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const FUNCTIONS: &str = "
CREATE FUNCTION not_mapped(TEXT) RETURNS BOOLEAN AS 'SELECT true' LANGUAGE SQL;
CREATE FUNCTION is_lowercase_relation(REGCLASS) RETURNS BOOLEAN AS 'SELECT true' LANGUAGE SQL;
CREATE FUNCTION is_shorter(shorter TEXT, longer TEXT) RETURNS BOOLEAN AS 'SELECT true' LANGUAGE SQL;
CREATE FUNCTION has_prefix(TEXT, TEXT DEFAULT '') RETURNS BOOLEAN AS 'SELECT true' LANGUAGE SQL;
";

/// Generates a workspace for the table and returns the generation error.
fn generation_error(table: &str) -> Error {
    let db = ParserDB::parse::<PostgreSqlDialect>(&format!("{FUNCTIONS}{table}"))
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let checks = ExternalCrate::new("checks")
        .expect("A valid crate name")
        .functions([
            ExternalFunction::new(
                "is_lowercase_relation",
                syn::parse_quote!(::checks::is_lowercase_relation),
            ),
            ExternalFunction::new("is_shorter", syn::parse_quote!(::checks::is_shorter)),
            ExternalFunction::new("has_prefix", syn::parse_quote!(::checks::has_prefix)),
        ])
        .into();
    let synql: SynQL<ParserDB> =
        SynQL::new(&db, temp_dir.path()).name("untranslatable").external_crate(checks).into();
    synql.generate().expect_err("The check constraint cannot be translated")
}

/// Returns the construct the check constraint of the table cannot translate.
fn unsupported_reason(table: &str) -> UnsupportedCheck {
    match generation_error(table) {
        Error::UnsupportedCheckConstraint { reason, .. } => reason,
        other => panic!("Expected an unsupported check constraint, got {other:?}"),
    }
}

#[test]
fn columns_without_a_rust_type_are_unsupported() {
    let db = ParserDB::parse::<PostgreSqlDialect>(
        "CREATE TABLE t (id INT PRIMARY KEY, a INT, b INT, CHECK (a > 0 OR b > 0));",
    )
    .expect("Unable to parse the schema");
    let table = db
        .table_by_target(TargetName::new("t", false), IdentifierCase::AsWritten)
        .expect("Unable to look up the table")
        .expect("The table exists");
    let check = table
        .check_constraints(&db)
        .expect("Unable to list the check constraints")
        .next()
        .expect("The table has a check constraint");
    let workspace_without_types: Workspace =
        Workspace::new().name("bare").expect("A valid workspace name").into();

    let error = check
        .to_syn(&db, &workspace_without_types, &[])
        .expect_err("No column has a Rust type in the workspace");
    assert!(
        matches!(
            &error,
            Error::UnsupportedCheckConstraint { reason: UnsupportedCheck::ColumnType(column), .. }
                if column == "t.a"
        ),
        "Unexpected error: {error:?}"
    );
}

#[test]
fn a_check_that_is_not_a_predicate_is_unsupported() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (a BETWEEN 1 AND 2));"
        ),
        UnsupportedCheck::Expression(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (a <> NULL AND a > 0));"
        ),
        UnsupportedCheck::Expression(_)
    ));
}

#[test]
fn numbers_need_a_typed_operand() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (1 < 2 AND a > 0));"
        ),
        UnsupportedCheck::UntypedNumber(number) if number == "1"
    ));
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (a > 1.5));"),
        UnsupportedCheck::MixedTypes { right, .. } if right == "1.5"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, a INT, b BIGINT, CHECK (a + 1 < b));"
        ),
        UnsupportedCheck::MixedTypes { left, right } if left == "i32" && right == "i64"
    ));
}

#[test]
fn enum_columns_compare_only_with_their_labels() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TYPE mood AS ENUM ('sad'); CREATE TABLE t (id INT PRIMARY KEY, a mood, CHECK (a = 'glad'));"
        ),
        UnsupportedCheck::MixedTypes { left, right } if left.ends_with("Mood") && right == "'glad'"
    ));
}

#[test]
fn operators_need_operands_of_their_kind() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (name || 'x' = 'y'));"
        ),
        UnsupportedCheck::Operator(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, start_date DATE, end_date DATE, CHECK (end_date - start_date > 0));"
        ),
        UnsupportedCheck::Operator(operator) if operator == "-"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (CAST(a AS TEXT) = 'x'));"
        ),
        UnsupportedCheck::Cast(_)
    ));
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (NOT a));"),
        UnsupportedCheck::Operator(operator) if operator == "NOT"
    ));
}

#[test]
fn builtin_functions_need_their_operand_types() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, data BYTEA, CHECK (length(data) > 1));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, data BYTEA, CHECK (length(data, 'UTF8') > 1));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, created DATE, CHECK (now() > created));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
}

#[test]
fn like_needs_text_and_a_literal_pattern_with_the_default_escape() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, other TEXT, CHECK (name LIKE other));"
        ),
        UnsupportedCheck::Expression(pattern) if pattern == "other"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (name ILIKE 'x\\'));"
        ),
        UnsupportedCheck::Expression(pattern) if pattern == "'x\\'"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (name LIKE 'x!%' ESCAPE '!'));"
        ),
        UnsupportedCheck::Expression(_)
    ));
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, data BYTEA, CHECK (data LIKE 'a%'));"),
        UnsupportedCheck::Operator(operator) if operator == "LIKE"
    ));
}

#[test]
fn validation_functions_need_plain_mapped_calls() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (has_prefix(name)));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (is_shorter(name, longer => name)));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (not_mapped(name)));"
        ),
        UnsupportedCheck::UnmappedFunction(function) if function == "not_mapped"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, created TIMESTAMPTZ, CHECK (created < CURRENT_TIMESTAMP));"
        ),
        UnsupportedCheck::UnmappedFunction(function) if function == "CURRENT_TIMESTAMP"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (is_lowercase_relation(name)));"
        ),
        UnsupportedCheck::FunctionArgumentType { function, index: 0 } if function == "is_lowercase_relation"
    ));
}

#[test]
fn now_shifts_need_intervals_of_fixed_length() {
    let reason = |interval: &str| {
        unsupported_reason(&format!(
            "CREATE TABLE t (id INT PRIMARY KEY, at TIMESTAMPTZ, CHECK (at <= now() + {interval}));"
        ))
    };
    assert!(matches!(
        reason("INTERVAL '1 day'"),
        UnsupportedCheck::Interval(interval) if interval == "INTERVAL '1 day'"
    ));
    for interval in [
        "INTERVAL '1 week'",
        "INTERVAL '1 mon'",
        "INTERVAL '1 year 5 minutes'",
        "'1 day 00:05:00'::interval",
        "INTERVAL '5' MINUTE",
        "INTERVAL '1 minute 5 minutes'",
        "INTERVAL '2 hours 00:30'",
        "INTERVAL '5 minutes ago ago'",
        "INTERVAL 'ago'",
        "INTERVAL '00:60'",
        "INTERVAL '00:05:00.1234567'",
        "INTERVAL '1000000000 hours'",
        "INTERVAL '43800001 hours'",
        "INTERVAL '-1 hour 30 minutes'",
        "INTERVAL '-5min 3 s'",
    ] {
        assert!(matches!(reason(interval), UnsupportedCheck::Interval(_)), "{interval}");
    }
}

#[test]
fn float_casts_need_a_float_their_type_can_read() {
    let reason = |column_type: &str, literal: &str| {
        unsupported_reason(&format!(
            "CREATE TABLE t (id INT PRIMARY KEY, a {column_type}, CHECK (a < {literal}));"
        ))
    };
    for (column_type, literal) in [
        ("DOUBLE PRECISION", "'abc'::double precision"),
        ("DOUBLE PRECISION", "'1e400'::double precision"),
        ("DOUBLE PRECISION", "'-1e400'::float8"),
        ("REAL", "'1e39'::real"),
    ] {
        let quoted = literal.split("::").next().expect("A quoted literal");
        assert!(
            matches!(
                reason(column_type, literal),
                UnsupportedCheck::MixedTypes { right, .. } if right == quoted
            ),
            "{literal}"
        );
    }
    assert!(matches!(
        reason("DOUBLE PRECISION", "'Infinity'::real"),
        UnsupportedCheck::MixedTypes { left, right } if left == "f64" && right == "f32"
    ));
    for literal in ["'NaN'::double precision", "' -nan '::float8", "'nan'::real"] {
        let quoted = literal.split("::").next().expect("A quoted literal");
        assert!(
            matches!(
                reason("DOUBLE PRECISION", literal),
                UnsupportedCheck::NotANumber(nan) if nan == quoted
            ),
            "{literal}"
        );
    }
}

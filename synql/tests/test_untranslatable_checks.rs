//! Check constraints with SQL that has no Rust translation fail the
//! generation with an error naming the construct.

use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const FUNCTIONS: &str = "
CREATE FUNCTION is_lowercase(TEXT) RETURNS BOOLEAN AS 'SELECT $1 = lower($1)' LANGUAGE SQL;
CREATE FUNCTION not_mapped(TEXT) RETURNS BOOLEAN AS 'SELECT true' LANGUAGE SQL;
CREATE FUNCTION is_lowercase_shape(SHAPE) RETURNS BOOLEAN AS 'SELECT true' LANGUAGE SQL;
CREATE FUNCTION is_shorter(TEXT, TEXT) RETURNS BOOLEAN AS 'SELECT true' LANGUAGE SQL;
";

/// Generates a workspace for the table and returns the generation error.
fn generation_error(table: &str) -> Error {
    let db = ParserDB::parse::<PostgreSqlDialect>(&format!("{FUNCTIONS}{table}"))
        .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let checks = ExternalCrate::new("checks")
        .expect("A valid crate name")
        .functions([
            ExternalFunction::new("is_lowercase", syn::parse_quote!(::checks::is_lowercase)),
            ExternalFunction::new(
                "is_lowercase_shape",
                syn::parse_quote!(::checks::is_lowercase_shape),
            ),
            ExternalFunction::new("is_shorter", syn::parse_quote!(::checks::is_shorter)),
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
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (a + 1));"),
        UnsupportedCheck::Expression(_)
    ));
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (a IN (1, 2)));"),
        UnsupportedCheck::Expression(_)
    ));
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (a <> NULL));"),
        UnsupportedCheck::Expression(_)
    ));
}

#[test]
fn numbers_need_a_typed_operand() {
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (1 < 2 OR a > 0));"),
        UnsupportedCheck::UntypedNumber(number) if number == "1"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, flag BOOLEAN NOT NULL, CHECK (flag = 1));"
        ),
        UnsupportedCheck::MixedTypes { right, .. } if right == "1"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, a INT, name TEXT, CHECK (a + 1 < name));"
        ),
        UnsupportedCheck::MixedTypes { .. }
    ));
}

#[test]
fn operators_need_operands_of_their_kind() {
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, b INT, CHECK (a OR b));"),
        UnsupportedCheck::Operator(operator) if operator == "OR"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (name || 'x' = 'y'));"
        ),
        UnsupportedCheck::Operator(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (name + name = 'x'));"
        ),
        UnsupportedCheck::Operator(operator) if operator == "+"
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (CAST(a AS TEXT) = 'x'));"
        ),
        UnsupportedCheck::Cast(_)
    ));
}

#[test]
fn builtin_functions_need_their_operand_types() {
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (length(a) > 1));"),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (length(name, name) > 1));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason("CREATE TABLE t (id INT PRIMARY KEY, a INT, CHECK (now() > a));"),
        UnsupportedCheck::FunctionCall(_)
    ));
}

#[test]
fn validation_functions_need_plain_mapped_calls() {
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (is_lowercase(DISTINCT name)));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (is_lowercase(name) OVER ()));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (is_lowercase(name, name)));"
        ),
        UnsupportedCheck::FunctionCall(_)
    ));
    assert!(matches!(
        unsupported_reason(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (is_shorter(name, *)));"
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
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, CHECK (is_lowercase_shape(name)));"
        ),
        UnsupportedCheck::FunctionArgumentType { function, index: 0 } if function == "is_lowercase_shape"
    ));
}

//! A column whose SQL type has no Rust type in the workspace fails the code
//! generation with an error naming the column and its SQL type.

use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

/// Runs `generate` on the `body` column of a table under a workspace mapping
/// no SQL type, and asserts the error names the column and its type.
fn assert_type_not_found(
    generate: impl Fn(
        &<ParserDB as DatabaseLike>::Column,
        &Workspace,
        &ParserDB,
    ) -> Result<proc_macro2::TokenStream, Error>,
) {
    let db = ParserDB::parse::<PostgreSqlDialect>(
        "CREATE TABLE notes (id INT PRIMARY KEY, body TEXT NOT NULL DEFAULT 'x' CHECK (length(body) > 1));",
    )
    .expect("Unable to parse the schema");
    let table = db
        .table_by_target(TargetName::new("notes", false), IdentifierCase::AsWritten)
        .expect("Unable to look up the table")
        .expect("The table exists");
    let column = table
        .column("body", &db, IdentifierCase::AsWritten)
        .expect("Unable to look up the column")
        .expect("The column exists");
    let workspace_without_types: Workspace =
        Workspace::new().name("bare").expect("A valid workspace name").into();

    let error = generate(column, &workspace_without_types, &db)
        .expect_err("The column type has no Rust type in the workspace");
    assert!(
        matches!(
            &error,
            Error::ColumnTypeNotFound { table_name, column_name, sql_type }
                if table_name == "notes" && column_name == "body" && sql_type == "TEXT"
        ),
        "Unexpected error: {error:?}"
    );
}

#[test]
fn struct_fields_name_the_unmapped_type() {
    assert_type_not_found(ColumnSynLike::generate_struct_field);
}

#[test]
fn default_decorators_name_the_unmapped_type() {
    assert_type_not_found(ColumnSynLike::generate_default_decorator);
}

#[test]
fn validation_impls_name_the_unmapped_type() {
    assert_type_not_found(ColumnSynLike::generate_validation_impl);
}

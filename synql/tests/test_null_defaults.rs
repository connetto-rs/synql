//! A column whose default is the NULL literal, bare, parenthesised or cast,
//! generates no default, exactly like a column without `DEFAULT`.

use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

/// Asserts that every column but the primary key of the single table in
/// `schema` generates no default decorator.
fn assert_no_default_decorators(schema: &str) {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let workspace: Workspace =
        Workspace::new().name("nulls").expect("A valid workspace name").core().std().into();
    let table = db
        .table_by_target(TargetName::new("t", false), IdentifierCase::AsWritten)
        .expect("Unable to look up the table")
        .expect("The table exists");
    let columns = table.columns(&db).expect("Unable to look up the columns");
    for column in columns.filter(|column| column.column_name() != "id") {
        let decorator = column
            .generate_default_decorator(&workspace, &db)
            .expect("A NULL default needs no Rust type conversion");
        assert!(
            decorator.is_empty(),
            "Column `{}` got a default decorator: {decorator}",
            column.column_name()
        );
    }
}

#[test]
fn integer_null_defaults_generate_no_default() {
    assert_no_default_decorators(
        "CREATE TABLE t (id INT PRIMARY KEY, bare INT DEFAULT NULL, nested INT DEFAULT (NULL), casted INT DEFAULT NULL::integer);",
    );
}

#[test]
fn text_null_defaults_generate_no_default() {
    assert_no_default_decorators(
        "CREATE TABLE t (id INT PRIMARY KEY, bare TEXT DEFAULT NULL, nested TEXT DEFAULT (NULL), casted TEXT DEFAULT NULL::text);",
    );
}

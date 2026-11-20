//! A column of type `TIMESTAMP` without time zone or `TIME WITH TIME ZONE`, in
//! any spelling, fails the code generation with an error naming the column.

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

/// Generates a workspace for the schema and returns the error.
fn generation_error(schema: &str) -> Error {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let synql: SynQL<ParserDB> = SynQL::new(&db, temp_dir.path()).into();
    synql.generate().expect_err("The column type is refused")
}

#[test]
fn timestamp_columns_are_refused() {
    for schema in [
        "CREATE TABLE events (id INT PRIMARY KEY, at TIMESTAMP NOT NULL);",
        "CREATE TABLE events (id INT PRIMARY KEY, at TIMESTAMP WITHOUT TIME ZONE);",
        "CREATE TABLE events (id INT PRIMARY KEY, at pg_catalog.timestamp);",
    ] {
        let error = generation_error(schema);
        assert!(
            matches!(
                &error,
                Error::TimestampWithoutTimeZone { table_name, column_name }
                    if table_name == "events" && column_name == "at"
            ),
            "{schema}: unexpected error {error:?}"
        );
        assert!(error.to_string().contains("TIMESTAMPTZ"), "{error}");
    }
}

#[test]
fn time_with_time_zone_columns_are_refused() {
    for schema in [
        "CREATE TABLE openings (id INT PRIMARY KEY, opens TIMETZ NOT NULL);",
        "CREATE TABLE openings (id INT PRIMARY KEY, opens TIME WITH TIME ZONE);",
        "CREATE TABLE openings (id INT PRIMARY KEY, opens TIME(3) WITH TIME ZONE);",
    ] {
        let error = generation_error(schema);
        assert!(
            matches!(
                &error,
                Error::TimeWithTimeZone { table_name, column_name }
                    if table_name == "openings" && column_name == "opens"
            ),
            "{schema}: unexpected error {error:?}"
        );
        assert!(error.to_string().contains("TIMESTAMPTZ"), "{error}");
    }
}

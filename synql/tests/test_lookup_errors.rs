//! Schema lookups that fail during workspace generation are reported as
//! errors to the caller.

use sql_traits::{
    errors::{LookupError, ObjectKind},
    prelude::ParserDB,
};
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

#[test]
fn unknown_check_constraint_function_is_a_lookup_error() {
    let db = ParserDB::parse::<PostgreSqlDialect>(
        "CREATE TABLE notes (id INT PRIMARY KEY, body TEXT CHECK (no_such_function(body)));",
    )
    .expect("the schema parses");
    let temp_dir = tempfile::tempdir().expect("a temporary directory");
    let synql: SynQL<ParserDB> = SynQL::new(&db, temp_dir.path()).name("lookup_errors").into();

    let error = synql.generate().expect_err("an undeclared function cannot be translated");

    assert!(
        matches!(
            &error,
            Error::Lookup(LookupError::ObjectNotInDatabase {
                object_kind: ObjectKind::Function,
                object,
            }) if object == "`no_such_function`"
        ),
        "unexpected error: {error:?}"
    );
}

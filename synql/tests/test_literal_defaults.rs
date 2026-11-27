//! A column whose default is a quoted literal, bare, cast or parenthesised,
//! defaults to the value of its Rust type that the literal spells.

use quote::quote;
use sql_traits::prelude::*;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

/// Asserts that every column but the primary key of the single table in
/// `schema` generates the `expected` default decorator.
fn assert_default_decorators(schema: &str, expected: &proc_macro2::TokenStream) {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let workspace: Workspace = Workspace::new()
        .name("literals")
        .expect("A valid workspace name")
        .core()
        .std()
        .rosetta_uuid()
        .into();
    let table = db
        .table_by_target(TargetName::new("t", false), IdentifierCase::AsWritten)
        .expect("Unable to look up the table")
        .expect("The table exists");
    let columns = table.columns(&db).expect("Unable to look up the columns");
    for column in columns.filter(|column| column.column_name() != "id") {
        let decorator = column
            .generate_default_decorator(&workspace, &db)
            .expect("The column type has a Rust type");
        assert_eq!(
            decorator.to_string(),
            expected.to_string(),
            "Column `{}` got the wrong default decorator",
            column.column_name()
        );
    }
}

#[test]
fn quoted_uuid_defaults_generate_the_uuid() {
    assert_default_decorators(
        "CREATE TABLE t (
            id INT PRIMARY KEY,
            bare UUID NOT NULL DEFAULT 'bc3cf2e2-768c-4af9-bf86-65f23378a212',
            casted UUID NOT NULL DEFAULT 'bc3cf2e2-768c-4af9-bf86-65f23378a212'::uuid,
            nested UUID NOT NULL DEFAULT ('bc3cf2e2-768c-4af9-bf86-65f23378a212'::uuid),
            braced UUID NOT NULL DEFAULT '{BC3CF2E2-768C4AF9-BF8665F2-3378A212}'
        );",
        &quote! {
            #[table_model(default = ::rosetta_uuid::Uuid::from([
                0xbc, 0x3c, 0xf2, 0xe2, 0x76, 0x8c, 0x4a, 0xf9,
                0xbf, 0x86, 0x65, 0xf2, 0x33, 0x78, 0xa2, 0x12
            ]))]
        },
    );
}

#[test]
fn quoted_integer_defaults_generate_the_integer() {
    assert_default_decorators(
        "CREATE TABLE t (id INT PRIMARY KEY, bare INT NOT NULL DEFAULT '5', casted INT NOT NULL DEFAULT '5'::integer);",
        &quote! { #[table_model(default = 5i32)] },
    );
}

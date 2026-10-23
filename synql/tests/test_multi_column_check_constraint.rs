//! Test to verify that tables with composite multi-column check constraints
//! validate them on the whole record and compile.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

#[test]
fn test_multi_column_check_constraint() {
    let db = ParserDB::parse::<GenericDialect>(
        "
    CREATE TABLE test_table (
        id INT PRIMARY KEY,
        value1 INT,
        value2 INT,
        CHECK (value1 < value2)
    );
",
    )
    .expect("Unable to parse the schema");

    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_multi_column_check");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-multi-column-check")
        .generate_workspace_toml()
        .generate_rustfmt()
        .into();
    synql.generate().expect("Unable to generate workspace");

    let rs_path =
        workspace_path.join("synql-multi-column-check-test_table").join("src").join("lib.rs");
    let content = std::fs::read_to_string(&rs_path)
        .unwrap_or_else(|e| panic!("Could not read file at {rs_path:?}: {e}"));
    let normalized: String = content.chars().filter(|c| !c.is_whitespace()).collect();

    assert!(
        normalized.contains(
            "impl::diesel_builders::ValidateRecord<test_table::table>for<test_table::tableas::diesel_builders::TableExt>::NewValues"
        ),
        "Missing ValidateRecord impl. Found:\n{content}"
    );
    assert!(
        !normalized.contains("ValidateColumn<test_table::value1>"),
        "A column with only multi-column checks must not validate alone. Found:\n{content}"
    );
    assert!(normalized.contains("value1>=value2"), "Missing order check. Found:\n{content}");

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo check");
    assert!(
        output.status.success(),
        "cargo check failed for the generated workspace:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

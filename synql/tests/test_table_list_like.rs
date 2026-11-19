//! Test to verify that tables with an ancestor root referring to a
//! table-list-like table receive the `#[table_model(default(...))]` decorator,
//! while others do not.

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

#[test]
fn test_ancestral_table_list_default_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let db = ParserDB::parse::<GenericDialect>(
        "
    CREATE TABLE valid_list (name TEXT PRIMARY KEY);
    CREATE TABLE root (id INT PRIMARY KEY, list_name TEXT, FOREIGN KEY(list_name) REFERENCES valid_list(name));
    CREATE TABLE child (
        id INT PRIMARY KEY,
        FOREIGN KEY(id) REFERENCES root(id)
    );
    CREATE TABLE root_no_list (id INT PRIMARY KEY);
    CREATE TABLE child_no_list (id INT PRIMARY KEY REFERENCES root_no_list(id));
",
    )?;

    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_table_list");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-table-list")
        .generate_workspace_toml()
        .generate_rustfmt()
        .one_crate_per_table()
        .into();
    synql.generate().expect("Unable to generate workspace");

    // Case 1: Child of Root with Table List
    // Crate: synql-table-list-child
    let child_crate_name = "synql-table-list-child";
    let child_rs_path = workspace_path.join(child_crate_name).join("src").join("lib.rs");

    let content = std::fs::read_to_string(&child_rs_path)
        .unwrap_or_else(|e| panic!("Could not read file at {child_rs_path:?}: {e}"));

    // Normalized check
    let normalized: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    // Expected: #[table_model(default(synql_table_list_root::root::list_name,"
    // child"))] The crate name "synql-table-list-root" becomes
    // "synql_table_list_root" as identifier.

    let expected = "default(synql_table_list_root::root::list_name,\"child\")";
    assert!(
        normalized.contains(expected),
        "Should contain default decorator for table list. Found:\n{content}"
    );

    // Case 2: Child No List
    // The table name is `child_no_list`. The snake case is `child_no_list`.
    // The crate name combines workspace name "synql-table-list" with table
    // name. So "synql-table-list-child_no_list".
    let child_no_list_crate_name = "synql-table-list-child_no_list";
    let child_no_list_rs_path =
        workspace_path.join(child_no_list_crate_name).join("src").join("lib.rs");
    let content_no_list = std::fs::read_to_string(&child_no_list_rs_path)
        .unwrap_or_else(|e| panic!("Could not read file at {child_no_list_rs_path:?}: {e}"));

    let normalized_no_list: String =
        content_no_list.chars().filter(|c| !c.is_whitespace()).collect();
    // Just ensure it doesn't have "default(" related to table model
    let unexpected = "default(";
    assert!(
        !normalized_no_list.contains(unexpected),
        "Should NOT contain default decorator. Found:\n{content_no_list}",
    );

    Ok(())
}

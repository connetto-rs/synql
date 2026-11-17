//! Test module to verify the callback functionality in SynQL.

use quote::quote;
use sql_traits::prelude::ParserDB;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

#[test]
fn test_callback_generation() -> Result<(), Box<dyn std::error::Error>> {
    let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id SERIAL PRIMARY KEY);")?;
    let temp_dir = tempfile::tempdir()?;
    let workspace_path = temp_dir.path().join("synql_workspace");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .callback(|_table, _db, _workspace| {
            Ok(Some(quote! {
                pub fn hello_world() -> &'static str {
                    "Hello, World!"
                }
            }))
        })
        .toml_callback(|_table, _db| Ok(Some(TomlDependency::new("extra-dep").version("1.0.0")?)))
        .into();

    synql.generate()?;

    let mut lib_rs_path = workspace_path.join("unknown_catalog-users").join("src").join("lib.rs");

    if !lib_rs_path.exists() {
        // Fallback: look for directory ending with users
        for entry in std::fs::read_dir(&workspace_path)? {
            let path = entry?.path();
            if path.is_dir()
                && path.file_name().unwrap_or_default().to_string_lossy().ends_with("users")
            {
                lib_rs_path = path.join("src").join("lib.rs");
                break;
            }
        }
    }

    let content = std::fs::read_to_string(lib_rs_path.clone())?;
    assert!(content.contains("pub fn hello_world"));

    let cargo_toml_path = lib_rs_path.parent().unwrap().parent().unwrap().join("Cargo.toml");
    let content = std::fs::read_to_string(cargo_toml_path)?;
    assert!(content.contains("extra-dep = { version = \"1.0.0\" }"));

    Ok(())
}

#[test]
fn sink_callbacks_extend_only_the_sink_crate() -> Result<(), Box<dyn std::error::Error>> {
    use std::cell::RefCell;

    use sql_traits::prelude::{DatabaseLike, IdentifierCase, TableLike, TargetName};

    let db = ParserDB::parse::<GenericDialect>(
        "
        CREATE TABLE users (id INT PRIMARY KEY, name TEXT NOT NULL);
        CREATE TABLE comments (id INT PRIMARY KEY, user_id INT NOT NULL REFERENCES users(id));
        CREATE TABLE extended_comments (id INT PRIMARY KEY REFERENCES comments(id));
        CREATE TABLE isolated (id INT PRIMARY KEY, data TEXT);
        ",
    )?;
    let isolated = db
        .table_by_target(TargetName::new("isolated", false), IdentifierCase::AsWritten)?
        .expect("isolated exists");
    let temp_dir = tempfile::tempdir()?;
    let workspace_path = temp_dir.path().join("synql_sink_callback");
    let sink_callback_tables: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-sink-callback")
        .deny(isolated)
        .sink_crate("everything")
        .dag_sink_crate_prefix("dag_")
        .sink_callback(|tables, _db, _workspace| {
            sink_callback_tables
                .borrow_mut()
                .push(tables.iter().map(|table| table.table_name().to_owned()).collect());
            Ok(Some(quote! {
                /// Hook added by the sink callback.
                pub fn sink_hook() {}
            }))
        })
        .sink_toml_callback(|_db| Ok(Some(TomlDependency::new("procedure-like").version("1.0.0")?)))
        .sink_toml_callback(|_db| Ok(Some(TomlDependency::new("procedure-like").version("1.0.0")?)))
        .into();
    synql.generate()?;
    drop(synql);

    let mut seen_tables = sink_callback_tables.into_inner();
    for tables in &mut seen_tables {
        tables.sort_unstable();
    }
    assert_eq!(
        seen_tables,
        vec![vec!["comments".to_owned(), "extended_comments".to_owned(), "users".to_owned()]]
    );

    let sink_path = workspace_path.join("everything");
    let sink_lib = std::fs::read_to_string(sink_path.join("src").join("lib.rs"))?;
    assert!(sink_lib.contains("pub fn sink_hook"));
    let sink_toml = std::fs::read_to_string(sink_path.join("Cargo.toml"))?;
    assert_eq!(sink_toml.matches("procedure-like = { version = \"1.0.0\" }").count(), 1);

    let dag_sink_path = workspace_path.join("dag_comments");
    let dag_sink_lib = std::fs::read_to_string(dag_sink_path.join("src").join("lib.rs"))?;
    assert!(!dag_sink_lib.contains("sink_hook"));
    let dag_sink_toml = std::fs::read_to_string(dag_sink_path.join("Cargo.toml"))?;
    assert!(!dag_sink_toml.contains("procedure-like"));

    Ok(())
}

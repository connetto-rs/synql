//! Test module to verify DAG sink generation.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

#[test]
fn test_dag_sink_generation() -> Result<(), Box<dyn std::error::Error>> {
    let db = ParserDB::parse::<GenericDialect>(
        "
		CREATE TABLE users (
		    id SERIAL PRIMARY KEY,
		    name TEXT NOT NULL
		);
        CREATE TABLE comments (
		    id SERIAL PRIMARY KEY,
		    comment TEXT NOT NULL,
            user_id INT REFERENCES users(id)
		);
        CREATE TABLE extended_comments (
            id INT PRIMARY KEY REFERENCES comments(id),
            extra_info TEXT
        );
        CREATE TABLE isolated (
            id SERIAL PRIMARY KEY,
            data TEXT
        );
",
    )?;
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_dag_workspace");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-dag-workspace")
        .generate_workspace_toml()
        .dag_sink_crate_prefix("sink_")
        .one_crate_per_table()
        .into();

    synql.generate().expect("Unable to generate workspace");

    // Verify that the sink_comments crate was created
    let sink_comments_path = workspace_path.join("sink_comments");
    assert!(sink_comments_path.exists(), "sink_comments crate should be created");

    // Verify sink_comments Cargo.toml dependencies
    let sink_comments_toml = std::fs::read_to_string(sink_comments_path.join("Cargo.toml"))?;

    // Check what is included
    assert!(
        sink_comments_toml.contains("synql-dag-workspace-comments.workspace = true"),
        "sink_comments should depend on comments"
    );
    assert!(
        sink_comments_toml.contains("synql-dag-workspace-extended_comments.workspace = true"),
        "sink_comments should depend on extended_comments"
    );
    // Users is an ancestor, not a descendant, so it might not be included if
    // logic is strictly "depends_on root"
    assert!(
        !sink_comments_toml.contains("synql-dag-workspace-users.workspace = true"),
        "sink_comments should NOT depend on users (ancestor)"
    );

    // Verify sink_comments lib.rs
    let sink_comments_lib = std::fs::read_to_string(sink_comments_path.join("src/lib.rs"))?;

    assert!(
        sink_comments_lib.contains("pub use synql_dag_workspace_comments"),
        "Should re-export comments"
    );
    assert!(
        sink_comments_lib.contains("pub use synql_dag_workspace_extended_comments"),
        "Should re-export extended_comments"
    );

    // Verify that the generated workspace can be checked
    let output = Command::new("cargo").arg("check").current_dir(&workspace_path).output()?;

    if !output.status.success() {
        eprintln!("cargo check stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cargo check stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cargo check failed for generated workspace");
    }

    Ok(())
}

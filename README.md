# SynQL

[![CI](https://github.com/connetto-rs/synql/workflows/Rust%20CI/badge.svg)](https://github.com/connetto-rs/synql/actions)
[![Security Audit](https://github.com/connetto-rs/synql/workflows/Security%20Audit/badge.svg)](https://github.com/connetto-rs/synql/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Codecov](https://codecov.io/gh/connetto-rs/synql/branch/main/graph/badge.svg)](https://codecov.io/gh/connetto-rs/synql)

SynQL is a library designed to generate a modular Cargo workspace from a SQL schema. SynQL generates a separate Type Safe Rust crate for each table in your database.

This architectural choice enables:

- **Parallel Compilation**: Crates are compiled following the dependency graph, allowing for maximum parallel compilation. There is no monolithic crate that becomes a bottleneck during compilation.
- **Granular Dependencies**: Consumers only import the specific table crates they need.
- **Unified Access**: Optionally generates a 'sink' crate that re-exports all table crates, allowing consumption via a single dependency in projects that need access to the entire schema.
- **Micro-crate Architecture**: Facilitates maintenance of large schemas by isolating changing components.

## Features

- **Advanced ORM Capabilities**: The generated models utilize the [diesel-builders](https://github.com/connetto-rs/diesel-builders/) crate, providing a sophisticated builder pattern, handling complex table relationships (DAGs, inheritance), and enforcing compile-time correctness for data insertion.
- **Type Safety**: Maps SQL types to Rust types with high fidelity.
- **SQL Relations**: SynQL builds upon the `sql_relations` crate, which extends standard foreign key introspection with semantic "Same As" topology. It identifies complex patterns like **Vertical Same As** (inheritance-like redundancy), **Horizontal Same As** (sibling table equivalence), and **Triangular Same As** (diamond dependency consistency), allowing the generated code to enforce deeper data integrity constraints.

## Use Cases

SynQL is versatile and powers different types of schema generation:

- **Diesel/Postgres Backend**: Generates fully typed Rust models compatible with [Diesel](https://diesel.rs/).
  - **Example**: [directus-schema-models](https://github.com/earth-metabolome-initiative/directus-schema-models)
  - This project uses SynQL to generate models from a legacy Directus database, enabling migration to a new architecture with minimal non-derived code.

- **SQL Documents & Shared Schemas**: Creates strongly typed Rust structures for defined SQL documents or shared schemas.
  - **Example**: [asset-procedure-schema](https://github.com/earth-metabolome-initiative/asset-procedure-schema)
  - This project uses SynQL to define a shared generated workspace of models that can be reused across multiple different projects.

## Generating from SQL Documents

This example shows how to parse SQL files directly (using `ParserDB` from `sql-traits`) to generate strongly typed Rust structs.

```rust
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use sql_traits::prelude::ParserDB;
use sqlparser::dialect::GenericDialect;
use synql::prelude::*;

// Setup a temporary directory with a SQL file
let dir = tempdir().unwrap();
let file_path = dir.path().join("model.sql");
let mut file = File::create(&file_path).unwrap();
writeln!(file, "CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT);").unwrap();

// Read the SQL content from the file
let sql = std::fs::read_to_string(&file_path).expect("Failed to read SQL file");

// Parse the SQL string
let db = ParserDB::parse::<GenericDialect>(&sql).expect("Failed to parse database schema");

// Generate to a temporary output path
let output_dir = tempdir().unwrap();

let synql: SynQL<ParserDB> = SynQL::new(&db, output_dir.path())
 .name("document_schema")
 .generate_workspace_toml()
 .into();
 
synql.generate().expect("Unable to generate workspace");
```

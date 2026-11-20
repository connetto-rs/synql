//! A timestamp column defaulting to the current time, spelled `now()`,
//! `transaction_timestamp()` or `CURRENT_TIMESTAMP` in any case, gets the
//! current time from the builder, a date column defaulting to `CURRENT_DATE`
//! gets the current UTC date, and a time column defaulting to `LOCALTIME` gets
//! the current UTC time of day.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated builders, and runs the assertions with `cargo test`.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE TABLE events (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE stamps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shouted TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    transaction TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    keyword TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    lowercase_keyword TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    parenthesised TIMESTAMPTZ NOT NULL DEFAULT ((now()))
);
CREATE TABLE openings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    day DATE NOT NULL DEFAULT CURRENT_DATE,
    opens TIME NOT NULL DEFAULT LOCALTIME,
    lasts INTERVAL NOT NULL
);
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel-builders.workspace = true
rosetta-utc.workspace = true
synql-current-time-events.workspace = true
synql-current-time-stamps.workspace = true
synql-current-time-openings.workspace = true
rosetta-time.workspace = true
jiff = "1"
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over the generated current time defaults.

use diesel_builders::{BuildableTable, MayGetColumn};
use rosetta_time::{Date, Time};
use rosetta_utc::TimestampUTC;
use synql_current_time_events::events;
use synql_current_time_openings::openings;
use synql_current_time_stamps::stamps;

/// Asserts that the builder filled the column with a time between the two
/// bounds.
fn assert_between(timestamp: Option<TimestampUTC>, before: TimestampUTC, after: TimestampUTC, column: &str) {
    let timestamp = timestamp.unwrap_or_else(|| panic!("The builder leaves `{column}` unset"));
    assert!(before <= timestamp && timestamp <= after, "`{column}` is {timestamp:?}, outside {before:?} and {after:?}");
}

#[test]
fn the_builder_fills_the_at_column() {
    let before = TimestampUTC::now();
    let builder = events::table::builder();
    let after = TimestampUTC::now();
    assert_between(MayGetColumn::<events::at>::may_get_column(&builder), before, after, "at");
}

#[test]
fn every_spelling_of_the_current_time_fills_its_column() {
    let before = TimestampUTC::now();
    let builder = stamps::table::builder();
    let after = TimestampUTC::now();
    assert_between(MayGetColumn::<stamps::shouted>::may_get_column(&builder), before, after, "shouted");
    assert_between(MayGetColumn::<stamps::transaction>::may_get_column(&builder), before, after, "transaction");
    assert_between(MayGetColumn::<stamps::keyword>::may_get_column(&builder), before, after, "keyword");
    assert_between(MayGetColumn::<stamps::lowercase_keyword>::may_get_column(&builder), before, after, "lowercase_keyword");
    assert_between(MayGetColumn::<stamps::parenthesised>::may_get_column(&builder), before, after, "parenthesised");
}

#[test]
fn the_current_date_and_time_of_day_fill_their_columns() {
    let before = TimestampUTC::now();
    let builder = openings::table::builder();
    let after = TimestampUTC::now();
    let day: Date = MayGetColumn::<openings::day>::may_get_column(&builder).expect("The builder leaves `day` unset");
    let opens: Time = MayGetColumn::<openings::opens>::may_get_column(&builder).expect("The builder leaves `opens` unset");
    let utc = |timestamp: TimestampUTC| jiff::tz::Offset::UTC.to_datetime(*timestamp);
    let (before, after) = (utc(before), utc(after));
    let filled = day.to_datetime(*opens);
    assert!(before <= filled && filled <= after, "`day` and `opens` are {filled}, outside {before} and {after}");
}
"#;

#[test]
fn test_current_time_defaults() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_current_time");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-current-time")
        .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/defaults.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over the generated defaults.\n"),
    ] {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    let output = Command::new("cargo")
        .args(["test", "-p", "assertions"])
        .env("RUSTFLAGS", "-D warnings")
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo test");
    assert!(
        output.status.success(),
        "The generated defaults failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

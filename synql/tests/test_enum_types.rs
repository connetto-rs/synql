//! PostgreSQL enum types become Rust enums, ordered as PostgreSQL orders their
//! labels and converted from and to the labels, which columns, defaults and
//! check constraints of the type use.
//!
//! The test generates a workspace, adds a crate of assertions over the
//! generated code, and runs the assertions with `cargo test`.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE SCHEMA authz;
CREATE TYPE authz.audit_operation AS ENUM ('INSERT', 'UPDATE', 'DELETE');
CREATE TYPE mood AS ENUM ('sad', 'so-so', 'happy', '1st', '', 'Self', 'a_b', 'a-b');
CREATE TABLE audit_events (
    id INT PRIMARY KEY,
    operation authz.audit_operation NOT NULL,
    old_row TEXT,
    new_row TEXT,
    CHECK ((operation = 'INSERT') = (old_row IS NULL) AND (operation = 'DELETE') = (new_row IS NULL))
);
CREATE TABLE people (
    id INT PRIMARY KEY,
    mood mood NOT NULL DEFAULT 'so-so'::mood CHECK (mood NOT IN ('1st', '')),
    previous public.mood CHECK (previous IN ('sad', 'happy')),
    CHECK (previous IS NULL OR mood >= previous)
);
CREATE TABLE moods_after_sadness (
    id INT PRIMARY KEY,
    mood mood NOT NULL CHECK (mood > 'sad')
);
";

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel = { workspace = true, features = ["postgres_backend", "i-implement-a-third-party-backend-and-opt-into-breaking-changes"] }
diesel-builders.workspace = true
serde_json.workspace = true
validation-errors.workspace = true
synql-enum-types-authz-audit_operation.workspace = true
synql-enum-types-public-mood.workspace = true
synql-enum-types-audit_events.workspace = true
synql-enum-types-people.workspace = true
synql-enum-types-moods_after_sadness.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over the generated enums.

use std::num::NonZeroU32;

use diesel::{
    deserialize::FromSql,
    pg::{Pg, PgMetadataLookup, PgTypeMetadata, PgValue},
    query_builder::bind_collector::RawBytesBindCollector,
    query_builder::BindCollector,
    serialize::ToSql,
    sql_types::HasSqlType,
};
use diesel_builders::{MayGetColumn, SetColumn, TableExt, ValidateColumn, ValidateRecord};
use synql_enum_types_authz_audit_operation::{AuditOperation, sql_types};
use synql_enum_types_public_mood::Mood;
use validation_errors::{ValidationError, ValidationErrorKind};

/// Records the type names diesel looks up.
struct RecordingLookup(Vec<(String, Option<String>)>);

impl PgMetadataLookup for RecordingLookup {
    fn lookup_type(&mut self, type_name: &str, schema: Option<&str>) -> PgTypeMetadata {
        self.0.push((type_name.to_owned(), schema.map(str::to_owned)));
        PgTypeMetadata::new(1, 2)
    }
}

/// Returns the bytes diesel sends for the value and the type names it looks
/// up to send them.
fn bind<T, U: ToSql<T, Pg>>(value: &U) -> (Vec<u8>, Vec<(String, Option<String>)>)
where
    Pg: HasSqlType<T>,
{
    let mut lookup = RecordingLookup(Vec::new());
    let mut collector = RawBytesBindCollector::<Pg>::new();
    collector.push_bound_value::<T, U>(value, &mut lookup).expect("The value serializes");
    let bytes = collector.binds.pop().flatten().expect("The value is not NULL");
    (bytes, lookup.0)
}

/// Returns the value diesel reads from the bytes.
fn read<T, U: FromSql<T, Pg>>(bytes: &[u8]) -> diesel::deserialize::Result<U> {
    let oid = || NonZeroU32::MIN;
    U::from_sql(PgValue::new(bytes, &oid))
}

fn is_check_violation(result: Result<(), ValidationError>) -> bool {
    matches!(result, Err(error) if matches!(error.kind(), ValidationErrorKind::CheckViolated { .. }))
}

#[test]
fn enums_follow_the_order_of_their_labels() {
    assert!(Mood::Sad < Mood::SoSo);
    assert!(Mood::SoSo < Mood::Happy);
    assert!(Mood::Happy < Mood::Label1st);
    assert!(AuditOperation::Insert < AuditOperation::Delete);
}

#[test]
fn labels_that_are_no_identifiers_name_their_variants() {
    let variants =
        [Mood::Label1st, Mood::Label, Mood::LabelSelf, Mood::AB, Mood::AB2].map(|mood| {
            serde_json::to_string(&mood).expect("The variant serializes")
        });
    assert_eq!(variants, ["\"1st\"", "\"\"", "\"Self\"", "\"a_b\"", "\"a-b\""]);
    assert_eq!(serde_json::from_str::<Mood>("\"so-so\"").expect("A label"), Mood::SoSo);
}

#[test]
fn postgres_receives_and_sends_the_labels() {
    let (bytes, lookups) = bind::<sql_types::AuditOperation, _>(&AuditOperation::Delete);
    assert_eq!(bytes, b"DELETE");
    assert_eq!(lookups, [("audit_operation".to_owned(), Some("authz".to_owned()))]);
    let (bytes, lookups) =
        bind::<synql_enum_types_public_mood::sql_types::Mood, _>(&Mood::AB2);
    assert_eq!(bytes, b"a-b");
    assert_eq!(lookups, [("mood".to_owned(), None)]);

    let read_operation = read::<sql_types::AuditOperation, AuditOperation>(b"UPDATE");
    assert_eq!(read_operation.expect("A label"), AuditOperation::Update);
    let read_mood = read::<synql_enum_types_public_mood::sql_types::Mood, Mood>(b"");
    assert_eq!(read_mood.expect("A label"), Mood::Label);
    assert!(read::<sql_types::AuditOperation, AuditOperation>(b"insert").is_err());
}

#[test]
fn checks_compare_enum_columns_with_labels() {
    use synql_enum_types_audit_events::audit_events;
    let validate = |operation: AuditOperation, old_row: Option<&str>, new_row: Option<&str>| {
        let mut values = audit_events::table::empty_new_values();
        SetColumn::<audit_events::operation>::set_column(&mut values, operation);
        SetColumn::<audit_events::old_row>::set_column(&mut values, old_row.map(str::to_owned));
        SetColumn::<audit_events::new_row>::set_column(&mut values, new_row.map(str::to_owned));
        ValidateRecord::<audit_events::table>::validate_record(&values)
    };
    assert!(validate(AuditOperation::Insert, None, Some("new")).is_ok());
    assert!(validate(AuditOperation::Update, Some("old"), Some("new")).is_ok());
    assert!(validate(AuditOperation::Delete, Some("old"), None).is_ok());
    assert!(is_check_violation(validate(AuditOperation::Insert, Some("old"), Some("new"))));
    assert!(is_check_violation(validate(AuditOperation::Update, Some("old"), None)));
}

#[test]
fn checks_test_enum_columns_against_label_lists() {
    use synql_enum_types_people::people;
    type People = <people::table as TableExt>::NewValues;
    assert!(<People as ValidateColumn<people::mood>>::validate_column(&Mood::Happy).is_ok());
    assert!(is_check_violation(<People as ValidateColumn<people::mood>>::validate_column(
        &Mood::Label1st
    )));
    assert!(is_check_violation(<People as ValidateColumn<people::mood>>::validate_column(
        &Mood::Label
    )));
    assert!(<People as ValidateColumn<people::previous>>::validate_column(&Mood::Sad).is_ok());
    assert!(is_check_violation(<People as ValidateColumn<people::previous>>::validate_column(
        &Mood::SoSo
    )));

    let validate = |mood: Mood, previous: Option<Mood>| {
        let mut values = people::table::empty_new_values();
        SetColumn::<people::mood>::set_column(&mut values, mood);
        SetColumn::<people::previous>::set_column(&mut values, previous);
        ValidateRecord::<people::table>::validate_record(&values)
    };
    assert!(validate(Mood::Sad, None).is_ok());
    assert!(validate(Mood::Happy, Some(Mood::Sad)).is_ok());
    assert!(is_check_violation(validate(Mood::Sad, Some(Mood::Happy))));
}

#[test]
fn checks_order_enum_columns_against_labels() {
    use synql_enum_types_moods_after_sadness::moods_after_sadness;
    type Moods = <moods_after_sadness::table as TableExt>::NewValues;
    assert!(<Moods as ValidateColumn<moods_after_sadness::mood>>::validate_column(&Mood::SoSo).is_ok());
    assert!(is_check_violation(
        <Moods as ValidateColumn<moods_after_sadness::mood>>::validate_column(&Mood::Sad)
    ));
}

#[test]
fn defaults_take_the_variant_of_their_label() {
    use synql_enum_types_people::people;
    let values = people::table::default_new_values();
    assert_eq!(MayGetColumn::<people::mood>::may_get_column(&values), Some(Mood::SoSo));
}
"#;

#[test]
fn test_enum_types() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_enum_types");

    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-enum-types")
        .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/enums.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over the generated enums.\n"),
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
        "The generated enums failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_crate_mapping_the_type_keeps_precedence() {
    let db = ParserDB::parse::<PostgreSqlDialect>(
        "CREATE TYPE mood AS ENUM ('sad', 'happy'); CREATE TABLE people (id INT PRIMARY KEY, mood mood NOT NULL);",
    )
    .expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let moods = ExternalCrate::new("moods")
        .expect("A valid crate name")
        .add_type(
            ExternalType::new(
                syn::parse_quote!(::moods::sql_types::Mood),
                syn::parse_quote!(::moods::Mood),
            )
            .postgres_type("mood")
            .expect("A lowercase type name")
            .supports_copy()
            .into(),
        )
        .expect("A single type")
        .into();
    let synql: SynQL<ParserDB> = SynQL::new(&db, temp_dir.path())
        .name("moody")
        .external_crate(moods)
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    assert!(!temp_dir.path().join("moody-public-mood").exists());
    let people_manifest = std::fs::read_to_string(temp_dir.path().join("moody-people/Cargo.toml"))
        .expect("The table crate has a manifest");
    assert!(people_manifest.contains("moods.workspace = true"), "{people_manifest}");
    let people_lib = std::fs::read_to_string(temp_dir.path().join("moody-people/src/lib.rs"))
        .expect("The table crate has a library");
    assert!(people_lib.contains(":: moods :: Mood"), "{people_lib}");
}

/// Generates the workspace of the schema and returns the generation result.
fn generate(name: &str, schema: &str) -> (tempfile::TempDir, Result<(), Error>) {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let synql: SynQL<ParserDB> =
        SynQL::new(&db, temp_dir.path()).name(name).generate_workspace_toml().into();
    let result = synql.generate().map(|_| ());
    drop(synql);
    (temp_dir, result)
}

#[test]
fn enum_types_sharing_a_crate_name_are_refused() {
    let (_temp_dir, result) = generate(
        "clash",
        "CREATE TYPE \"Mood\" AS ENUM ('a'); CREATE TYPE mood AS ENUM ('b'); CREATE TABLE t (id INT PRIMARY KEY, x mood);",
    );
    let error = result.expect_err("Both types fold to one crate name");
    assert!(
        matches!(
            &error,
            Error::CrateNameClash { first, second, crate_ident }
                if first == "enum type `public.Mood`" && second == "enum type `public.mood`" && crate_ident == "clash_public_mood"
        ),
        "Unexpected error: {error:?}"
    );
}

#[test]
fn an_enum_type_sharing_the_crate_of_a_table_is_refused() {
    let (_temp_dir, result) = generate(
        "clash",
        "CREATE TYPE mood AS ENUM ('a'); CREATE TABLE public_mood (id INT PRIMARY KEY, x mood);",
    );
    let error = result.expect_err("The table crate and the enum crate fold to one Rust crate");
    assert!(
        matches!(
            &error,
            Error::CrateNameClash { first, second, crate_ident }
                if first == "table `public_mood`" && second == "enum type `public.mood`" && crate_ident == "clash_public_mood"
        ),
        "Unexpected error: {error:?}"
    );
}

#[test]
fn a_default_that_is_no_label_is_unsupported() {
    let (_temp_dir, result) = generate(
        "defaults",
        "CREATE TYPE mood AS ENUM ('a'); CREATE FUNCTION first_mood() RETURNS mood AS 'SELECT ''a''::mood' LANGUAGE SQL; CREATE TABLE t (id INT PRIMARY KEY, x mood DEFAULT first_mood());",
    );
    let error = result.expect_err("A call is no label");
    assert!(
        matches!(
            &error,
            Error::UnsupportedDefault { table_name, column_name, default }
                if table_name == "t" && column_name == "x" && default == "first_mood()"
        ),
        "Unexpected error: {error:?}"
    );
}

#[test]
fn a_schema_of_types_alone_generates_the_enum_crates() {
    let (temp_dir, result) =
        generate("alone", "CREATE TYPE pair AS (a INT, b INT); CREATE TYPE mood AS ENUM ('a');");
    result.expect("Unable to generate workspace");
    let manifest = std::fs::read_to_string(temp_dir.path().join("Cargo.toml"))
        .expect("The workspace has a manifest");
    assert!(manifest.contains("members = [\"./alone-public-mood\"]"), "{manifest}");
    assert!(!temp_dir.path().join("alone-public-pair").exists());
}

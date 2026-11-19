//! Test module to verify horizontal same-as foreign keys wider than two
//! columns.

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

/// Generates the workspace of `schema` and returns the whitespace-free source
/// of its `disposals` crate.
fn disposals_source(schema: &str) -> String {
    let db = ParserDB::parse::<PostgreSqlDialect>(schema).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let synql: SynQL<ParserDB> =
        SynQL::new(&db, temp_dir.path()).name("sameas").one_crate_per_table().into();
    synql.generate().expect("Unable to generate workspace");
    let source = std::fs::read_to_string(
        temp_dir.path().join("sameas-disposals").join("src").join("lib.rs"),
    )
    .expect("Unable to read the disposals crate");
    source.chars().filter(|character| !character.is_whitespace()).collect()
}

const FOUR_COLUMN_KEY: &str = "#[table_model(foreign_key((slot_id,id,model_id,role_id,),(::sameas_slots::slots::id,::sameas_slots::slots::template_id,::sameas_slots::slots::model_id,::sameas_slots::slots::role_id)))]";
const THREE_COLUMN_KEY: &str = "#[table_model(foreign_key((slot_id,id,model_id,),(::sameas_slots::slots::id,::sameas_slots::slots::template_id,::sameas_slots::slots::model_id)))]";

/// Generates a `disposals` table extending `templates` and tied to its slot
/// by `foreign_keys`, and asserts the decorators of its columns and that its
/// explicit slot keys are exactly `explicit_keys`.
fn assert_disposal_decorators(slot_id: &str, foreign_keys: &str, explicit_keys: &[&str]) {
    let source = disposals_source(&format!(
        "
CREATE TABLE templates (id INT PRIMARY KEY);
CREATE TABLE models (id INT PRIMARY KEY);
CREATE TABLE roles (id INT PRIMARY KEY);
CREATE TABLE slots (
    id INT PRIMARY KEY,
    template_id INT NOT NULL REFERENCES templates (id),
    model_id INT NOT NULL REFERENCES models (id),
    role_id INT NOT NULL REFERENCES roles (id),
    UNIQUE (id, template_id, model_id),
    UNIQUE (id, template_id, model_id, role_id)
);
CREATE TABLE disposals (
    id INT PRIMARY KEY REFERENCES templates (id),
    {slot_id},
    model_id INT NOT NULL REFERENCES models (id),
    role_id INT NOT NULL REFERENCES roles (id),
    {foreign_keys}
);"
    ));
    for (decorator, field) in [
        ("#[same_as(sameas_slots::slots::template_id,slot_id)]", "id:i32"),
        ("#[mandatory(sameas_slots::slots)]", "slot_id:i32"),
        ("#[same_as(sameas_slots::slots::model_id,slot_id)]", "model_id:i32"),
        ("#[same_as(sameas_slots::slots::role_id,slot_id)]", "role_id:i32"),
    ] {
        assert!(
            source.contains(&format!("{decorator}{field}")),
            "Expected `{decorator}` on `{field}` in:\n{source}"
        );
        assert_eq!(
            source.matches(decorator).count(),
            1,
            "Expected one `{decorator}` in:\n{source}"
        );
    }
    assert_eq!(
        source.matches("#[mandatory(").count(),
        1,
        "Expected one mandatory key in:\n{source}"
    );
    for explicit_key in explicit_keys {
        assert!(source.contains(explicit_key), "Expected `{explicit_key}` in:\n{source}");
    }
    assert_eq!(
        source.matches("foreign_key((slot_id,").count(),
        explicit_keys.len(),
        "Unexpected explicit slot keys in:\n{source}"
    );
}

#[test]
fn every_host_column_of_a_four_column_key_is_same_as_its_satellite_column() {
    assert_disposal_decorators(
        "slot_id INT NOT NULL REFERENCES slots (id)",
        "FOREIGN KEY (slot_id, id, model_id, role_id)
            REFERENCES slots (id, template_id, model_id, role_id)",
        &[FOUR_COLUMN_KEY],
    );
}

#[test]
fn the_key_is_the_column_paired_with_the_satellite_primary_key_wherever_it_sits() {
    assert_disposal_decorators(
        "slot_id INT NOT NULL REFERENCES slots (id)",
        "FOREIGN KEY (id, model_id, slot_id, role_id)
            REFERENCES slots (template_id, model_id, id, role_id)",
        &[FOUR_COLUMN_KEY],
    );
}

#[test]
fn a_composite_key_alone_references_the_satellite_through_its_key_column() {
    assert_disposal_decorators(
        "slot_id INT NOT NULL",
        "FOREIGN KEY (id, model_id, slot_id, role_id)
            REFERENCES slots (template_id, model_id, id, role_id)",
        &[FOUR_COLUMN_KEY],
    );
}

#[test]
fn composite_keys_sharing_their_key_column_make_one_triangle() {
    assert_disposal_decorators(
        "slot_id INT NOT NULL",
        "FOREIGN KEY (slot_id, id, model_id) REFERENCES slots (id, template_id, model_id),
        FOREIGN KEY (slot_id, id, model_id, role_id)
            REFERENCES slots (id, template_id, model_id, role_id)",
        &[THREE_COLUMN_KEY, FOUR_COLUMN_KEY],
    );
}

#[test]
fn a_horizontal_key_outside_any_triangle_is_kept_in_the_index_column_order() {
    let source = disposals_source(
        "
CREATE TABLE models (id INT PRIMARY KEY);
CREATE TABLE slots (
    id INT PRIMARY KEY,
    model_id INT NOT NULL REFERENCES models (id),
    role TEXT NOT NULL,
    UNIQUE (id, model_id, role)
);
CREATE TABLE disposals (
    id INT PRIMARY KEY,
    slot_id INT NOT NULL,
    model_id INT NOT NULL REFERENCES models (id),
    role TEXT NOT NULL,
    FOREIGN KEY (model_id, slot_id, role) REFERENCES slots (model_id, id, role)
);",
    );

    let expected = "#[table_model(foreign_key((slot_id,model_id,role,),(::sameas_slots::slots::id,::sameas_slots::slots::model_id,::sameas_slots::slots::role)))]";
    assert!(source.contains(expected), "Expected `{expected}` in:\n{source}");
    assert!(!source.contains("same_as("), "Unexpected same-as decorator in:\n{source}");
}

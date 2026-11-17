//! Foreign keys that look like same-as relationships but are not.

use sql_relations::prelude::*;
use sqlparser::dialect::GenericDialect;

fn foreign_keys<'db>(
    db: &'db ParserDB,
    table: &str,
) -> Vec<&'db <ParserDB as DatabaseLike>::ForeignKey> {
    db.table_by_target(TargetName::new(table, false), IdentifierCase::AsWritten)
        .expect("Unable to look up the table")
        .expect("The table exists")
        .foreign_keys(db)
        .expect("Unable to list the foreign keys")
        .collect()
}

#[test]
fn plain_keys_are_neither_vertical_nor_horizontal_same_as() {
    let db = ParserDB::parse::<GenericDialect>(
        "
    CREATE TABLE parent (id INT PRIMARY KEY);
    CREATE TABLE brother (id INT PRIMARY KEY, name TEXT);
    CREATE TABLE child (
        id INT PRIMARY KEY REFERENCES parent(id),
        brother_id INT REFERENCES brother(id),
        name TEXT
    );
    ",
    )
    .expect("Unable to parse the schema");
    let [extension, brother_fk] = foreign_keys(&db, "child")[..] else {
        panic!("Expected two foreign keys in child");
    };

    assert!(extension.is_extension_foreign_key(&db).unwrap());
    assert!(!extension.is_vertical_same_as(&db).unwrap());
    assert_eq!(extension.vertical_same_as_column_pair(&db).unwrap(), None);
    assert!(!brother_fk.is_horizontal_same_as(&db).unwrap());
    assert_eq!(brother_fk.horizontal_same_as_column_pairs(&db).unwrap(), None);
    assert!(!brother_fk.is_horizontal_same_as_of_triangular(&db).unwrap());
}

#[test]
fn a_horizontal_same_as_to_a_composite_primary_key_has_no_key_column() {
    let db = ParserDB::parse::<GenericDialect>(
        "
    CREATE TABLE parent (id INT PRIMARY KEY);
    CREATE TABLE brother (
        first_id INT,
        second_id INT,
        name TEXT,
        PRIMARY KEY (first_id, second_id),
        UNIQUE (first_id, second_id, name)
    );
    CREATE TABLE child (
        id INT PRIMARY KEY REFERENCES parent(id),
        brother_first_id INT,
        brother_second_id INT,
        name TEXT,
        FOREIGN KEY (brother_first_id, brother_second_id, name)
            REFERENCES brother(first_id, second_id, name)
    );
    ",
    )
    .expect("Unable to parse the schema");
    let [_extension, brother_fk] = foreign_keys(&db, "child")[..] else {
        panic!("Expected two foreign keys in child");
    };

    assert!(brother_fk.is_horizontal_same_as(&db).unwrap());
    assert_eq!(brother_fk.horizontal_same_as_column_pairs(&db).unwrap(), None);
    assert!(!brother_fk.is_horizontal_same_as_of_triangular(&db).unwrap());
}

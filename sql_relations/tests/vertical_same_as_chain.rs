//! A column equal to a column every ancestor carries names only its nearest
//! ancestor as vertically same-as.

use sql_relations::prelude::*;
use sqlparser::dialect::GenericDialect;

const SCHEMA: &str = "
CREATE TABLE resources (id INT PRIMARY KEY, organization_id INT NOT NULL, UNIQUE (organization_id, id));
CREATE TABLE ownables (
    id INT PRIMARY KEY REFERENCES resources (id),
    organization_id INT NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE asset_models (
    id INT PRIMARY KEY REFERENCES ownables (id),
    organization_id INT NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE physical_asset_models (
    id INT PRIMARY KEY REFERENCES asset_models (id),
    organization_id INT NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
CREATE TABLE container_models (
    id INT PRIMARY KEY REFERENCES physical_asset_models (id),
    organization_id INT NOT NULL,
    FOREIGN KEY (organization_id, id) REFERENCES resources (organization_id, id)
);
";

fn organization_id<'db>(db: &'db ParserDB, table: &str) -> &'db <ParserDB as DatabaseLike>::Column {
    db.table_by_target(TargetName::new(table, false), IdentifierCase::AsWritten)
        .expect("Unable to look up the table")
        .expect("The table exists")
        .column("organization_id", db, IdentifierCase::AsWritten)
        .expect("Unable to look up the column")
        .expect("The column exists")
}

#[test]
fn each_level_names_only_its_parent() {
    let db = ParserDB::parse::<GenericDialect>(SCHEMA).expect("Unable to parse the schema");
    for (table, parent) in [
        ("ownables", "resources"),
        ("asset_models", "ownables"),
        ("physical_asset_models", "asset_models"),
        ("container_models", "physical_asset_models"),
    ] {
        let column = organization_id(&db, table);
        let tables = column
            .dominant_vertical_same_as_columns(&db)
            .unwrap()
            .into_iter()
            .map(|same_as| ColumnLike::table(same_as, &db).table_name())
            .collect::<Vec<_>>();
        assert_eq!(tables, [parent], "{table}.organization_id");
    }
}

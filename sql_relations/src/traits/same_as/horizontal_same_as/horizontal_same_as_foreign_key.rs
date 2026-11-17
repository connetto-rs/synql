//! Submodule providing the `HorizontalSameAsForeignKeyLike` trait for
//! determining whether a foreign key relationship is a horizontal same-as
//! relationship.

use sql_traits::{
    errors::LookupError,
    traits::{DatabaseLike, TableLike},
};

use crate::traits::{SameAsIndexLike, VerticalSameAsForeignKeyLike};

/// Trait for foreign keys that can be checked for being horizontal same-as
/// relationships.
pub trait HorizontalSameAsForeignKeyLike: VerticalSameAsForeignKeyLike {
    /// Returns whether this key column usage is an horizontal same-as
    /// constraint.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE parent (id INT PRIMARY KEY, parent_name TEXT, UNIQUE(id, parent_name));
    /// CREATE TABLE brother (id INT PRIMARY KEY, brother_name TEXT, UNIQUE(id, brother_name));
    /// CREATE TABLE child (
    ///     id INT PRIMARY KEY REFERENCES parent(id),
    ///     brother_id INT,
    ///     child_name TEXT,
    ///     FOREIGN KEY (id, child_name) REFERENCES parent(id, parent_name),
    ///     FOREIGN KEY (brother_id, child_name) REFERENCES brother(id, brother_name)
    /// );
    /// ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let [extension_primary_key, parent_fk, brother_fk] =
    ///     child_table.foreign_keys(&db)?.collect::<Vec<_>>()[..]
    /// else {
    ///     panic!("Expected exactly 3 foreign keys in child table");
    /// };
    ///
    /// assert!(extension_primary_key.is_extension_foreign_key(&db)?, "Expected extension primary key");
    /// assert!(
    ///     !parent_fk.is_horizontal_same_as(&db)?,
    ///     "Expected parent foreign key to not be horizontal same-as"
    /// );
    /// assert!(
    ///     brother_fk.is_horizontal_same_as(&db)?,
    ///     "Expected brother foreign key to be horizontal same-as"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the columns, the referenced table or its
    /// unique indices cannot be resolved.
    fn is_horizontal_same_as(&self, database: &Self::DB) -> Result<bool, LookupError> {
        let references_same_as_index = self
            .is_referenced_unique_key(database)?
            .map_or(Ok(false), |unique_index| unique_index.is_same_as(database))?;
        Ok(references_same_as_index
            && !self.is_vertical_same_as(database)?
            && !self.is_self_referential(database)?
            && !self.references_ancestor_table(database)?)
    }

    /// Returns the host column paired with the primary key of the referenced
    /// table, followed by every other host-referenced column pair of this
    /// horizontal same-as foreign key, whatever the order of its columns.
    ///
    /// Returns `None` when this foreign key is not horizontal same-as or the
    /// referenced table has a composite primary key.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE brother (id INT PRIMARY KEY, name TEXT, age INT, UNIQUE(id, name, age));
    /// CREATE TABLE child (
    ///     id INT PRIMARY KEY,
    ///     brother_id INT REFERENCES brother(id),
    ///     child_name TEXT,
    ///     child_age INT,
    ///     FOREIGN KEY (child_name, brother_id, child_age) REFERENCES brother(name, id, age)
    /// );
    /// ",
    /// )?;
    /// let column = |table: &str, column: &str| {
    ///     db.table_by_target(TargetName::new(table, false), IdentifierCase::AsWritten)
    ///         .unwrap()
    ///         .unwrap()
    ///         .column(column, &db, IdentifierCase::AsWritten)
    ///         .unwrap()
    ///         .unwrap()
    /// };
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let [_brother_fk, horizontal_fk] = child_table.foreign_keys(&db)?.collect::<Vec<_>>()[..]
    /// else {
    ///     panic!("Expected exactly 2 foreign keys in child table");
    /// };
    ///
    /// assert_eq!(
    ///     horizontal_fk.horizontal_same_as_column_pairs(&db)?,
    ///     Some((
    ///         column("child", "brother_id"),
    ///         vec![
    ///             (column("child", "child_name"), column("brother", "name")),
    ///             (column("child", "child_age"), column("brother", "age")),
    ///         ]
    ///     )),
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the host or referenced columns, the
    /// referenced table or its primary key cannot be resolved.
    #[expect(
        clippy::type_complexity,
        reason = "the key column and its paired columns are borrowed from the database"
    )]
    fn horizontal_same_as_column_pairs<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<
        Option<(
            &'db <Self::DB as DatabaseLike>::Column,
            Vec<(&'db <Self::DB as DatabaseLike>::Column, &'db <Self::DB as DatabaseLike>::Column)>,
        )>,
        LookupError,
    > {
        if !self.is_horizontal_same_as(database)? {
            return Ok(None);
        }

        let mut primary_key_columns =
            self.referenced_table(database)?.primary_key_columns(database)?;
        let (Some(primary_key_column), None) =
            (primary_key_columns.next(), primary_key_columns.next())
        else {
            return Ok(None);
        };

        let mut key = None;
        let mut pairs = Vec::new();
        for (host_column, referenced_column) in
            self.host_columns(database)?.zip(self.referenced_columns(database)?)
        {
            if referenced_column == primary_key_column {
                key = Some(host_column);
            } else {
                pairs.push((host_column, referenced_column));
            }
        }
        Ok(key.map(|key| (key, pairs)))
    }
}

impl<T> HorizontalSameAsForeignKeyLike for T where T: VerticalSameAsForeignKeyLike {}

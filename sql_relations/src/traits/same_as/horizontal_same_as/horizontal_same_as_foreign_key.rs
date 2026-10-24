//! Submodule providing the `HorizontalSameAsForeignKeyLike` trait for
//! determining whether a foreign key relationship is a horizontal same-as
//! relationship.

use sql_traits::{errors::LookupError, traits::DatabaseLike};

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

    /// Returns the tuple of host-referenced column pairs for this horizontal
    /// same-as foreign key, excluding the primary key columns.
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
    ///     child_name TEXT,
    ///     brother_id INT REFERENCES brother(id),
    ///     FOREIGN KEY (id, child_name) REFERENCES parent(id, parent_name),
    ///     FOREIGN KEY (brother_id, child_name) REFERENCES brother(id, brother_name)
    /// );
    /// ",
    /// )?;
    /// let parent_table =
    ///     db.table_by_target(TargetName::new("parent", false), IdentifierCase::AsWritten)?.unwrap();
    /// let brother_table =
    ///     db.table_by_target(TargetName::new("brother", false), IdentifierCase::AsWritten)?.unwrap();
    /// let brother_name = brother_table
    ///     .column("brother_name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'brother_name' should exist in brother table");
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let child_name = child_table
    ///     .column("child_name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'child_name' should exist in child table");
    /// let brother_id = child_table
    ///     .column("brother_id", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'brother_id' should exist in child table");
    /// let foreign_keys = child_table.foreign_keys(&db)?.collect::<Vec<_>>();
    /// let [pk_fk, brother_fk, vertical_fk, horizontal_fk] = &foreign_keys.as_slice() else {
    ///     panic!("Expected exactly 4 foreign keys in child table, got {}", foreign_keys.len());
    /// };
    /// assert!(pk_fk.is_extension_foreign_key(&db)?, "Expected extension primary key");
    /// assert!(!brother_fk.is_extension_foreign_key(&db)?, "Expected non-extension foreign key");
    /// assert!(!brother_fk.is_vertical_same_as(&db)?, "Expected non-vertical same-as foreign key");
    /// assert!(vertical_fk.is_vertical_same_as(&db)?, "Expected vertical same-as foreign key");
    /// assert!(
    ///     !vertical_fk.is_horizontal_same_as(&db)?,
    ///     "Expected non-horizontal same-as foreign key"
    /// );
    /// assert!(horizontal_fk.is_horizontal_same_as(&db)?, "Expected horizontal same-as foreign key");
    /// assert!(!horizontal_fk.is_vertical_same_as(&db)?, "Expected non-vertical same-as foreign key");
    ///
    /// assert_eq!(
    ///     horizontal_fk.horizontal_same_as_column_pair(&db)?,
    ///     Some((brother_id, child_name, brother_name)),
    ///     "Expected horizontal same-as column pair to be (brother.name, child.name)"
    /// );
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the host or referenced columns cannot be
    /// resolved.
    #[allow(clippy::type_complexity)]
    fn horizontal_same_as_column_pair<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<
        Option<(
            &'db <Self::DB as DatabaseLike>::Column,
            &'db <Self::DB as DatabaseLike>::Column,
            &'db <Self::DB as DatabaseLike>::Column,
        )>,
        LookupError,
    > {
        if !self.is_horizontal_same_as(database)? {
            return Ok(None);
        }

        let mut pairs = self.host_columns(database)?.zip(self.referenced_columns(database)?);
        let (Some((host_fk_column, _)), Some((host_column, referenced_column)), None) =
            (pairs.next(), pairs.next(), pairs.next())
        else {
            return Ok(None);
        };
        Ok(Some((host_fk_column, host_column, referenced_column)))
    }
}

impl<T> HorizontalSameAsForeignKeyLike for T where T: VerticalSameAsForeignKeyLike {}

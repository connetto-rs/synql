//! Submodule providing the `HorizontalSameAsTableLike` trait for working
//! with tables that have horizontal same-as relationships.

use sql_traits::{
    errors::LookupError,
    traits::{DatabaseLike, TableLike},
};

use crate::traits::HorizontalSameAsForeignKeyLike;

/// Trait for tables which may include horizontal same-as relationships.
pub trait HorizontalSameAsTableLike: TableLike
where
    <Self::DB as DatabaseLike>::ForeignKey: HorizontalSameAsForeignKeyLike<DB = Self::DB>,
{
    /// Returns an iterator over the foreign keys of this table that
    /// represent horizontal same-as relationships.
    ///
    /// # Arguments
    ///
    /// * `database` - The database context in which the table exists.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE parent (id INT PRIMARY KEY, name TEXT, UNIQUE(id, name));
    /// CREATE TABLE brother (id INT PRIMARY KEY, name TEXT, UNIQUE(id, name));
    /// CREATE TABLE child (
    ///     id INT PRIMARY KEY REFERENCES parent(id),
    ///     brother_id INT,
    ///     name TEXT,
    ///     FOREIGN KEY (id, name) REFERENCES parent(id, name),
    ///     FOREIGN KEY (brother_id, name) REFERENCES brother(id, name)
    /// );
    /// ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let horizontal_same_as_fks =
    ///     child_table.horizontal_same_as_foreign_keys(&db)?.collect::<Vec<_>>();
    /// assert_eq!(
    ///     horizontal_same_as_fks.len(),
    ///     1,
    ///     "Expected exactly one horizontal same-as foreign key"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the table cannot be
    /// resolved.
    fn horizontal_same_as_foreign_keys<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<impl Iterator<Item = &'db <Self::DB as DatabaseLike>::ForeignKey>, LookupError>
    where
        Self: 'db,
    {
        let mut foreign_keys = Vec::new();
        for foreign_key in self.foreign_keys(database)? {
            if foreign_key.is_horizontal_same_as(database)? {
                foreign_keys.push(foreign_key);
            }
        }
        Ok(foreign_keys.into_iter())
    }

    /// Returns whether the table has any horizontal same-as foreign keys.
    ///
    /// # Arguments
    /// * `database` - The database context in which the table exists.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE parent (id INT PRIMARY KEY, name TEXT, UNIQUE(id, name));
    /// CREATE TABLE brother (id INT PRIMARY KEY, name TEXT, UNIQUE(id, name));
    /// CREATE TABLE child (
    ///     id INT PRIMARY KEY REFERENCES parent(id),
    ///     brother_id INT,
    ///     name TEXT,
    ///     FOREIGN KEY (id, name) REFERENCES parent(id, name),
    ///     FOREIGN KEY (brother_id, name) REFERENCES brother(id, name)
    /// );
    /// ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let parent_table =
    ///     db.table_by_target(TargetName::new("parent", false), IdentifierCase::AsWritten)?.unwrap();
    /// let brother_table =
    ///     db.table_by_target(TargetName::new("brother", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert!(child_table.has_horizontal_same_as(&db)?);
    /// assert!(!parent_table.has_horizontal_same_as(&db)?);
    /// assert!(!brother_table.has_horizontal_same_as(&db)?);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the table cannot be
    /// resolved.
    fn has_horizontal_same_as(&self, database: &Self::DB) -> Result<bool, LookupError> {
        for foreign_key in self.foreign_keys(database)? {
            if foreign_key.is_horizontal_same_as(database)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl<T> HorizontalSameAsTableLike for T
where
    T: TableLike,
    <T::DB as DatabaseLike>::ForeignKey: HorizontalSameAsForeignKeyLike<DB = T::DB>,
{
}

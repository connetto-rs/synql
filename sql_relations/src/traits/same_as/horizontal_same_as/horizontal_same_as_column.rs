//! Submodule providing the `HorizontalSameAsColumnLike` trait for working
//! with columns that have horizontal same-as relationships.

use sql_traits::{
    errors::LookupError,
    traits::{ColumnLike, DatabaseLike, ForeignKeyLike},
};

use crate::traits::HorizontalSameAsForeignKeyLike;
/// Trait for tables which may include horizontal same-as relationships.
pub trait HorizontalSameAsColumnLike: ColumnLike
where
    <Self::DB as DatabaseLike>::ForeignKey: HorizontalSameAsForeignKeyLike<DB = Self::DB>,
{
    /// Returns an iterator over the horizontal same-as foreign keys in the host
    /// table that reference this column.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables and foreign keys.
    /// * `host_table` - The table that may contain horizontal same-as foreign
    ///   keys referencing this column.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    ///
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE right (id SERIAL PRIMARY KEY, name TEXT, age INT, UNIQUE(id, name), UNIQUE(id, age));
    /// CREATE TABLE left (
    ///     id SERIAL PRIMARY KEY,
    ///     right_id INTEGER REFERENCES right(id),
    ///     name TEXT,
    ///     age INT,
    ///     FOREIGN KEY (right_id, name) REFERENCES right(id, name),
    ///     FOREIGN KEY (right_id, age) REFERENCES right(id, age)
    /// );
    ///   ",
    /// )?;
    /// let left_table =
    ///     db.table_by_target(TargetName::new("left", false), IdentifierCase::AsWritten)?.unwrap();
    /// let right_id_column = left_table
    ///     .column("right_id", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'right_id' should exist");
    /// assert_eq!(right_id_column.horizontal_same_as_foreign_keys(&db)?.count(), 2);
    /// let name_column = left_table
    ///     .column("name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'name' should exist");
    /// let age_column = left_table
    ///     .column("age", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'age' should exist");
    /// assert_eq!(name_column.horizontal_same_as_foreign_keys(&db)?.count(), 1);
    /// assert_eq!(age_column.horizontal_same_as_foreign_keys(&db)?.count(), 1);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the table or their
    /// columns cannot be resolved.
    fn horizontal_same_as_foreign_keys<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<impl Iterator<Item = &'db <Self::DB as DatabaseLike>::ForeignKey>, LookupError>
    {
        use crate::traits::same_as::HorizontalSameAsTableLike;
        let mut foreign_keys = Vec::new();
        for foreign_key in self.table(database).horizontal_same_as_foreign_keys(database)? {
            for host_column in foreign_key.host_columns(database)? {
                if host_column == self.borrow() {
                    foreign_keys.push(foreign_key);
                    break;
                }
            }
        }
        Ok(foreign_keys.into_iter())
    }

    /// Returns whether the column has any horizontal same-as foreign key.
    ///
    /// # Arguments
    /// * `database` - The database containing the tables and foreign keys.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    ///
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE right (id SERIAL PRIMARY KEY, name TEXT, age INT, UNIQUE(id, name), UNIQUE(id, age));
    /// CREATE TABLE left (
    ///     id SERIAL PRIMARY KEY,
    ///     right_id INTEGER REFERENCES right(id),
    ///     name TEXT,
    ///     age INT,
    ///     FOREIGN KEY (right_id, name) REFERENCES right(id, name),
    ///     FOREIGN KEY (right_id, age) REFERENCES right(id, age)
    /// );
    ///   ",
    /// )?;
    /// let left_table =
    ///     db.table_by_target(TargetName::new("left", false), IdentifierCase::AsWritten)?.unwrap();
    /// let name_column = left_table
    ///     .column("name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'name' should exist");
    /// let age_column = left_table
    ///     .column("age", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'age' should exist");
    /// assert!(name_column.has_horizontal_same_as_foreign_key(&db)?);
    /// assert!(age_column.has_horizontal_same_as_foreign_key(&db)?);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the table or their
    /// columns cannot be resolved.
    fn has_horizontal_same_as_foreign_key(&self, database: &Self::DB) -> Result<bool, LookupError> {
        Ok(self.horizontal_same_as_foreign_keys(database)?.next().is_some())
    }
}

impl<T> HorizontalSameAsColumnLike for T
where
    T: ColumnLike,
    <T::DB as DatabaseLike>::ForeignKey: HorizontalSameAsForeignKeyLike<DB = T::DB>,
{
}

//! Submodule providing the `TriangularSameAsTableLike` trait for working
//! with tables that have triangular same-as relationships.

use sql_traits::{
    errors::LookupError,
    traits::{DatabaseLike, TableLike},
};

use crate::traits::TriangularSameAsForeignKeyLike;

/// Trait for tables which may include triangular same-as relationships.
pub trait TriangularSameAsTableLike: TableLike
where
    <Self::DB as DatabaseLike>::ForeignKey: TriangularSameAsForeignKeyLike<DB = Self::DB>,
{
    /// Returns an iterator over the foreign keys of this table that
    /// represent triangular same-as relationships.
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
    /// CREATE TABLE grandparent (id INT PRIMARY KEY);
    /// CREATE TABLE parent (id INT PRIMARY KEY REFERENCES grandparent(id));
    /// CREATE TABLE grandparent_hyphotenuse (
    ///     id INT PRIMARY KEY,
    ///     grandparent_id INT REFERENCES grandparent(id),
    ///     UNIQUE(id, grandparent_id)
    /// );
    /// CREATE TABLE child (
    ///     id INT PRIMARY KEY REFERENCES parent(id),
    ///     triangular_id INT REFERENCES grandparent_hyphotenuse(id),
    ///     FOREIGN KEY (triangular_id, id) REFERENCES grandparent_hyphotenuse(id, grandparent_id)
    /// );
    /// ",
    /// )?;
    /// let child =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let triangular_fks = child.triangular_same_as_foreign_keys(&db)?.collect::<Vec<_>>();
    /// assert_eq!(triangular_fks.len(), 1, "Expected exactly one triangular same-as foreign key");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the table or the
    /// tables they reference cannot be resolved.
    fn triangular_same_as_foreign_keys<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<impl Iterator<Item = &'db <Self::DB as DatabaseLike>::ForeignKey>, LookupError>
    where
        Self: 'db,
    {
        let mut foreign_keys = Vec::new();
        for foreign_key in self.foreign_keys(database)? {
            if foreign_key.is_triangular_same_as(database)? {
                foreign_keys.push(foreign_key);
            }
        }
        Ok(foreign_keys.into_iter())
    }
}

impl<T> TriangularSameAsTableLike for T
where
    T: TableLike,
    <T::DB as DatabaseLike>::ForeignKey: TriangularSameAsForeignKeyLike<DB = T::DB>,
{
}

//! Submodule defining the `SameAsTable` trait which provides methods for
//! tables which include same-as relationships.

use sql_traits::{
    errors::LookupError,
    traits::{DatabaseLike, TableLike},
};

use crate::traits::SameAsIndexLike;

/// Trait characterizing whether an index can be used to define a same-as
/// relationship, i.e. it is a unique index over a single column.
pub trait SameAsTableLike: TableLike
where
    <Self::DB as DatabaseLike>::UniqueIndex: SameAsIndexLike<DB = Self::DB>,
{
    /// Returns the indices on the table which can be used to define same-as
    /// relationships.
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
    /// CREATE TABLE with_same_as (id INT PRIMARY KEY, name TEXT, UNIQUE(id, name));
    /// CREATE TABLE no_same_as_one (id INT PRIMARY KEY, name TEXT, UNIQUE(name));
    /// CREATE TABLE no_same_as_two (id INT PRIMARY KEY);
    /// ",
    /// )?;
    /// let table_with_same_as = db
    ///     .table_by_target(TargetName::new("with_same_as", false), IdentifierCase::AsWritten)?
    ///     .unwrap();
    /// let same_as_indices = table_with_same_as.same_as_indices(&db)?.collect::<Vec<_>>();
    /// assert_eq!(same_as_indices.len(), 1, "Expected exactly one same-as index");
    /// let table_no_same_as_one = db
    ///     .table_by_target(TargetName::new("no_same_as_one", false), IdentifierCase::AsWritten)?
    ///     .unwrap();
    /// assert_eq!(
    ///     table_no_same_as_one.same_as_indices(&db)?.count(),
    ///     0,
    ///     "Expected no same-as indices"
    /// );
    /// let table_no_same_as_two = db
    ///     .table_by_target(TargetName::new("no_same_as_two", false), IdentifierCase::AsWritten)?
    ///     .unwrap();
    /// assert_eq!(
    ///     table_no_same_as_two.same_as_indices(&db)?.count(),
    ///     0,
    ///     "Expected no same-as indices"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the unique indices of the table or their
    /// columns cannot be resolved.
    fn same_as_indices<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<impl Iterator<Item = &'db <Self::DB as DatabaseLike>::UniqueIndex>, LookupError>
    where
        Self: 'db,
    {
        let mut indices = Vec::new();
        for index in self.unique_indices(database)? {
            if index.is_same_as(database)? {
                indices.push(index);
            }
        }
        Ok(indices.into_iter())
    }
}

impl<T> SameAsTableLike for T
where
    T: TableLike,
    <T::DB as DatabaseLike>::UniqueIndex: SameAsIndexLike<DB = T::DB>,
{
}

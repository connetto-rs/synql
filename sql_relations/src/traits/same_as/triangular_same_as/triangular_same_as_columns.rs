//! Submodule providing the `TriangularSameAsColumnLike` trait for
//! querying triangular same-as relationships on foreign keys associated
//! to a column.

use sql_traits::{
    errors::LookupError,
    traits::{ColumnLike, DatabaseLike},
};

use crate::traits::TriangularSameAsForeignKeyLike;

/// Trait for columns potentially involved in triangular same-as
/// relationships via foreign keys.
pub trait TriangularSameAsColumnLike: ColumnLike {
    /// Returns whether the column is the key of a triangular same-as
    /// relationship via foreign keys.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the column or the
    /// tables they reference cannot be resolved.
    fn has_triangular_same_as_foreign_key(&self, database: &Self::DB) -> Result<bool, LookupError> {
        for foreign_key in self.foreign_keys(database)? {
            if foreign_key.is_triangular_same_as(database)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Returns the triangular same-as foreign keys defined on the column.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the column or the
    /// tables they reference cannot be resolved.
    fn triangular_same_as_foreign_keys<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<impl Iterator<Item = &'db <Self::DB as DatabaseLike>::ForeignKey>, LookupError>
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

impl<T> TriangularSameAsColumnLike for T where T: ColumnLike {}

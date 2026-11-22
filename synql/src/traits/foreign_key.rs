//! Submodule providing syn functionalities for `ForeignKeyLike` objects.

use proc_macro2::TokenStream;
use quote::quote;
use sql_relations::traits::{HorizontalSameAsForeignKeyLike, TriangularSameAsColumnLike};
use sql_traits::{
    errors::LookupError,
    traits::{ColumnLike, DatabaseLike, ForeignKeyLike, TableIndexLike},
};

use crate::{
    structs::Workspace,
    traits::{ColumnSynLike, TableSynLike},
};

/// Trait defining syn functionalities for `ForeignKeyLike` objects.
pub trait ForeignKeySynLike: ForeignKeyLike {
    /// Returns the syn representation of the foreign key using
    /// the `fk` or the `fpk` macro, depending on whether the foreign key
    /// is a foreign primary key or not.
    ///
    /// The column pairs follow the order of the referenced unique index, the
    /// only order `diesel-builders` resolves the index in.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the referenced table, its unique indices
    /// or the columns of the foreign key cannot be resolved.
    fn to_syn(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<TokenStream, LookupError> {
        let foreign_table = self.referenced_table(database)?;
        let foreign_table_ident = foreign_table.table_ident();
        let foreign_table_crate_ident = foreign_table.crate_ident(workspace);
        let mut column_pairs = self
            .host_columns(database)?
            .zip(self.referenced_columns(database)?)
            .collect::<Vec<_>>();
        let referenced_index = self.is_referenced_unique_key(database)?;
        let index_columns = referenced_index.map(|index| index.columns(database)).transpose()?;
        let index_columns = index_columns.map_or_else(Vec::new, Iterator::collect::<Vec<_>>);
        column_pairs.sort_by_key(|(_, referenced_column)| {
            index_columns.iter().position(|index_column| index_column == referenced_column)
        });
        let is_self_referential = self.is_self_referential(database)?;
        let host_column_paths = column_pairs
            .iter()
            .map(|(host_column, _)| host_column.column_snake_ident())
            .collect::<Vec<syn::Ident>>();
        let foreign_column_paths = column_pairs
            .iter()
            .map(|(_, referenced_column)| {
                let col_ident = referenced_column.column_snake_ident();
                if is_self_referential {
                    syn::parse_quote!(#foreign_table_ident::#col_ident)
                } else {
                    syn::parse_quote!(::#foreign_table_crate_ident::#foreign_table_ident::#col_ident)
                }
            })
            .collect::<Vec<syn::Path>>();

        Ok(quote! {
            #[table_model(foreign_key((#(#host_column_paths),*), (#(#foreign_column_paths),*)))]
        })
    }

    /// Returns the key column of this horizontal same-as foreign key of a
    /// triangle and the pairs whose host column mirrors its referenced column,
    /// or `None` for any other foreign key.
    ///
    /// `diesel-builders` 0.1.2 cannot convert a nullable referenced column into
    /// a `NOT NULL` host column, so such a pair mirrors nothing.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the host or referenced columns, the
    /// referenced table, its primary key or the foreign keys of the key
    /// column cannot be resolved.
    #[expect(
        clippy::type_complexity,
        reason = "the key column and its mirrored columns are borrowed from the database"
    )]
    fn mirrored_column_pairs<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<
        Option<(
            &'db <Self::DB as DatabaseLike>::Column,
            Vec<(&'db <Self::DB as DatabaseLike>::Column, &'db <Self::DB as DatabaseLike>::Column)>,
        )>,
        LookupError,
    > {
        let Some((key, pairs)) = self.horizontal_same_as_column_pairs(database)? else {
            return Ok(None);
        };
        if !key.has_triangular_same_as_foreign_key(database)? {
            return Ok(None);
        }
        let mut mirrored = Vec::with_capacity(pairs.len());
        for (host_column, referenced_column) in pairs {
            if host_column.is_nullable(database)? || !referenced_column.is_nullable(database)? {
                mirrored.push((host_column, referenced_column));
            }
        }
        Ok(Some((key, mirrored)))
    }
}

impl<FK: ForeignKeyLike> ForeignKeySynLike for FK {}

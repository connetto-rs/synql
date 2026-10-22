//! Submodule providing syn functionalities for `ForeignKeyLike` objects.

use proc_macro2::TokenStream;
use quote::quote;
use sql_traits::{errors::LookupError, traits::ForeignKeyLike};

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
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the referenced table or the columns of
    /// the foreign key cannot be resolved.
    fn to_syn(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<TokenStream, LookupError> {
        let foreign_table = self.referenced_table(database)?;
        let foreign_table_ident = foreign_table.table_ident();
        let foreign_table_crate_ident = foreign_table.crate_ident(workspace);
        let host_column_paths = self
            .host_columns(database)?
            .map(ColumnSynLike::column_snake_ident)
            .collect::<Vec<syn::Ident>>();
        let is_self_referential = self.is_self_referential(database)?;
        let foreign_column_paths = self
            .referenced_columns(database)?
            .map(|col| {
                let col_ident = col.column_snake_ident();
                if is_self_referential {
                    syn::parse_quote!(#foreign_table_ident::#col_ident)
                } else {
                    syn::parse_quote!(::#foreign_table_crate_ident::#foreign_table_ident::#col_ident)
                }
            })
            .collect::<Vec<syn::Path>>();

        Ok(quote! {
            #[table_model(foreign_key((#(#host_column_paths,)*), (#(#foreign_column_paths),*)))]
        })
    }
}

impl<FK: ForeignKeyLike> ForeignKeySynLike for FK {}

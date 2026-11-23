//! Submodule implementing the writing of the crate library files.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use sql_relations::prelude::{ColumnLike, TableLike};

use super::write_rust_file;
use crate::{
    structs::{SynQL, Workspace},
    traits::{SynQLDatabaseLike, column::ColumnSynLike, table::TableSynLike},
};

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Writes the `lib.rs` of the crate holding `tables`.
    pub(super) fn write_crate_lib(
        &self,
        tables: &[&DB::Table],
        workspace: &Workspace,
    ) -> Result<(), crate::Error> {
        // We create the `src` directory if it does not exist
        let crate_path = tables[0].crate_absolute_path(workspace);
        let src_path = crate_path.join("src");
        std::fs::create_dir_all(&src_path)?;
        let lib_rs_path = src_path.join("lib.rs");

        let allow_non_snake_case = tables
            .iter()
            .any(|table| !table.has_snake_case_table_name())
            .then(|| quote! { #![allow(non_snake_case)] });
        let multi_table_header = (tables.len() > 1).then(|| {
            let crate_ident = tables[0].crate_ident(workspace);
            quote! {
                #![allow(ambiguous_glob_reexports, reason = "TableModel trait names can collide across tables")]
                extern crate self as #crate_ident;
            }
        });
        let mut table_items = Vec::with_capacity(tables.len());
        for table in tables {
            let items = self.table_items(table, workspace)?;
            table_items.push(if tables.len() > 1 {
                Self::table_module(table, &items, &TokenStream::new())
            } else {
                items
            });
        }

        let content = quote! {
            #allow_non_snake_case
            #multi_table_header
            #(#table_items)*
        };
        write_rust_file(&lib_rs_path, content)
    }

    /// Returns the ident of the public module holding the items of `table`
    /// in a crate holding several tables, named after the table with a
    /// `_model` suffix.
    pub(super) fn table_module_ident(table: &DB::Table) -> syn::Ident {
        format_ident!("{}_model", table.table_snake_name())
    }

    /// Wraps the `items` of `table` in a public `{table}_model` module, glob
    /// re-exported at the crate root, both under `attributes`.
    pub(super) fn table_module(
        table: &DB::Table,
        items: &TokenStream,
        attributes: &TokenStream,
    ) -> TokenStream {
        let module_ident = Self::table_module_ident(table);
        quote! {
            #attributes
            pub mod #module_ident {
                #items
            }
            #attributes
            pub use #module_ident::*;
        }
    }

    /// Returns the items generated for `table`.
    #[allow(clippy::too_many_lines)]
    pub(super) fn table_items(
        &self,
        table: &DB::Table,
        workspace: &Workspace,
    ) -> Result<TokenStream, crate::Error> {
        let core_derives = table.supported_core_derives(self.database, workspace)?;
        let camel_case_name = table.table_singular_camel_ident();
        let table_ident = table.table_ident();
        let struct_documentation =
            table.table_doc(self.database)?.map(|doc| quote! { #[doc = #doc] });

        let mut ancestor_decorator = None;
        let ancestors = table.ancestral_extended_tables_topological(self.database)?;
        if !ancestors.is_empty() {
            let ancestor_table_paths: Vec<syn::Path> = ancestors
                .iter()
                .map(|ancestor_table| {
                    let ancestor_table_ident = ancestor_table.table_snake_ident();
                    let ancestor_table_crate_ident = ancestor_table.crate_ident(workspace);
                    syn::parse_quote! { #ancestor_table_crate_ident::#ancestor_table_ident }
                })
                .collect();
            ancestor_decorator = Some(quote! {
                #[table_model(ancestors(#(#ancestor_table_paths),*))]
            });
        }

        // If the crate has check constraints, it means we need to specify
        // the error type in the derive macro.
        let error_decorator =
            table.has_non_tautological_check_constraints_in_hierarchy(self.database)?.then(|| {
                quote! {
                    #[table_model(error = ::validation_errors::ValidationError)]
                }
            });

        let record_validation_impl =
            table.generate_record_validation_impl(workspace, self.database)?;
        let record_error_decorator = record_validation_impl.is_some().then(|| {
            quote! {
                #[table_model(record_error = ::validation_errors::ValidationError)]
            }
        });

        // #[diesel(primary_key(user_id, role_id))]
        let mut primary_key_decorator = None;
        let primary_key_columns = table.primary_key_columns(self.database)?.collect::<Vec<_>>();
        if !primary_key_columns.is_empty()
            && (primary_key_columns.len() > 1 || primary_key_columns[0].column_name() != "id")
        {
            let primary_key_idents: Vec<syn::Ident> =
                primary_key_columns.iter().map(ColumnSynLike::column_snake_ident).collect();
            primary_key_decorator = Some(quote! {
                #[diesel(primary_key(#(#primary_key_idents),*))]
            });
        }

        // #[table_model(surrogate_key)]
        // For when the primary key is auto-generated by operations like
        // `SERIAL`
        let surrogate_key_decorator = table.has_surrogate_primary_key(self.database)?.then(|| {
            quote! {
                #[table_model(surrogate_key)]
            }
        });

        // Each table pair that diesel queries together is declared once, as
        // `joinable_tables` assigns.
        let mut allow_tables_to_appear_in_same_query = Vec::new();
        for dependency in self.joinable_tables(table)? {
            let dependency_crate_ident = dependency.crate_ident(workspace);
            let dependency_table_ident = dependency.table_ident();
            allow_tables_to_appear_in_same_query.push(quote! {
                ::diesel::allow_tables_to_appear_in_same_query!(
                    #table_ident,
                    ::#dependency_crate_ident::#dependency_table_ident
                );
            });
        }

        let ancestral_table_list_decorator =
            table.ancestral_table_list_decorator(self.database, workspace)?;
        let ancestral_discriminator_decorators =
            table.ancestral_discriminator_decorators(self.database, workspace)?;

        let fields = table.generate_struct_fields(workspace, self.database)?;
        let unique_indices = table.unique_indices_macros(self.database)?;
        let foreign_key_decorators = table.foreign_key_decorators(self.database, workspace)?;
        let check_constraint_impls = table.generate_validation_impls(workspace, self.database)?;
        let belonging_to_decorators =
            table.generate_belonging_to_decorators(self.database, workspace)?;
        let ancestral_primary_key_column_getters =
            table.generate_ancestral_primary_key_column_getters(self.database, workspace)?;

        let mut extra_implementations = Vec::new();
        for callback in &self.callbacks {
            if let Some(implementation) = callback(table, self.database, workspace)? {
                extra_implementations.push(implementation);
            }
        }

        let derive_associations = if belonging_to_decorators.is_empty() {
            None
        } else {
            Some(quote! {
                ::diesel::Associations,
            })
        };

        Ok(quote! {
            #[derive(#(#core_derives),*)]
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            #[derive(::diesel::Queryable, ::diesel::Selectable, ::diesel::Identifiable, #derive_associations ::diesel_builders::prelude::TableModel)]
            #struct_documentation
            #ancestor_decorator
            #error_decorator
            #record_error_decorator
            #(#belonging_to_decorators)*
            #primary_key_decorator
            #surrogate_key_decorator
            #(#foreign_key_decorators)*
            #(#ancestral_table_list_decorator)*
            #(#ancestral_discriminator_decorators)*
            #[diesel(table_name = #table_ident)]
            pub struct #camel_case_name {
                #(#fields),*
            }
            #(#unique_indices)*
            #(#check_constraint_impls)*
            #record_validation_impl
            #(#ancestral_primary_key_column_getters)*
            #(#extra_implementations)*
            #(#allow_tables_to_appear_in_same_query)*
        })
    }
}

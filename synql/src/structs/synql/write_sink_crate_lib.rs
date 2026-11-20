//! Submodule implementing the writing of the sink crate library files.

use std::{collections::HashSet, path::Path};

use proc_macro2::TokenStream;
use quote::quote;

use super::write_rust_file;
use crate::{
    structs::{SynQL, Workspace},
    traits::{SynQLDatabaseLike, table::TableSynLike},
};

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Writes the `lib.rs` of a sink crate re-exporting the crates of
    /// `tables`, followed by `items`.
    pub(super) fn write_sink_crate_lib(
        workspace: &Workspace,
        sink_crate_name: &str,
        sink_crate_path: &Path,
        tables: &[&DB::Table],
        items: &[TokenStream],
    ) -> Result<(), crate::Error> {
        // We create the `src` directory if it does not exist
        let src_path = sink_crate_path.join("src");
        std::fs::create_dir_all(&src_path)?;
        let lib_rs_path = src_path.join("lib.rs");

        let crate_documentation = format!(
            "Auto-generated sink crate `{sink_crate_name}` which re-exports all table crates."
        );

        let mut re_exports = Vec::new();
        let mut re_exported_crates = HashSet::new();
        for table in tables {
            let crate_ident = table.crate_ident(workspace);
            let table_struct_ident = table.table_singular_camel_ident();
            let table_ident = table.table_ident();
            if re_exported_crates.insert(crate_ident.to_string()) {
                re_exports.push(quote! { pub use #crate_ident; });
            }
            re_exports.push(quote! {
                pub use #crate_ident::#table_ident;
                pub use #crate_ident::#table_struct_ident;
            });
        }

        let content = quote! {
            #![doc = #crate_documentation]

            #(#re_exports)*

            #(#items)*
        };
        write_rust_file(&lib_rs_path, content)
    }
}

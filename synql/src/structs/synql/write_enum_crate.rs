//! Submodule implementing the writing of the crates of the PostgreSQL enum
//! types.

use std::io::Write;

use quote::quote;
use sql_traits::traits::UserDefinedTypeLike;

use crate::{
    structs::{SynQL, Workspace},
    traits::{SynQLDatabaseLike, UserDefinedTypeSynLike},
};

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Returns the PostgreSQL enum types of the database whose crates the
    /// workspace holds, grouped by crate, with crates ordered by their first
    /// type.
    pub(super) fn enum_crates(&self, workspace: &Workspace) -> Vec<Vec<&DB::UserDefinedType>> {
        let mut crates: Vec<(String, Vec<&DB::UserDefinedType>)> = Vec::new();
        for user_defined_type in self.database.user_defined_types() {
            if user_defined_type.enumeration().is_none() {
                continue;
            }
            let crate_name = user_defined_type.enum_crate_name(workspace);
            if !workspace
                .external_crates()
                .any(|external_crate| external_crate.name() == crate_name)
            {
                continue;
            }
            match crates.iter_mut().find(|(name, _)| *name == crate_name) {
                Some((_, types)) => types.push(user_defined_type),
                None => crates.push((crate_name, vec![user_defined_type])),
            }
        }
        crates.into_iter().map(|(_, types)| types).collect()
    }

    /// Writes the crate holding the provided enum types, which is the crate
    /// of the first of them.
    pub(super) fn write_enum_crate(
        types: &[&DB::UserDefinedType],
        workspace: &Workspace,
    ) -> Result<(), crate::Error> {
        let [first, ..] = types else {
            return Ok(());
        };
        let crate_path = first.enum_crate_absolute_path(workspace);
        let src_path = crate_path.join("src");
        std::fs::create_dir_all(&src_path)?;

        let name = first.enum_crate_name(workspace);
        let (major, minor, patch) = workspace.version();
        let mut cargo_toml = std::fs::File::create(crate_path.join("Cargo.toml"))?;
        writeln!(
            cargo_toml,
            r#"[package]
name = "{name}"
version = "{major}.{minor}.{patch}"
edition.workspace = true

[dependencies]
serde.workspace = true
diesel = {{ workspace = true, features = ["postgres_backend"] }}

[lints]
workspace = true"#
        )?;
        let documentation = format!(
            "Auto-generated crate for the PostgreSQL enum {} {}.",
            if types.len() == 1 { "type" } else { "types" },
            types
                .iter()
                .map(|user_defined_type| format!("`{}`", user_defined_type.qualified_name()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let sql_types =
            types.iter().filter_map(|user_defined_type| user_defined_type.enum_sql_type_tokens());
        let enums = types.iter().filter_map(|user_defined_type| user_defined_type.enum_tokens());
        let content = quote! {
            #![doc = #documentation]

            /// Diesel SQL types of the crate.
            pub mod sql_types {
                #(#sql_types)*
            }

            #(#enums)*
        };
        let mut lib_rs = std::fs::File::create(src_path.join("lib.rs"))?;
        write!(lib_rs, "{content}")?;
        Ok(())
    }
}

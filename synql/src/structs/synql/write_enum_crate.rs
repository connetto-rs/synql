//! Submodule implementing the writing of the crates of the PostgreSQL enum
//! types.

use quote::quote;
use sql_traits::traits::UserDefinedTypeLike;

use crate::{
    structs::{SynQL, Workspace},
    traits::{SynQLDatabaseLike, UserDefinedTypeSynLike},
};

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Returns the PostgreSQL enum types of the database whose crates the
    /// workspace holds.
    pub(super) fn enum_types<'workspace>(
        &self,
        workspace: &'workspace Workspace,
    ) -> impl Iterator<Item = &DB::UserDefinedType> + use<'_, 'workspace, DB> {
        self.database.user_defined_types().filter(move |user_defined_type| {
            let crate_name = user_defined_type.enum_crate_name(workspace);
            user_defined_type.enumeration().is_some()
                && workspace
                    .external_crates()
                    .any(|external_crate| external_crate.name() == crate_name)
        })
    }

    /// Writes the crate holding the Rust enum of the provided enum type.
    pub(super) fn write_enum_crate(
        user_defined_type: &DB::UserDefinedType,
        workspace: &Workspace,
    ) -> Result<(), crate::Error> {
        let crate_path = user_defined_type.enum_crate_absolute_path(workspace);
        let src_path = crate_path.join("src");
        std::fs::create_dir_all(&src_path)?;

        let name = user_defined_type.enum_crate_name(workspace);
        let (major, minor, patch) = workspace.version();
        let cargo_toml = format!(
            r#"[package]
name = "{name}"
version = "{major}.{minor}.{patch}"
edition.workspace = true

[dependencies]
serde.workspace = true
diesel = {{ workspace = true, features = ["postgres_backend"] }}

[features]
sqlite = ["diesel/sqlite"]

[lints]
workspace = true
"#
        );
        std::fs::write(crate_path.join("Cargo.toml"), cargo_toml)?;
        let documentation = format!(
            "Auto-generated crate for the PostgreSQL enum type `{}`.",
            user_defined_type.qualified_name()
        );
        let sql_type = user_defined_type.enum_sql_type_tokens();
        let enum_tokens = user_defined_type.enum_tokens();
        let content = quote! {
            #![doc = #documentation]

            /// Diesel SQL types of the crate.
            pub mod sql_types {
                #sql_type
            }

            #enum_tokens
        };
        std::fs::write(src_path.join("lib.rs"), content.to_string())?;
        Ok(())
    }
}

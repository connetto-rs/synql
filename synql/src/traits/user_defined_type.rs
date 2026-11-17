//! Submodule defining and implementing the `UserDefinedTypeSynLike` trait,
//! which provides the methods generating the crate of a PostgreSQL enum type,
//! building on top of the [`UserDefinedTypeLike`] trait.

use std::{borrow::Cow, path::PathBuf};

use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use sql_traits::traits::{EnumerationMember, UserDefinedTypeLike};
use syn::Ident;

use crate::structs::{
    EnumVariant, ExternalCrate, ExternalType, TomlDependency, Workspace, rust_type_name,
};

/// The schema PostgreSQL creates a type in when its name names none.
const DEFAULT_SCHEMA: &str = "public";

/// Returns the identifier as PostgreSQL prints it, between double quotes when
/// it was quoted.
fn sql_identifier(identifier: &str, quoted: bool) -> String {
    if quoted { format!("\"{}\"", identifier.replace('"', "\"\"")) } else { identifier.to_owned() }
}

/// Trait implemented by types that represent PostgreSQL user-defined types,
/// generating the crate of an enum type.
pub trait UserDefinedTypeSynLike: UserDefinedTypeLike {
    /// Returns the variants of the Rust enum standing for this enum type, in
    /// the order PostgreSQL sorts its labels, or `None` when this is not an
    /// enum type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::PostgreSqlDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<PostgreSqlDialect>(
    ///     "CREATE TYPE mood AS ENUM ('happy', 'so-so'); CREATE TYPE point AS (x INT, y INT);",
    /// )
    /// .expect("A valid schema");
    /// let type_named = |name: &str| db.user_defined_types().find(|type_| type_.name() == name);
    /// let variants = type_named("mood").unwrap().enum_variants().unwrap();
    /// let names: Vec<String> = variants.iter().map(|variant| variant.ident().to_string()).collect();
    /// assert_eq!(names, ["Happy", "SoSo"]);
    /// assert!(type_named("point").unwrap().enum_variants().is_none());
    /// ```
    fn enum_variants(&self) -> Option<Vec<EnumVariant>> {
        let enumeration = self.enumeration()?;
        Some(EnumVariant::from_labels(enumeration.into_members().map(EnumerationMember::label)))
    }

    /// Returns the name PostgreSQL stores for the schema of this type.
    fn schema_name(&self) -> String {
        self.stored_schema().map_or_else(|| DEFAULT_SCHEMA.to_owned(), Cow::into_owned)
    }

    /// Returns the name of the crate holding the Rust enum of this type, made
    /// of the workspace name, the schema and the type name.
    ///
    /// A table crate name has no hyphen after the workspace name, so the
    /// hyphen between the schema and the type name keeps the two apart.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::PostgreSqlDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<PostgreSqlDialect>(
    ///     "CREATE SCHEMA authz; CREATE TYPE authz.team_role AS ENUM ('member'); CREATE TYPE mood AS ENUM ('sad');",
    /// )
    /// .expect("A valid schema");
    /// let workspace: Workspace = Workspace::new().name("connetto").expect("A valid name").into();
    /// let crate_names: Vec<String> =
    ///     db.user_defined_types().map(|type_| type_.enum_crate_name(&workspace)).collect();
    /// assert_eq!(crate_names, ["connetto-public-mood", "connetto-authz-team_role"]);
    /// ```
    #[must_use]
    fn enum_crate_name(&self, workspace: &Workspace) -> String {
        format!(
            "{}-{}-{}",
            workspace.name(),
            self.schema_name().to_snake_case(),
            self.stored_name().to_snake_case()
        )
    }

    /// Returns the ident of the crate holding the Rust enum of this type.
    #[must_use]
    fn enum_crate_ident(&self, workspace: &Workspace) -> Ident {
        Ident::new(
            &self.enum_crate_name(workspace).replace('-', "_"),
            proc_macro2::Span::call_site(),
        )
    }

    /// Returns the path of the crate holding the Rust enum of this type,
    /// relative to the workspace.
    #[must_use]
    fn enum_crate_relative_path(&self, workspace: &Workspace) -> PathBuf {
        workspace.crate_base_path().join(self.enum_crate_name(workspace))
    }

    /// Returns the absolute path of the crate holding the Rust enum of this
    /// type.
    #[must_use]
    fn enum_crate_absolute_path(&self, workspace: &Workspace) -> PathBuf {
        workspace.path().join(self.enum_crate_relative_path(workspace))
    }

    /// Returns the workspace dependency on the crate holding the Rust enum of
    /// this type.
    ///
    /// # Panics
    ///
    /// Panics if the path cannot be set.
    #[must_use]
    fn enum_crate_dependency(&self, workspace: &Workspace) -> TomlDependency {
        TomlDependency::new(self.enum_crate_name(workspace))
            .path(self.enum_crate_relative_path(workspace).display().to_string())
            .expect("Should be able to set path")
    }

    /// Returns the ident of the Rust enum of this type and of its diesel SQL
    /// type.
    #[must_use]
    fn enum_ident(&self) -> Ident {
        Ident::new(&rust_type_name(&self.stored_name()), proc_macro2::Span::call_site())
    }

    /// Returns the lowercase names a column of this type spells it with,
    /// qualified by the schema the type was created in, or both bare and
    /// qualified by `public` for the default schema.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::PostgreSqlDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<PostgreSqlDialect>(
    ///     "CREATE SCHEMA authz; CREATE TYPE authz.\"Role\" AS ENUM ('reader'); CREATE TYPE mood AS ENUM ('sad');",
    /// )
    /// .expect("A valid schema");
    /// let names: Vec<Vec<String>> =
    ///     db.user_defined_types().map(UserDefinedTypeSynLike::enum_postgres_types).collect();
    /// assert_eq!(names, [vec!["public.mood", "mood"], vec!["authz.\"role\""]]);
    /// ```
    #[must_use]
    fn enum_postgres_types(&self) -> Vec<String> {
        let name = sql_identifier(self.name(), self.name_is_quoted()).to_lowercase();
        match self.schema() {
            Some(schema) => {
                vec![
                    format!("{}.{name}", sql_identifier(schema, self.schema_is_quoted()))
                        .to_lowercase(),
                ]
            }
            None => vec![format!("{DEFAULT_SCHEMA}.{name}"), name],
        }
    }

    /// Returns the external crate providing the Rust enum of this type to the
    /// table crates, mapping the names of [`Self::enum_postgres_types`], or
    /// `None` when this is not an enum type.
    ///
    /// # Panics
    ///
    /// Panics if the crate name is invalid.
    #[must_use]
    fn enum_external_crate(&self, workspace: &Workspace) -> Option<ExternalCrate> {
        let variants = self.enum_variants()?;
        let crate_ident = self.enum_crate_ident(workspace);
        let enum_ident = self.enum_ident();
        let external_type = ExternalType::new(
            syn::parse_quote!(::#crate_ident::sql_types::#enum_ident),
            syn::parse_quote!(::#crate_ident::#enum_ident),
        )
        .postgres_types(self.enum_postgres_types())
        .expect("The names of a type are distinct and lowercase")
        .variants(variants)
        .supports_debug()
        .supports_copy()
        .supports_hash()
        .supports_ord()
        .into();
        Some(
            ExternalCrate::new(&self.enum_crate_name(workspace))
                .expect("A valid crate name")
                .add_type(external_type)
                .expect("A single type has no duplicate")
                .into(),
        )
    }

    /// Returns the name of this type qualified by its schema, as the
    /// documentation and errors of the generated code spell it.
    fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema_name(), self.stored_name())
    }

    /// Returns the diesel SQL type of this type, to be placed in the
    /// `sql_types` module at the root of its crate, or `None` when this is
    /// not an enum type.
    #[must_use]
    fn enum_sql_type_tokens(&self) -> Option<TokenStream> {
        self.enumeration()?;
        let enum_ident = self.enum_ident();
        let stored_name = self.stored_name();
        let postgres_type = if let Some(schema) = self.stored_schema() {
            quote! { postgres_type(name = #stored_name, schema = #schema) }
        } else {
            quote! { postgres_type(name = #stored_name) }
        };
        let documentation =
            format!("Diesel SQL type of the `{}` PostgreSQL enum type.", self.qualified_name());
        Some(quote! {
            #[doc = #documentation]
            #[derive(Debug, Clone, Copy, ::diesel::query_builder::QueryId, ::diesel::sql_types::SqlType)]
            #[diesel(#postgres_type)]
            pub struct #enum_ident;
        })
    }

    /// Returns the Rust enum of this type with its conversions from and to
    /// PostgreSQL, to be placed at the root of its crate, or `None` when this
    /// is not an enum type.
    ///
    /// The tokens reach the diesel SQL type through the `sql_types` module at
    /// the root of the crate.
    #[must_use]
    fn enum_tokens(&self) -> Option<TokenStream> {
        let variants = self.enum_variants()?;
        let enum_ident = self.enum_ident();
        let qualified_name = self.qualified_name();
        let documentation = format!(
            "Rust enum of the `{qualified_name}` PostgreSQL enum type, ordered as PostgreSQL orders its labels."
        );
        let variant_idents: Vec<&Ident> = variants.iter().map(EnumVariant::ident).collect();
        let labels: Vec<&str> = variants.iter().map(EnumVariant::label).collect();
        let variant_documentation: Vec<String> =
            labels.iter().map(|label| format!("The `{label}` label.")).collect();
        let byte_labels: Vec<syn::LitByteStr> = labels
            .iter()
            .map(|label| syn::LitByteStr::new(label.as_bytes(), proc_macro2::Span::call_site()))
            .collect();

        Some(quote! {
            #[doc = #documentation]
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            #[derive(::diesel::AsExpression, ::diesel::FromSqlRow)]
            #[diesel(sql_type = crate::sql_types::#enum_ident)]
            pub enum #enum_ident {
                #(
                    #[doc = #variant_documentation]
                    #[serde(rename = #labels)]
                    #variant_idents,
                )*
            }

            impl ::diesel::serialize::ToSql<crate::sql_types::#enum_ident, ::diesel::pg::Pg> for #enum_ident {
                fn to_sql<'b>(
                    &'b self,
                    out: &mut ::diesel::serialize::Output<'b, '_, ::diesel::pg::Pg>,
                ) -> ::diesel::serialize::Result {
                    let label: &[u8] = match *self {
                        #(Self::#variant_idents => #byte_labels,)*
                    };
                    ::std::io::Write::write_all(out, label)?;
                    Ok(::diesel::serialize::IsNull::No)
                }
            }

            impl ::diesel::deserialize::FromSql<crate::sql_types::#enum_ident, ::diesel::pg::Pg> for #enum_ident {
                fn from_sql(value: ::diesel::pg::PgValue<'_>) -> ::diesel::deserialize::Result<Self> {
                    match value.as_bytes() {
                        #(#byte_labels => Ok(Self::#variant_idents),)*
                        unknown => Err(format!(
                            "Unrecognized label of the `{}` enum type: {}",
                            #qualified_name,
                            String::from_utf8_lossy(unknown)
                        )
                        .into()),
                    }
                }
            }
        })
    }
}

impl<T: UserDefinedTypeLike> UserDefinedTypeSynLike for T {}

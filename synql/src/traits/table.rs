//! Submodule defining and implementing the `TableSynLike` trait, which provides
//! methods to facilitate the rust code generation starting from a SQL table
//! representation, building on top of the
//! [`TableLike`] trait and the traits from the
//! [`sql_relations`] crate.

use std::path::PathBuf;

use heck::{ToSnakeCase, ToUpperCamelCase};
use inflection_rs::inflection::singularize;
use quote::quote;
use sql_relations::{
    prelude::{ForeignKeyLike, UniqueIndexLike},
    traits::{
        HorizontalSameAsForeignKeyLike, TableListLike, TriangularSameAsForeignKeyLike,
        VerticalSameAsForeignKeyLike,
    },
};
use sql_traits::{
    errors::LookupError,
    traits::{CheckConstraintLike, ColumnLike, DatabaseLike, TableLike},
};
use strum::IntoEnumIterator;
use syn::Ident;

use crate::{
    structs::{ExternalCrate, TomlDependency, Trait, Workspace},
    traits::{
        ColumnSynLike, UniqueIndexSynLike, foreign_key::ForeignKeySynLike,
        function::FunctionSynLike,
    },
    utils::is_reserved_rust_word,
};

/// Trait implemented by types that represent SQL tables and can be used to
/// generate Rust code for them.
pub trait TableSynLike: TableLike
where
    <Self::DB as DatabaseLike>::Column: ColumnSynLike,
{
    /// Returns the name of the crate associated with this table.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the crate is defined.
    #[must_use]
    fn crate_name(&self, workspace: &Workspace) -> String {
        format!("{}-{}", workspace.name(), self.table_snake_name())
    }

    /// Returns the ident of the crate associated with this table.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the crate is defined.
    #[must_use]
    fn crate_ident(&self, workspace: &Workspace) -> Ident {
        Ident::new(&self.crate_name(workspace).replace('-', "_"), proc_macro2::Span::call_site())
    }

    /// Returns the absolute path of the crate associated with this table.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the crate is defined.
    #[must_use]
    fn crate_absolute_path(&self, workspace: &Workspace) -> PathBuf {
        workspace.path().join(workspace.crate_base_path()).join(self.crate_name(workspace))
    }

    /// Returns the relative path of the crate associated with this table.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the crate is defined.
    #[must_use]
    fn crate_relative_path(&self, workspace: &Workspace) -> PathBuf {
        workspace.crate_base_path().join(self.crate_name(workspace))
    }

    /// Returns the dependency of the crate associated with this table.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the crate is defined.
    ///
    /// # Panics
    ///
    /// Panics if the path cannot be set.
    #[must_use]
    fn crate_dependency(&self, workspace: &Workspace) -> TomlDependency {
        TomlDependency::new(self.crate_name(workspace))
            .path(self.crate_relative_path(workspace).display().to_string())
            .expect("Should be able to set path")
    }

    /// Returns the ident of this table, with no normalization applied.
    fn table_ident(&self) -> Ident {
        let table_name = self.table_name();
        if is_reserved_rust_word(table_name) {
            Ident::new_raw(table_name, proc_macro2::Span::call_site())
        } else {
            Ident::new(table_name, proc_macro2::Span::call_site())
        }
    }

    /// Returns the snake-cased name of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE _my__table (id INT);")?;
    /// let table = db
    ///     .table_by_target(TargetName::new("_my__table", false), IdentifierCase::AsWritten)?
    ///     .unwrap();
    /// assert_eq!(table.table_snake_name(), "my_table");
    /// # Ok(())
    /// # }
    /// ```
    fn table_snake_name(&self) -> String {
        self.table_name().to_snake_case()
    }

    /// Returns the singular snake-cased name of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("users", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(table.table_singular_snake_name(), "user");
    /// # Ok(())
    /// # }
    /// ```
    fn table_singular_snake_name(&self) -> String {
        let snake_name = self.table_snake_name();
        singularize(&snake_name)
    }

    /// Returns whether the table name is snake case.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    ///         CREATE TABLE my_table (id INT);
    ///         CREATE TABLE MyTable (id INT);
    /// ",
    /// )?;
    /// let table1 =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// let table2 =
    ///     db.table_by_target(TargetName::new("MyTable", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert!(table1.has_snake_case_table_name());
    /// assert!(!table2.has_snake_case_table_name());
    /// # Ok(())
    /// # }
    /// ```
    fn has_snake_case_table_name(&self) -> bool {
        self.table_snake_name() == self.table_name()
    }

    /// Returns the snake-cased ident of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE box (id INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("box", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(table.table_snake_ident().to_string(), "r#box");
    /// # Ok(())
    /// # }
    /// ```
    fn table_snake_ident(&self) -> Ident {
        let snake_name = self.table_snake_name();
        if is_reserved_rust_word(&snake_name) {
            Ident::new_raw(&snake_name, proc_macro2::Span::call_site())
        } else {
            Ident::new(&snake_name, proc_macro2::Span::call_site())
        }
    }

    /// Returns the singular snake-cased ident of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("users", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(table.table_singular_snake_ident().to_string(), "user");
    /// # Ok(())
    /// # }
    /// ```
    fn table_singular_snake_ident(&self) -> Ident {
        let snake_name = self.table_singular_snake_name();
        if is_reserved_rust_word(&snake_name) {
            Ident::new_raw(&snake_name, proc_macro2::Span::call_site())
        } else {
            Ident::new(&snake_name, proc_macro2::Span::call_site())
        }
    }

    /// Returns the camel-cased name of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE my_table (id INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(table.table_camel_name(), "MyTable");
    /// # Ok(())
    /// # }
    /// ```
    fn table_camel_name(&self) -> String {
        self.table_name().to_upper_camel_case()
    }

    /// Returns the singular camel-cased name of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("users", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(table.table_singular_camel_name(), "User");
    /// # Ok(())
    /// # }
    /// ```
    fn table_singular_camel_name(&self) -> String {
        let singular = singularize(self.table_name());
        singular.to_upper_camel_case()
    }

    /// Returns the camel-cased ident of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE struct (id INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("struct", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(table.table_camel_ident().to_string(), "Struct");
    /// # Ok(())
    /// # }
    /// ```
    fn table_camel_ident(&self) -> Ident {
        let camel_name = self.table_camel_name();
        if is_reserved_rust_word(&camel_name) {
            Ident::new_raw(&camel_name, proc_macro2::Span::call_site())
        } else {
            Ident::new(&camel_name, proc_macro2::Span::call_site())
        }
    }

    /// Returns the singular camel-cased ident of this table.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE users (id INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("users", false), IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(table.table_singular_camel_ident().to_string(), "User");
    /// # Ok(())
    /// # }
    /// ```
    fn table_singular_camel_ident(&self) -> Ident {
        let camel_name = self.table_singular_camel_name();
        if is_reserved_rust_word(&camel_name) {
            Ident::new_raw(&camel_name, proc_macro2::Span::call_site())
        } else {
            Ident::new(&camel_name, proc_macro2::Span::call_site())
        }
    }

    /// Iterates over the identifiers of the primary key columns of this table.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the primary key columns cannot be
    /// resolved.
    fn primary_key_idents<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<impl Iterator<Item = Ident> + 'db, LookupError> {
        Ok(self.primary_key_columns(database)?.map(ColumnSynLike::column_snake_ident))
    }

    /// Returns the sorted unique external crates required by this table,
    /// including those required by its columns and its check constraints.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the columns, check constraints or their
    /// functions cannot be resolved.
    fn external_crates<'workspace>(
        &self,
        database: &Self::DB,
        workspace: &'workspace Workspace,
    ) -> Result<Vec<&'workspace ExternalCrate>, LookupError> {
        let mut crates = Vec::new();
        for column in self.columns(database)? {
            if let Some(postgres_type) = column.external_postgres_type(workspace, database) {
                crates.push(postgres_type.external_crate());
            }
        }
        for check_constraint in self.check_constraints(database)? {
            for function in check_constraint.functions(database)? {
                if let Some(external_crate) = function.external_function_ref(workspace) {
                    crates.push(external_crate.external_crate());
                }
            }
        }
        crates.sort_unstable();
        crates.dedup();
        Ok(crates)
    }

    /// Returns the list of core derives that can be derived for this table.
    ///
    /// # Arguments
    /// * `database` - The database where the table is defined.
    /// * `workspace` - The workspace where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the columns cannot be resolved.
    fn supported_core_derives(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<Vec<syn::Path>, LookupError> {
        let columns = self.columns(database)?.collect::<Vec<_>>();
        let mut derives = Vec::new();
        for core_trait in Trait::iter() {
            if columns.iter().all(|col| col.supports(core_trait, workspace, database)) {
                derives.push(core_trait.path());
            }
        }
        Ok(derives)
    }

    /// Generates the struct field tokens for all columns of this table.
    ///
    /// # Arguments
    /// * `workspace` - The workspace where the table is defined.
    /// * `database` - The database where the table is defined.
    ///
    ///  # Errors
    ///
    /// Returns an error if the external type of any column cannot be
    /// determined.
    fn generate_struct_fields(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Vec<proc_macro2::TokenStream>, crate::Error> {
        let mut attributes = Vec::new();
        for column in self.columns(database)? {
            let attr_tokens = column.generate_struct_field(workspace, database)?;
            attributes.push(attr_tokens);
        }
        Ok(attributes)
    }

    /// Generates the `unique_index!` macro invocations for all unique indexes
    /// of this table.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the unique indices or their columns
    /// cannot be resolved.
    fn unique_indices_macros(
        &self,
        database: &Self::DB,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let mut indices = Vec::new();
        for unique_index in self.unique_indices(database)? {
            // Primary keys already automatically have unique constraints macro
            // generated, so we skip them here.
            if unique_index.is_primary_key(database)? {
                continue;
            }
            indices.push(unique_index.to_syn(database)?);
        }
        Ok(indices)
    }

    /// Generates the decorators for the ancestral table list columns of this
    /// table.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    /// * `workspace` - The workspace where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the extension root table or the tables
    /// its columns refer to cannot be resolved.
    fn ancestral_table_list_decorator(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let Some(table_root) = self.extension_root_table(database)? else {
            return Ok(Vec::new());
        };
        let current_table_name = self.table_name();
        let root_table_ident = table_root.table_ident();
        let root_table_crate = table_root.crate_ident(workspace);
        Ok(table_root
            .columns_referring_to_table_lists(database)?
            .map(|col| {
                let col_ident = col.column_snake_ident();
                quote! {
                    #[table_model(default(#root_table_crate::#root_table_ident::#col_ident, #current_table_name))]
                }
            })
            .collect::<Vec<_>>())
    }

    /// Returns the decorators for the foreign keys of this table, excluding
    /// those which are not already handled by same-as relations.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    /// * `workspace` - The workspace where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys or the tables and
    /// columns they involve cannot be resolved.
    fn foreign_key_decorators(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let mut fks = Vec::new();
        for foreign_key in self.foreign_keys(database)? {
            if foreign_key.is_vertical_same_as(database)?
                || foreign_key.is_horizontal_same_as(database)?
                || foreign_key.is_triangular_same_as(database)?
            {
                continue;
            }
            fks.push(foreign_key.to_syn(database, workspace)?);
        }
        Ok(fks)
    }

    /// Generates the validation impls for all check constraints of this table.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the table is defined.
    /// * `database` - The database where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns an error if the external type of any column cannot be
    /// determined.
    fn generate_validation_impls(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Vec<proc_macro2::TokenStream>, crate::Error> {
        let mut validation_impls = Vec::new();
        for column in self.columns(database)? {
            if column.has_non_tautological_check_constraints(database)? {
                validation_impls.push(column.generate_validation_impl(workspace, database)?);
            }
        }
        Ok(validation_impls)
    }

    /// Generates the `#[diesel(belongs_to(Foo, foreign_key = mykey))]`
    /// decorators for this table.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys or the tables and
    /// columns they involve cannot be resolved.
    fn generate_belonging_to_decorators(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let mut decorators = Vec::new();
        for foreign_key in self.foreign_keys(database)? {
            if foreign_key.is_composite(database)? || !foreign_key.is_singleton(database)? {
                continue;
            }
            let host_column = foreign_key
                .host_columns(database)?
                .next()
                .expect("Expected at least one host column for non-composite foreign key")
                .column_snake_ident();
            let referenced_table = foreign_key.referenced_table(database)?;
            let foreign_crate_ident = if foreign_key.is_self_referential(database)? {
                None
            } else {
                Some(referenced_table.crate_ident(workspace))
            };
            let foreign_table_ident = referenced_table.table_singular_camel_ident();
            decorators.push(quote! {
                #[diesel(belongs_to(#foreign_crate_ident::#foreign_table_ident, foreign_key = #host_column))]
            });
        }
        Ok(decorators)
    }

    /// Generates the ancestral primary key column getter functions for this
    /// table.
    ///
    /// # Implementative details
    ///
    /// The `diesel-builders` crate defines a `diesel_builders::GetColumn` trait
    /// which is derived for each field in the table model struct. While it
    /// would be ideal for the `diesel-builders`'s derive macro to also
    /// generate getters for the ancestral primary key columns, since at the
    /// context of the model struct generation the ancestral tables are not
    /// known, we need to manually generate these getters here.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    /// * `workspace` - The workspace where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the ancestral tables or the primary key
    /// columns cannot be resolved.
    fn generate_ancestral_primary_key_column_getters(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let struct_ident = self.table_singular_camel_ident();
        let table_ident = self.table_snake_ident();
        let self_primary_key_columns = self.primary_key_columns(database)?.collect::<Vec<_>>();
        let mut getters = Vec::new();
        for ancestor in self.ancestral_extended_tables(database)? {
            let ancestor_table_ident = ancestor.table_snake_ident();
            let ancestor_table_crate_ident = ancestor.crate_ident(workspace);
            for (ancestor_pk_col, self_pk_col) in
                ancestor.primary_key_columns(database)?.zip(self_primary_key_columns.iter())
            {
                let ancestor_pk_col_ident = ancestor_pk_col.column_snake_ident();
                let self_pk_col_ident = self_pk_col.column_snake_ident();
                getters.push(quote! {
                    impl ::diesel_builders::GetColumn<#ancestor_table_crate_ident::#ancestor_table_ident::#ancestor_pk_col_ident> for #struct_ident {
                        fn get_column_ref(&self) -> &<#table_ident::#self_pk_col_ident as ::diesel_builders::ColumnTyped>::ColumnType {
                            &self.#self_pk_col_ident
                        }
                    }
                });
            }
        }
        Ok(getters)
    }
}

impl<T: TableLike> TableSynLike for T where <T::DB as DatabaseLike>::Column: ColumnSynLike {}

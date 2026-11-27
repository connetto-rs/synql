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
    traits::{TableListLike, TriangularSameAsForeignKeyLike, VerticalSameAsForeignKeyLike},
};
use sql_traits::{
    errors::LookupError,
    traits::{CheckConstraintLike, ColumnLike, DatabaseLike, TableLike},
};
use sqlparser::ast::Expr;
use strum::IntoEnumIterator;
use syn::Ident;

use crate::{
    structs::{ExternalCrate, TomlDependency, Trait, Workspace},
    traits::{
        ColumnSynLike, UniqueIndexSynLike, check_constraint::CheckConstraintSynLike,
        foreign_key::ForeignKeySynLike, function::FunctionSynLike,
    },
    utils::is_reserved_rust_word,
};

/// An extension key, the ancestor column it fixes to a constant, and that
/// constant, as [`TableSynLike::ancestral_discriminators`] returns them.
type Discriminator<'db, DB> =
    (&'db <DB as DatabaseLike>::ForeignKey, &'db <DB as DatabaseLike>::Column, &'db Expr);

/// Trait implemented by types that represent SQL tables and can be used to
/// generate Rust code for them.
pub trait TableSynLike: TableLike
where
    <Self::DB as DatabaseLike>::Column: ColumnSynLike,
{
    /// Returns the name of the crate associated with this table, which holds
    /// the tables grouped with it and is named after one of them, or which
    /// is the single crate named after the workspace, or which the base
    /// workspace holds it in.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the crate is defined.
    #[must_use]
    fn crate_name(&self, workspace: &Workspace) -> String {
        let table_snake_name = self.table_snake_name();
        match workspace.base_table_crate(&table_snake_name) {
            Some(dependency) => dependency.name().to_owned(),
            None if workspace.is_single_crate() => workspace.name().to_owned(),
            None => {
                format!("{}-{}", workspace.name(), workspace.crate_table_name(&table_snake_name))
            }
        }
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

    /// Returns the workspace dependency on the crate associated with this
    /// table, the crate of the base workspace or a path into the workspace
    /// pinned at the version of the workspace.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the crate is defined.
    #[must_use]
    fn crate_dependency(&self, workspace: &Workspace) -> TomlDependency {
        if let Some(dependency) = workspace.base_table_crate(&self.table_snake_name()) {
            return dependency.clone();
        }
        workspace
            .member_dependency(self.crate_name(workspace), &self.crate_relative_path(workspace))
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

    /// Returns the sorted unique external crates required by this table,
    /// including those required by its columns, by the ancestor columns it
    /// writes a [discriminator](Self::ancestral_discriminators) into, and by
    /// its check constraints.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the columns, the ancestors, their
    /// extension keys, the check constraints or their functions cannot be
    /// resolved.
    fn external_crates<'workspace>(
        &self,
        database: &Self::DB,
        workspace: &'workspace Workspace,
    ) -> Result<Vec<&'workspace ExternalCrate>, LookupError> {
        let mut crates = Vec::new();
        let discriminator_columns =
            self.ancestral_discriminators(database)?.into_iter().map(|(_, column, _)| column);
        for column in self.columns(database)?.chain(discriminator_columns) {
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

    /// Returns whether the database assigns every column of the primary key
    /// of this table, through a serial or identity column, a generated
    /// column or a `BEFORE INSERT` trigger, so the builder sets none of them
    /// and the table marks its key as a `surrogate_key`. A table without a
    /// primary key has no such key.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the primary key or the columns the
    /// table's triggers fill cannot be resolved.
    fn has_database_assigned_primary_key(&self, database: &Self::DB) -> Result<bool, LookupError> {
        if !self.has_primary_key(database)? {
            return Ok(false);
        }
        let filled: Vec<_> = self.filled_on_insert_columns(database)?.collect();
        for column in self.primary_key_columns(database)? {
            if !column.is_generated(database) && !filled.contains(&column) {
                return Ok(false);
            }
        }
        Ok(true)
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

    /// Returns the columns of the extended ancestors of this table that its
    /// extension keys, and those of its ancestors, fix to a constant, each
    /// with the key and the constant. A key `ownables (id, kind)` referencing
    /// `resources (id, kind)`, with `ownables.kind` generated as `'ownable'`,
    /// fixes `resources.kind` to `'ownable'` for `ownables` and for every
    /// table extending it. A column the database fills on insert is left
    /// out, as the builder never writes it.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the ancestors, their extension keys or
    /// the columns of those keys cannot be resolved.
    fn ancestral_discriminators<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<Vec<Discriminator<'db, Self::DB>>, LookupError> {
        let mut extension_keys: Vec<_> = self.extension_foreign_keys(database)?.collect();
        for ancestor in self.ancestral_extended_tables(database)? {
            extension_keys.extend(ancestor.extension_foreign_keys(database)?);
        }
        let mut discriminators = Vec::new();
        for foreign_key in extension_keys {
            for (column, constant) in foreign_key.referenced_constants(database)? {
                if !column.is_filled_on_insert(database)? {
                    discriminators.push((foreign_key, column, constant));
                }
            }
        }
        Ok(discriminators)
    }

    /// Generates the decorators writing into the extended ancestors of this
    /// table its [discriminators](Self::ancestral_discriminators), as
    /// `ResourceKind::Ownable` into `resources.kind`. A `default(...)`
    /// applies only to the builder of the table declaring it, so each
    /// descendant repeats the decorators of its ancestors.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    /// * `workspace` - The workspace where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::UnsupportedDiscriminator`] when a constant has
    /// no value of the Rust type of its column, and a lookup error when the
    /// ancestors or their extension keys cannot be resolved.
    fn ancestral_discriminator_decorators(
        &self,
        database: &Self::DB,
        workspace: &Workspace,
    ) -> Result<Vec<proc_macro2::TokenStream>, crate::Error> {
        let mut decorators = Vec::new();
        for (foreign_key, column, constant) in self.ancestral_discriminators(database)? {
            let referenced_table = foreign_key.referenced_table(database)?;
            let referenced_table_ident = referenced_table.table_ident();
            let referenced_table_crate = referenced_table.crate_ident(workspace);
            let Some(value) = column.constant_value(constant, workspace, database) else {
                return Err(crate::Error::UnsupportedDiscriminator {
                    foreign_key: foreign_key_signature(foreign_key, database)?,
                    column_name: column.column_name().to_owned(),
                    constant: constant.to_string(),
                });
            };
            let column_ident = column.column_snake_ident();
            decorators.push(quote! {
                #[table_model(default(#referenced_table_crate::#referenced_table_ident::#column_ident, #value))]
            });
        }
        Ok(decorators)
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
            if foreign_key.is_vertical_same_as(database)? {
                continue;
            }
            // The same-as decorators of a triangle form one two-column key per
            // mirrored column, so only a wider horizontal key is declared, and
            // it replaces the two-column keys it covers.
            if foreign_key.is_horizontal_same_as_of_triangular(database)? {
                if foreign_key.host_columns(database)?.nth(2).is_none() {
                    continue;
                }
            } else if foreign_key.is_triangular_same_as(database)? {
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
        let filled: Vec<_> = self.filled_on_insert_columns(database)?.collect();
        for column in self.columns(database)? {
            // A column the database fills is never set, so never validated.
            if !filled.contains(&column) && column.has_single_column_check_constraints(database)? {
                validation_impls.push(column.generate_validation_impl(workspace, database)?);
            }
        }
        Ok(validation_impls)
    }

    /// Generates the `ValidateRecord` impl checking the constraints that span
    /// several columns, or `None` when the table has no such constraint.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the table is defined.
    /// * `database` - The database where the table is defined.
    ///
    /// # Errors
    ///
    /// Returns an error if a check constraint cannot be translated.
    fn generate_record_validation_impl(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Option<proc_macro2::TokenStream>, crate::Error> {
        let database_assigned_key = self.has_database_assigned_primary_key(database)?;
        let filled: Vec<_> = self.filled_on_insert_columns(database)?.collect();
        let mut validations = Vec::new();
        for check_constraint in self.non_tautological_check_constraints(database)? {
            if check_constraint.number_of_columns(database)? <= 1
                || check_constraint.is_mutual_nullability_constraint(database)
            {
                continue;
            }
            // Only the database holds a value for these columns.
            let mut involves_unset_column = false;
            for column in check_constraint.columns(database)? {
                if (database_assigned_key && column.is_primary_key(database)?)
                    || filled.contains(&column)
                {
                    involves_unset_column = true;
                    break;
                }
            }
            if involves_unset_column {
                continue;
            }
            validations.push(check_constraint.to_syn(database, workspace, &[])?);
        }
        if validations.is_empty() {
            return Ok(None);
        }
        let table_ident = self.table_snake_ident();
        Ok(Some(quote! {
            impl ::diesel_builders::ValidateRecord<#table_ident::table>
                for <#table_ident::table as ::diesel_builders::TableExt>::NewValues
            {
                #[inline]
                fn validate_record(&self) -> Result<(), ::validation_errors::ValidationError> {
                    use diesel::Column;
                    #(#validations)*
                    Ok(())
                }
            }
        }))
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

    /// Returns the number of columns of the widest tuple the
    /// `diesel-builders` builder of this table flattens, which sets the
    /// `size-*` feature the workspace needs.
    ///
    /// The builder flattens the columns of the table and of its extended
    /// ancestors into one tuple. It flattens the columns of the tables its
    /// mandatory triangular same-as keys reference into another, one copy of
    /// a referenced table per key, and those of its discretionary keys into a
    /// third. The ancestors of a referenced table are not part of these
    /// tuples.
    ///
    /// # Arguments
    ///
    /// * `database` - The database where the table is defined.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE templates (id INT PRIMARY KEY);
    /// CREATE TABLE slots (
    ///     id INT PRIMARY KEY,
    ///     template_id INT NOT NULL REFERENCES templates (id),
    ///     note TEXT,
    ///     UNIQUE (id, template_id)
    /// );
    /// CREATE TABLE disposals (
    ///     id INT PRIMARY KEY REFERENCES templates (id),
    ///     from_slot_id INT NOT NULL REFERENCES slots (id),
    ///     into_slot_id INT NOT NULL REFERENCES slots (id),
    ///     FOREIGN KEY (from_slot_id, id) REFERENCES slots (id, template_id),
    ///     FOREIGN KEY (into_slot_id, id) REFERENCES slots (id, template_id)
    /// );",
    /// )
    /// .expect("Unable to parse the schema");
    /// let disposals = db
    ///     .table_by_target(TargetName::new("disposals", false), IdentifierCase::AsWritten)
    ///     .expect("Resolvable tables")
    ///     .expect("The disposals table");
    /// // Two copies of the three columns of `slots` outnumber the four columns
    /// // of `disposals` and `templates`.
    /// assert_eq!(disposals.number_of_columns_in_builder(&db).expect("Resolvable keys"), 6);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the ancestors, the foreign keys or the
    /// tables they reference cannot be resolved.
    fn number_of_columns_in_builder(&self, database: &Self::DB) -> Result<usize, LookupError> {
        let mut number_of_columns_in_hierarchy = self.number_of_columns(database)?;
        for ancestor_table in self.ancestral_extended_tables(database)? {
            number_of_columns_in_hierarchy += ancestor_table.number_of_columns(database)?;
        }
        let mut number_of_mandatory_columns = 0;
        let mut number_of_discretionary_columns = 0;
        for foreign_key in self.foreign_keys(database)? {
            let Some(triangular) = foreign_key.triangular_same_as(database)? else {
                continue;
            };
            let number_of_columns =
                foreign_key.referenced_table(database)?.number_of_columns(database)?;
            if triangular.is_mandatory() {
                number_of_mandatory_columns += number_of_columns;
            } else {
                number_of_discretionary_columns += number_of_columns;
            }
        }
        Ok(number_of_columns_in_hierarchy
            .max(number_of_mandatory_columns)
            .max(number_of_discretionary_columns))
    }
}

impl<T: TableLike> TableSynLike for T where <T::DB as DatabaseLike>::Column: ColumnSynLike {}

/// Spells `foreign_key` as `host (columns) REFERENCES referenced (columns)`,
/// which names it whether or not the schema gave it a name.
fn foreign_key_signature<DB: DatabaseLike>(
    foreign_key: &DB::ForeignKey,
    database: &DB,
) -> Result<String, LookupError> {
    fn column_list<'db, C: ColumnLike + 'db>(columns: impl Iterator<Item = &'db C>) -> String {
        columns.map(ColumnLike::column_name).collect::<Vec<_>>().join(", ")
    }
    Ok(format!(
        "{} ({}) REFERENCES {} ({})",
        foreign_key.host_table(database).table_name(),
        column_list(foreign_key.host_columns(database)?),
        foreign_key.referenced_table(database)?.table_name(),
        column_list(foreign_key.referenced_columns(database)?),
    ))
}

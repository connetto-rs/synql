//! Submodule defining and implementing the `ColumnSyn` trait, which provide
//! methods to facilitate the rust code generation starting from a SQL column
//! representation, building on top of the
//! [`ColumnLike`] trait.

use std::borrow::Borrow;

use heck::{ToSnakeCase, ToUpperCamelCase};
use quote::{ToTokens, quote};
use sql_relations::traits::{
    HorizontalSameAsColumnLike, TriangularSameAsColumnLike, TriangularSameAsForeignKeyLike,
    VerticalSameAsColumnLike,
};
use sql_traits::{
    errors::LookupError,
    traits::{CheckConstraintLike, ColumnLike, DatabaseLike, ForeignKeyLike, TableLike},
};
use syn::{Ident, Type};

use crate::{
    structs::{ExternalTypeRef, Workspace},
    traits::{CheckConstraintSynLike, TableSynLike},
    utils::{is_reserved_diesel_keyword, is_reserved_rust_word},
};

/// Trait implemented by types that represent SQL columns and can be used to
/// generate Rust code for them.
pub trait ColumnSynLike: ColumnLike {
    /// Returns the uppercased acronym of this column.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE my_table (my_column INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// let column = table.column("my_column", &db, IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(column.column_acronym(), "MC");
    /// # Ok(())
    /// # }
    /// ```
    fn column_acronym(&self) -> String {
        let snake_name = self.column_snake_name();
        snake_name
            .split('_')
            .filter_map(|part| part.chars().next())
            .collect::<String>()
            .to_uppercase()
    }

    /// Returns the uppercased acronym ident of this column.
    fn column_acronym_generic(&self) -> syn::GenericParam {
        syn::GenericParam::Type(syn::TypeParam::from(Ident::new(
            &self.column_acronym(),
            proc_macro2::Span::call_site(),
        )))
    }

    /// Returns the snake-cased name of this column.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE my_table (_my__column INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// let column = table.column("_my__column", &db, IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(column.column_snake_name(), "my_column");
    /// # Ok(())
    /// # }
    /// ```
    fn column_snake_name(&self) -> String {
        self.column_name().to_snake_case()
    }

    /// Returns whether the column name is snake case.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    /// let db =
    ///     ParserDB::parse::<GenericDialect>("CREATE TABLE my_table (my_column INT, MyColumn INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// let column1 = table.column("my_column", &db, IdentifierCase::AsWritten)?.unwrap();
    /// let column2 = table.column("MyColumn", &db, IdentifierCase::AsWritten)?.unwrap();
    /// assert!(column1.has_snake_case_column_name());
    /// assert!(!column2.has_snake_case_column_name());
    /// # Ok(())
    /// # }
    /// ```
    fn has_snake_case_column_name(&self) -> bool {
        self.column_snake_name() == self.column_name()
    }

    /// Returns the snake-cased ident of this column.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE my_table (type INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// let column = table.column("type", &db, IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(column.column_snake_ident().to_string(), "r#type");
    /// # Ok(())
    /// # }
    /// ```
    fn column_snake_ident(&self) -> Ident {
        let snake_name = self.column_snake_name();
        if is_reserved_rust_word(&snake_name) {
            Ident::new_raw(&snake_name, proc_macro2::Span::call_site())
        } else {
            Ident::new(&snake_name, proc_macro2::Span::call_site())
        }
    }

    /// Returns the camel-cased name of this column.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    ///
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE my_table (my_column INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// let column = table.column("my_column", &db, IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(column.column_camel_name(), "MyColumn");
    /// # Ok(())
    /// # }
    /// ```
    fn column_camel_name(&self) -> String {
        self.column_name().to_upper_camel_case()
    }

    /// Returns the camel-cased ident of this column.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_traits::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// use synql::prelude::*;
    /// let db = ParserDB::parse::<GenericDialect>("CREATE TABLE my_table (struct INT);")?;
    /// let table =
    ///     db.table_by_target(TargetName::new("my_table", false), IdentifierCase::AsWritten)?.unwrap();
    /// let column = table.column("struct", &db, IdentifierCase::AsWritten)?.unwrap();
    /// assert_eq!(column.column_camel_ident().to_string(), "Struct");
    /// # Ok(())
    /// # }
    /// ```
    fn column_camel_ident(&self) -> Ident {
        Ident::new(&self.column_camel_name(), proc_macro2::Span::call_site())
    }

    /// Returns the type ref curresponding to the postgres type of this column.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the column is defined.
    fn external_postgres_type<'workspace>(
        &self,
        workspace: &'workspace Workspace,
        database: &Self::DB,
    ) -> Option<ExternalTypeRef<'workspace>> {
        workspace.external_postgres_type(&self.normalized_data_type(database))
    }

    /// Returns the Diesel type of this column.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the nullability of the column cannot be
    /// resolved.
    fn diesel_type(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Option<Type>, LookupError> {
        let Some(external_type) = self.external_postgres_type(workspace, database) else {
            return Ok(None);
        };
        let diesel_type = external_type.diesel_type();
        Ok(Some(if self.is_nullable(database)? {
            syn::parse_quote!(diesel::sql_types::Nullable<#diesel_type>)
        } else {
            diesel_type.clone()
        }))
    }

    /// Returns the Rust type of this column.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the nullability of the column cannot be
    /// resolved.
    fn rust_type(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Option<Type>, LookupError> {
        let Some(external_type) = self.external_postgres_type(workspace, database) else {
            return Ok(None);
        };
        let rust_type = external_type.rust_type();
        Ok(Some(if self.is_nullable(database)? {
            syn::parse_quote!(Option<#rust_type>)
        } else {
            rust_type.clone()
        }))
    }

    /// Returns whether the column type supports the `Copy` trait in Rust.
    ///
    /// # Arguments
    ///
    /// * `database` - The database connection to use to query the column type.
    /// * `workspace` - The workspace where the column is defined.
    fn supports_copy(&self, database: &Self::DB, workspace: &Workspace) -> bool {
        match self.external_postgres_type(workspace, database) {
            Some(external_type) => external_type.supports_copy(),
            None => false,
        }
    }

    /// Returns whether the column type supports the given core trait in Rust.
    ///
    /// # Arguments
    ///
    /// * `core_trait` - The core trait to check support for.
    /// * `database` - The database connection to use to query the column type.
    fn supports(
        &self,
        core_trait: crate::structs::Trait,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> bool {
        match self.external_postgres_type(workspace, database) {
            Some(external_type) => external_type.supports_trait(core_trait),
            None => false,
        }
    }

    /// Generates the vertical same-as decorators for this column.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the vertical same-as columns cannot be
    /// resolved.
    fn generate_vertical_same_as_decorators(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let mut vertical_same_as_decorators = vec![];
        for same_as in self.dominant_vertical_same_as_columns(database)? {
            let parent_table = same_as.table(database);
            let parent_table_ident = parent_table.table_snake_ident();
            let parent_column_ident = same_as.column_snake_ident();
            let parent_table_crate = parent_table.crate_ident(workspace);
            vertical_same_as_decorators.push(quote! {
                #[same_as(#parent_table_crate::#parent_table_ident::#parent_column_ident)]
            });
        }
        Ok(vertical_same_as_decorators)
    }

    /// Generates the horizontal same-as decorators for this column.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the horizontal same-as foreign keys or
    /// their tables and columns cannot be resolved.
    fn generate_horizontal_same_as_decorators(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let mut horizontal_same_as_decorators = vec![];
        for same_as in self.horizontal_same_as_foreign_keys(database)? {
            let host_columns = same_as
                .host_columns(database)?
                .map(Borrow::borrow)
                .collect::<Vec<&<Self::DB as DatabaseLike>::Column>>();
            let referenced_columns = same_as
                .referenced_columns(database)?
                .map(Borrow::borrow)
                .collect::<Vec<&<Self::DB as DatabaseLike>::Column>>();
            let satellite_table = same_as.referenced_table(database)?;

            let [key, other] = host_columns.as_slice() else {
                unimplemented!(
                    "Only binary foreign keys are supported for horizontal same-as constraints, found a foreign key in table `{}` to table `{}` with `{}` columns: {}",
                    same_as.host_table(database).table_name(),
                    satellite_table.table_name(),
                    host_columns.len(),
                    host_columns.iter().map(ColumnLike::column_name).collect::<Vec<_>>().join(", ")
                );
            };
            let [_foreign_pk, referenced_column] = referenced_columns.as_slice() else {
                unreachable!();
            };

            if !same_as.is_horizontal_same_as_of_triangular(database)? {
                continue;
            }

            if key == &self.borrow() {
                // The key is handled separately in the discretionary/mandatory
                // decorators
                continue;
            }
            if other != &self.borrow() {
                unreachable!(
                    "The column must be either the key or the other column in the foreign key"
                );
            }
            let key_ident = key.column_snake_ident();
            let satellite_table_ident = satellite_table.table_snake_ident();
            let satellite_table_crate = satellite_table.crate_ident(workspace);
            let referenced_column_ident = referenced_column.column_snake_ident();

            horizontal_same_as_decorators.push(quote! {
                #[same_as(#satellite_table_crate::#satellite_table_ident::#referenced_column_ident, #key_ident)]
            });
        }
        Ok(horizontal_same_as_decorators)
    }

    /// Generates the triangular same-as decorators for this column.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the triangular same-as foreign keys or
    /// their tables cannot be resolved.
    fn generate_triangular_same_as_decorators(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Vec<proc_macro2::TokenStream>, LookupError> {
        let mut triangular_same_as_decorators = vec![];
        for foreign_key in self.triangular_same_as_foreign_keys(database)? {
            let Some(triangular) = foreign_key.triangular_same_as(database)? else {
                continue;
            };

            let referenced_table = foreign_key.referenced_table(database)?;
            let referenced_table_ident = referenced_table.table_snake_ident();
            let referenced_table_crate = referenced_table.crate_ident(workspace);
            let referenced_table_path = quote! { #referenced_table_crate::#referenced_table_ident };

            triangular_same_as_decorators.push(if triangular.is_mandatory() {
                quote! {
                    #[mandatory(#referenced_table_path)]
                }
            } else {
                quote! {
                    #[discretionary(#referenced_table_path)]
                }
            });
        }
        Ok(triangular_same_as_decorators)
    }

    /// Generates the default decorator for this column.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the column is defined.
    /// * `database` - The database connection to use to query the column type.
    ///
    /// # Errors
    ///
    /// * Returns `crate::Error::ColumnTypeNotFound` if the column type cannot
    ///   be found.
    ///
    /// # Panics
    ///
    /// * Panics if the default value cannot be casted to the corresponding Rust
    ///   type.
    fn generate_default_decorator(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<proc_macro2::TokenStream, crate::Error> {
        if self.is_primary_key(database)?
            && self.table(database).has_surrogate_primary_key(database)?
        {
            return Ok(quote! {});
        }
        let Some(default_value) = self.default_value() else {
            return Ok(quote! {});
        };
        let Some(external_postgres_type) = self.external_postgres_type(workspace, database) else {
            return Err(crate::Error::ColumnTypeNotFound {
                table_name: self.table(database).table_name().to_string(),
                column_name: self.column_name().to_string(),
                sql_type: self.data_type(database).to_string(),
            });
        };

        let casted_default_value = match default_value.as_str() {
            candidate if candidate.starts_with("NULL::") => {
                return Ok(quote! {});
            }
            candidate if external_postgres_type.is_string() => {
                let stripped_value =
                    candidate.trim_end_matches("::character varying").trim_matches('\'');
                quote! { #stripped_value }
            }
            "gen_random_uuid()" | "uuidv4()" if external_postgres_type.is_uuid() => {
                quote! { ::uuid::Uuid::new_v4() }
            }
            "gen_random_uuid()" | "uuidv4()" if external_postgres_type.is_rosetta_uuid() => {
                quote! { ::rosetta_uuid::Uuid::new_v4() }
            }
            "uuidv7()" if external_postgres_type.is_rosetta_uuid() => {
                quote! {
                    ::rosetta_uuid::Uuid::utc_v7()
                }
            }
            "CURRENT_TIMESTAMP" if external_postgres_type.crate_name() == "chrono" => {
                quote! { ::chrono::Utc::now() }
            }
            "CURRENT_TIMESTAMP" if external_postgres_type.crate_name() == "rosetta-utc" => {
                quote! { ::rosetta_utc::TimestampUTC::default() }
            }
            _ => {
                let Ok(casted_default_value) = external_postgres_type.cast(&default_value) else {
                    unimplemented!(
                        "Default value `{}` for column `{}` in table `{}` with SQL type `{}` cannot be casted to the corresponding Rust type `{}` from crate `{}`. Please implement the `cast` method for the `ExternalTypeRef` corresponding to this SQL type.",
                        default_value,
                        self.column_name(),
                        self.table(database).table_name(),
                        self.data_type(database),
                        external_postgres_type.rust_type().to_token_stream().to_string(),
                        external_postgres_type.crate_name(),
                    );
                };
                casted_default_value
            }
        };

        Ok(quote! {
            #[table_model(default = #casted_default_value)]
        })
    }

    /// Returns whether a non-tautological check constraint of this column
    /// involves this column alone.
    ///
    /// # Errors
    ///
    /// Returns an error if the check constraints cannot be resolved.
    fn has_single_column_check_constraints(
        &self,
        database: &Self::DB,
    ) -> Result<bool, crate::Error> {
        for check_constraint in self.non_tautological_check_constraints(database)? {
            if check_constraint.number_of_columns(database)? <= 1 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Generates the context-less validations for this column.
    ///
    /// # Errors
    ///
    /// Returns an error if validation generation fails.
    fn generate_context_less_validations(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<Vec<proc_macro2::TokenStream>, crate::Error> {
        let mut validations = vec![];
        for check_constraint in self.non_tautological_check_constraints(database)? {
            if check_constraint.number_of_columns(database)? > 1 {
                continue;
            }
            validations.push(check_constraint.to_syn(database, workspace, &[self.borrow()])?);
        }
        Ok(validations)
    }

    /// Generates the validation impl for this column.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace where the column is defined.
    /// * `database` - The database connection to use to query the column type.
    ///
    /// # Errors
    fn generate_validation_impl(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<proc_macro2::TokenStream, crate::Error> {
        let table_ident = self.table(database).table_snake_ident();
        let column_ident = self.column_snake_ident();
        let external_postgres_type =
            self.external_postgres_type(workspace, database).ok_or_else(|| {
                crate::Error::ColumnTypeNotFound {
                    table_name: self.table(database).table_name().to_string(),
                    column_name: self.column_name().to_string(),
                    sql_type: self.data_type(database).to_string(),
                }
            })?;
        let rust_type = external_postgres_type.rust_type();

        let context_less_validations =
            self.generate_context_less_validations(workspace, database)?;

        let context_less_validation = if context_less_validations.is_empty() {
            None
        } else {
            Some(quote! {
                #[inline]
                fn validate_column(#column_ident: &#rust_type) -> Result<(), Self::Error> {
                    use diesel::Column;
                    #(#context_less_validations)*
                    Ok(())
                }
            })
        };

        Ok(quote! {
            impl ::diesel_builders::ValidateColumn<#table_ident::#column_ident>
                for <#table_ident::table as ::diesel_builders::TableExt>::NewValues
            {
                type Error = ::validation_errors::ValidationError;

                #context_less_validation
            }
        })
    }

    /// Generates the struct field tokens for this column.
    ///
    /// # Errors
    ///
    /// Returns an error if struct field generation fails.
    fn generate_struct_field(
        &self,
        workspace: &Workspace,
        database: &Self::DB,
    ) -> Result<proc_macro2::TokenStream, crate::Error> {
        let column_name = self.column_name();
        let table = self.table(database);
        let (sql_name_decorator, column_ident) = if is_reserved_diesel_keyword(column_name) {
            let ident_str = format!("__{}", self.column_snake_name());
            (
                Some(quote! {#[table_model(sql_name = #column_name)]}),
                Ident::new(&ident_str, proc_macro2::Span::call_site()),
            )
        } else {
            (None, self.column_snake_ident())
        };

        let external_postgres_type =
            self.external_postgres_type(workspace, database).ok_or_else(|| {
                crate::Error::ColumnTypeNotFound {
                    table_name: table.table_name().to_string(),
                    column_name: self.column_name().to_string(),
                    sql_type: self.data_type(database).to_string(),
                }
            })?;
        let documentation = self.column_doc(database)?.map_or_else(
            || {
                format!(
                    "Field representing the `{}` column in table `{}`.",
                    self.column_name(),
                    table.table_name()
                )
            },
            ToString::to_string,
        );
        let rust_type = external_postgres_type.rust_type();
        let diesel_type = external_postgres_type.diesel_type();
        let mut sql_type_decorator = None;
        if !["std", "core"].contains(&external_postgres_type.crate_name()) {
            sql_type_decorator = Some(quote! {
                #[diesel(sql_type = #diesel_type)]
            });
        }

        // If the column has vertical same-as constraint, we add the
        // ` #[same_as(parent::parent_column)]` decorators
        let vertical_same_as_decorators =
            self.generate_vertical_same_as_decorators(workspace, database)?;

        // If the column has horizontal same-as constraint, we add the
        // ` #[same_as(satellite::table::column, key)]` decorators
        let horizontal_same_as_decorators =
            self.generate_horizontal_same_as_decorators(workspace, database)?;

        // If the handle has foreign keys which define discretionary/mandatory
        // same-as constraints, we add the corresponding decorators
        let triangular_same_as_decorators =
            self.generate_triangular_same_as_decorators(workspace, database)?;

        // Generate the default decorator if the column has a default value
        let default_decorator = self.generate_default_decorator(workspace, database)?;

        // A column without single-column check constraints never fails its own
        // validation, since the record validates the multi-column ones.
        let infallible_decorator = if !self.has_single_column_check_constraints(database)?
            && table.has_non_tautological_check_constraints_in_hierarchy(database)?
            && !self.is_surrogate_key(database)?
        {
            Some(quote! {
                #[infallible]
            })
        } else {
            None
        };

        let rust_type = if self.is_nullable(database)? {
            syn::parse_quote!(Option<#rust_type>)
        } else {
            rust_type.clone()
        };

        Ok(quote! {
            #[doc = #documentation]
            #(#vertical_same_as_decorators)*
            #(#horizontal_same_as_decorators)*
            #(#triangular_same_as_decorators)*
            #default_decorator
            #infallible_decorator
            #sql_type_decorator
            #sql_name_decorator
            #column_ident: #rust_type
        })
    }
}

impl<T: ColumnLike> ColumnSynLike for T {}

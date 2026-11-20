//! Submodule defining and implementing the `CheckConstraintSyn` trait, which
//! provide methods to facilitate the rust code generation starting from a SQL
//! column representation, building on top of the
//! [`CheckConstraintLike`] trait.

use proc_macro2::TokenStream;
use quote::quote;
use sql_traits::traits::{CheckConstraintLike, DatabaseLike};

mod interval;
mod like_pattern;
mod sub_expressions;
mod translate_expression;
use translate_expression::{Check, TranslateExpression};

use crate::{
    structs::Workspace,
    traits::{TableSynLike, column::ColumnSynLike},
};

/// Trait implemented by types that represent SQL check constraints and can be
/// used to generate Rust code for them.
pub trait CheckConstraintSynLike: CheckConstraintLike {
    /// Returns the tokenstream representing the check constraint in Rust code.
    ///
    /// PostgreSQL cases text in `lower`, `upper` and `ILIKE` by the collation
    /// of the database, and the generated Rust cases it by the Unicode default
    /// case mappings, with `str::to_lowercase` and `str::to_uppercase` for the
    /// functions and `char::to_lowercase` on each character for `ILIKE`. The
    /// two agree except under collations with their own casing rules, such as
    /// the Turkish dotted and dotless i.
    ///
    /// # Arguments
    ///
    /// * `database` - The database connection to use to query additional
    ///   information about the check constraint if needed.
    /// * `workspace` - The workspace where the generated code will be placed.
    /// * `contextual_columns` - The columns that are in the context where the
    ///   check constraint is applied.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Lookup`](crate::Error::Lookup) when the table, columns
    /// or functions of the check constraint cannot be resolved, and
    /// [`Error::UnsupportedCheckConstraint`](crate::Error::UnsupportedCheckConstraint)
    /// when it uses SQL without a Rust translation.
    fn to_syn<'db>(
        &'db self,
        database: &'db Self::DB,
        workspace: &Workspace,
        contextual_columns: &[&'db <Self::DB as DatabaseLike>::Column],
    ) -> Result<TokenStream, crate::Error> {
        let translator: TranslateExpression<'_, 'db, <Self as CheckConstraintLike>::DB> =
            TranslateExpression::new(self.borrow(), workspace, contextual_columns, database);

        let checks = sub_expressions::sub_expressions(self.expression(database))
            .into_iter()
            .map(|sub_expression| translator.parse(sub_expression))
            .collect::<Result<Vec<Check>, _>>()?;

        let relevant_optional_columns = self
            .columns(database)?
            .filter(|column| !contextual_columns.iter().any(|c| c == column))
            .collect::<Vec<_>>();
        if relevant_optional_columns.is_empty() {
            return Ok(quote! { #(#checks)* });
        }

        let column_idents = relevant_optional_columns
            .iter()
            .map(ColumnSynLike::column_snake_ident)
            .collect::<Vec<_>>();
        let table_ident = self.table(database)?.table_snake_ident();
        let (pattern, values) = if let [column_ident] = column_idents.as_slice() {
            (
                quote! { Some(#column_ident) },
                quote! { <Self as diesel_builders::MayGetColumn<#table_ident::#column_ident>>::may_get_column_ref(self) },
            )
        } else {
            (
                quote! { (#(Some(#column_idents)),*) },
                quote! { (#(<Self as diesel_builders::MayGetColumn<#table_ident::#column_idents>>::may_get_column_ref(self)),*) },
            )
        };
        Ok(match checks.as_slice() {
            // From the 2024 edition on, a single condition joins the pattern
            // in a let chain rather than nesting in its block.
            [Check::Guard { violated, error }] if workspace.edition() >= 2024 => {
                quote! {
                    if let #pattern = #values && #violated {
                        return Err(#error);
                    }
                }
            }
            _ => {
                quote! {
                    if let #pattern = #values {
                        #(#checks)*
                    }
                }
            }
        })
    }
}

impl<T> CheckConstraintSynLike for T where T: CheckConstraintLike {}

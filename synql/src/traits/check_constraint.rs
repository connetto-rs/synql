//! Submodule defining and implementing the `CheckConstraintSyn` trait, which
//! provide methods to facilitate the rust code generation starting from a SQL
//! column representation, building on top of the
//! [`CheckConstraintLike`] trait.

use proc_macro2::TokenStream;
use quote::quote;
use sql_traits::traits::{CheckConstraintLike, DatabaseLike};

mod sub_expressions;
mod translate_expression;
use translate_expression::TranslateExpression;

use crate::{
    structs::Workspace,
    traits::{TableSynLike, column::ColumnSynLike},
};

/// Trait implemented by types that represent SQL check constraints and can be
/// used to generate Rust code for them.
pub trait CheckConstraintSynLike: CheckConstraintLike {
    /// Returns the tokenstream representing the check constraint in Rust code.
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

        let mut translated_expressions: Vec<TokenStream> = Vec::new();

        for sub_expression in sub_expressions::sub_expressions(self.expression(database)) {
            translated_expressions.push(translator.parse(sub_expression)?);
        }

        let relevant_optional_columns = self
            .columns(database)?
            .filter(|column| !contextual_columns.iter().any(|c| c == column))
            .collect::<Vec<_>>();

        Ok(if relevant_optional_columns.is_empty() {
            translated_expressions.into_iter().collect()
        } else {
            let column_idents = relevant_optional_columns
                .iter()
                .map(ColumnSynLike::column_snake_ident)
                .collect::<Vec<_>>();
            let table_ident = self.table(database)?.table_snake_ident();
            if column_idents.len() == 1 {
                quote! {
                    if let #(Some(#column_idents)),* = #(<Self as diesel_builders::MayGetColumn<#table_ident::#column_idents>>::may_get_column_ref(self)),* {
                        #( #translated_expressions )*
                    }
                }
            } else {
                quote! {
                    if let (#(Some(#column_idents)),*) = (#(<Self as diesel_builders::MayGetColumn<#table_ident::#column_idents>>::may_get_column_ref(self)),*) {
                        #( #translated_expressions )*
                    }
                }
            }
        })
    }
}

impl<T> CheckConstraintSynLike for T where T: CheckConstraintLike {}

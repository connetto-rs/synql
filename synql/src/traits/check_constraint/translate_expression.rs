//! Submodule providing the `TranslateExpression` struct for translating SQL
//! check constraint expressions into Rust code.
//!
//! A check constraint accepts a row when its expression is TRUE or NULL and
//! rejects it when the expression is FALSE. Operands that may be NULL
//! translate to `Option` values, `None` standing for NULL, and every operator
//! propagates NULL as SQL's three-valued logic does.

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use sql_traits::{
    errors::{LookupError, ObjectKind},
    structs::IdentifierCase,
    traits::{CheckConstraintLike, ColumnLike, DatabaseLike, TableLike},
};
use sqlparser::ast::{
    BinaryOperator, CastKind, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments,
    Ident, Value, ValueWithSpan,
};

use crate::{
    Error,
    structs::{ExternalTypeRef, UnsupportedCheck, Workspace},
    traits::{column::ColumnSynLike, function::FunctionSynLike, table::TableSynLike},
};

/// How tightly a Rust expression binds, which decides where it needs
/// parentheses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    /// A literal, path, call or method chain.
    Postfix,
    /// A dereference.
    Prefix,
    /// A binary operation or a `match`.
    Loose,
}

/// A SQL expression translated to a Rust expression.
struct Operand<'workspace, 'db, DB: DatabaseLike> {
    /// The Rust expression.
    tokens: TokenStream,
    /// How tightly the Rust expression binds.
    binding: Binding,
    /// Whether the expression evaluates to an `Option`, `None` standing for
    /// SQL NULL.
    nullable: bool,
    /// Whether the expression evaluates to a reference.
    by_reference: bool,
    /// The Rust type of the value.
    ty: ExternalTypeRef<'workspace>,
    /// The columns the expression reads.
    columns: Vec<&'db DB::Column>,
}

impl<DB: DatabaseLike> Operand<'_, '_, DB> {
    /// Returns the expression ready to have a method called on it.
    fn receiver(&self) -> TokenStream {
        let tokens = &self.tokens;
        if self.binding == Binding::Postfix {
            quote! { #tokens }
        } else {
            quote! { (#tokens) }
        }
    }

    /// Returns the expression ready to be the operand of an operator.
    fn operand(&self) -> TokenStream {
        let tokens = &self.tokens;
        if self.binding == Binding::Loose {
            quote! { (#tokens) }
        } else {
            quote! { #tokens }
        }
    }

    /// Returns the expression as an `Option`, wrapping it when it cannot be
    /// NULL.
    fn as_option(&self) -> TokenStream {
        let tokens = &self.tokens;
        if self.nullable {
            quote! { #tokens }
        } else {
            quote! { Some(#tokens) }
        }
    }

    /// Returns the expression as an `Option` ready to have a method called on
    /// it.
    fn as_option_receiver(&self) -> TokenStream {
        if self.nullable { self.receiver() } else { self.as_option() }
    }

    /// Returns the constant truth value of the expression, if it is a boolean
    /// literal.
    fn constant(&self) -> Option<bool> {
        if self.nullable {
            return None;
        }
        match self.tokens.to_string().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }
}

/// Returns the Rust spelling of the type.
fn type_name(ty: ExternalTypeRef<'_>) -> String {
    ty.rust_type().to_token_stream().to_string()
}

/// Returns the positional and named arguments of the call, none for SQL's
/// keyword calls such as `CURRENT_TIMESTAMP`.
fn call_arguments(function: &Function) -> &[FunctionArg] {
    match &function.args {
        FunctionArguments::List(list) => &list.args,
        FunctionArguments::None | FunctionArguments::Subquery(_) => &[],
    }
}

/// Returns the inner expression of any parenthesised expression.
fn strip_nesting(expr: &Expr) -> &Expr {
    match expr {
        Expr::Nested(inner) => strip_nesting(inner),
        _ => expr,
    }
}

/// Returns whether the expression is a call to SQL's `now`.
fn is_now(expr: &Expr) -> bool {
    matches!(strip_nesting(expr), Expr::Function(function) if function.name.to_string().eq_ignore_ascii_case("now"))
}

/// Returns whether the expression takes its type from the other operand, as
/// a literal or `now()` does.
fn takes_type_hint(expr: &Expr) -> bool {
    matches!(strip_nesting(expr), Expr::Value(_)) || is_now(expr)
}

/// Returns the operator with its operands swapped, for the comparison
/// operators.
fn invert_operator(op: &BinaryOperator) -> Option<BinaryOperator> {
    Some(match op {
        BinaryOperator::Eq => BinaryOperator::Eq,
        BinaryOperator::NotEq => BinaryOperator::NotEq,
        BinaryOperator::Gt => BinaryOperator::Lt,
        BinaryOperator::Lt => BinaryOperator::Gt,
        BinaryOperator::GtEq => BinaryOperator::LtEq,
        BinaryOperator::LtEq => BinaryOperator::GtEq,
        _ => return None,
    })
}

/// Returns the Rust comparison operator for the SQL comparison operator.
fn comparison_operator(op: &BinaryOperator) -> Option<TokenStream> {
    Some(match op {
        BinaryOperator::Eq => quote! { == },
        BinaryOperator::NotEq => quote! { != },
        BinaryOperator::Gt => quote! { > },
        BinaryOperator::Lt => quote! { < },
        BinaryOperator::GtEq => quote! { >= },
        BinaryOperator::LtEq => quote! { <= },
        _ => return None,
    })
}

/// Returns the Rust arithmetic operator for the SQL arithmetic operator.
fn arithmetic_operator(op: &BinaryOperator) -> Option<TokenStream> {
    Some(match op {
        BinaryOperator::Plus => quote! { + },
        BinaryOperator::Minus => quote! { - },
        BinaryOperator::Multiply => quote! { * },
        BinaryOperator::Divide => quote! { / },
        BinaryOperator::Modulo => quote! { % },
        _ => return None,
    })
}

pub(super) struct TranslateExpression<'workspace, 'db, DB: DatabaseLike> {
    check_constraint: &'db DB::CheckConstraint,
    workspace: &'workspace Workspace,
    contextual_columns: &'workspace [&'db DB::Column],
    database: &'db DB,
}

impl<'workspace, 'db, DB> TranslateExpression<'workspace, 'db, DB>
where
    DB: DatabaseLike,
{
    pub(super) fn new(
        check_constraint: &'db DB::CheckConstraint,
        workspace: &'workspace Workspace,
        contextual_columns: &'workspace [&'db DB::Column],
        database: &'db DB,
    ) -> Self {
        Self { check_constraint, workspace, contextual_columns, database }
    }

    /// Returns the error reporting that the check constraint uses an
    /// untranslatable construct.
    fn unsupported(&self, reason: UnsupportedCheck) -> Error {
        Error::UnsupportedCheckConstraint {
            check: self.check_constraint.expression(self.database).to_string(),
            reason,
        }
    }

    /// Returns the table of the check constraint.
    fn table(&self) -> Result<&'db DB::Table, LookupError> {
        self.check_constraint.table(self.database)
    }

    /// Returns the tokens of the name of the table of the check constraint.
    fn table_name(&self) -> Result<TokenStream, LookupError> {
        let table_ident = self.table()?.table_snake_ident();
        Ok(quote! { <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME })
    }

    /// Returns the tokens of the names of the provided columns, each once.
    fn column_names(&self, columns: &[&'db DB::Column]) -> Result<Vec<TokenStream>, LookupError> {
        let table_ident = self.table()?.table_snake_ident();
        let mut seen: Vec<&DB::Column> = Vec::with_capacity(columns.len());
        let mut names = Vec::with_capacity(columns.len());
        for &column in columns {
            if seen.contains(&column) {
                continue;
            }
            seen.push(column);
            let column_ident = column.column_snake_ident();
            names.push(quote! { crate::#table_ident::#column_ident::NAME });
        }
        Ok(names)
    }

    /// Returns the requested function of the check constraint.
    fn function(&self, name: &str) -> Result<&'db DB::Function, LookupError> {
        self.check_constraint
            .function(self.database, name)?
            .ok_or_else(|| ObjectKind::Function.not_in_database(name))
    }

    /// Returns the requested column of the check constraint.
    fn column(&self, name: &str) -> Result<&'db DB::Column, LookupError> {
        let column = self.check_constraint.column(self.database, name, IdentifierCase::Exact)?;
        column.ok_or_else(|| ObjectKind::Column.not_in_database(name))
    }

    /// Returns whether the column is the one being validated on its own,
    /// whose value is always present.
    fn is_contextual_column(&self, column: &DB::Column) -> bool {
        self.contextual_columns.contains(&column)
    }

    /// Returns whether the column may hold NULL where the check reads it.
    fn is_optional_column(&self, column: &DB::Column) -> Result<bool, LookupError> {
        Ok(column.is_nullable(self.database)? && !self.is_contextual_column(column))
    }

    /// Returns the Rust type of the column.
    fn column_type(&self, column: &DB::Column) -> Result<ExternalTypeRef<'workspace>, Error> {
        column.external_postgres_type(self.workspace, self.database).ok_or_else(|| {
            self.unsupported(UnsupportedCheck::ColumnType(format!(
                "{}.{}",
                column.table(self.database).table_name(),
                column.column_name()
            )))
        })
    }

    /// Translates the check constraint sub-expression into statements that
    /// return an error when it evaluates to FALSE.
    pub(super) fn parse(&self, expr: &Expr) -> Result<TokenStream, Error> {
        if let Some(tokens) = self.map_expr_to_validation_error(expr)? {
            return Ok(tokens);
        }
        if let Expr::Function(function) = strip_nesting(expr) {
            return self.validation_call(function);
        }

        let operand = self.operand(expr, None)?;
        if !operand.ty.is_bool() {
            return Err(self.unsupported(UnsupportedCheck::Expression(expr.to_string())));
        }
        let tokens = operand.operand();
        let violated = if operand.nullable {
            quote! { #tokens == Some(false) }
        } else {
            quote! { !#tokens }
        };
        let table_name = self.table_name()?;
        let fields = self.column_names(&operand.columns)?;
        let check = expr.to_string();
        Ok(quote! {
            if #violated {
                return Err(::validation_errors::ValidationError::check_violated(
                    #table_name,
                    &[#(#fields),*],
                    #check,
                ));
            }
        })
    }

    /// Maps the expression to a dedicated validation error, when one exists
    /// for its shape.
    fn map_expr_to_validation_error(&self, expr: &Expr) -> Result<Option<TokenStream>, Error> {
        let Expr::BinaryOp { left, right, op } = expr else {
            return Ok(None);
        };
        Ok(match (left.as_ref(), right.as_ref()) {
            (
                Expr::Identifier(Ident { value: ident, .. }),
                Expr::Value(ValueWithSpan { value, .. }),
            ) => self.map_value_expr_to_single_field_error(ident, value, op)?,
            (Expr::Identifier(Ident { value: ident, .. }), Expr::Function(func))
                if func.name.to_string() == "NOW" =>
            {
                self.map_expr_to_in_the_past_error(ident, op)?
            }
            (Expr::Function(func), Expr::Value(ValueWithSpan { value, .. }))
                if func.name.to_string() == "length" =>
            {
                self.map_expr_to_max_length_error(func, value, op)?
            }
            (
                Expr::Value(ValueWithSpan { value, .. }),
                Expr::Identifier(Ident { value: ident, .. }),
            ) => {
                match invert_operator(op) {
                    Some(inverted) => {
                        self.map_value_expr_to_single_field_error(ident, value, &inverted)?
                    }
                    None => None,
                }
            }
            (
                Expr::Identifier(Ident { value: left_ident, .. }),
                Expr::Identifier(Ident { value: right_ident, .. }),
            ) => self.map_expr_to_double_field_error(left_ident, right_ident, op)?,
            _ => None,
        })
    }

    /// Maps `column < NOW()` and `column <= NOW()` to the in-the-past error.
    fn map_expr_to_in_the_past_error(
        &self,
        ident: &str,
        op: &BinaryOperator,
    ) -> Result<Option<TokenStream>, Error> {
        let column = self.column(ident)?;
        if self.is_optional_column(column)? {
            return Ok(None);
        }
        let operator = match op {
            BinaryOperator::Lt => quote! { >= },
            BinaryOperator::LtEq => quote! { > },
            _ => return Ok(None),
        };
        let column_ident = column.column_snake_ident();
        let table_ident = self.table()?.table_snake_ident();
        let table_name = self.table_name()?;
        Ok(Some(quote! {
            if *#column_ident #operator ::rosetta_utc::TimestampUTC::now() {
                return Err(::validation_errors::ValidationError::in_the_future(
                    #table_name,
                    crate::#table_ident::#column_ident::NAME,
                ));
            }
        }))
    }

    /// Maps `length(column) <op> n` to the maximum length error.
    fn map_expr_to_max_length_error(
        &self,
        func: &Function,
        value: &Value,
        op: &BinaryOperator,
    ) -> Result<Option<TokenStream>, Error> {
        let [FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Identifier(ident)))] =
            call_arguments(func)
        else {
            return Ok(None);
        };
        let column = self.column(&ident.value)?;
        let violation = match op {
            BinaryOperator::Lt => quote! { >= },
            BinaryOperator::LtEq => quote! { > },
            _ => return Ok(None),
        };
        if self.is_optional_column(column)? {
            return Ok(None);
        }
        let column_ident = column.column_snake_ident();
        let table_ident = self.table()?.table_snake_ident();
        let table_name = self.table_name()?;
        let max_length = self.parse_value(value, Some(self.workspace.usize()))?.0;
        Ok(Some(quote! {
            if #column_ident.chars().count() #violation #max_length {
                return Err(::validation_errors::ValidationError::exceeds_max_length(
                    #table_name,
                    crate::#table_ident::#column_ident::NAME,
                    #max_length
                ));
            }
        }))
    }

    /// Maps a comparison between two columns to the dedicated error of its
    /// operator.
    fn map_expr_to_double_field_error(
        &self,
        left: &str,
        right: &str,
        op: &BinaryOperator,
    ) -> Result<Option<TokenStream>, Error> {
        let (violation, constructor) = match op {
            BinaryOperator::NotEq => (quote! { == }, quote! { equal }),
            BinaryOperator::LtEq => (quote! { > }, quote! { smaller_than }),
            BinaryOperator::Lt => (quote! { >= }, quote! { strictly_smaller_than }),
            BinaryOperator::Gt => (quote! { <= }, quote! { strictly_greater_than }),
            BinaryOperator::GtEq => (quote! { < }, quote! { greater_than }),
            _ => return Ok(None),
        };
        let left_column = self.column(left)?;
        let right_column = self.column(right)?;
        let table_ident = self.table()?.table_snake_ident();
        let table_name = self.table_name()?;
        let left_ident = left_column.column_snake_ident();
        let right_ident = right_column.column_snake_ident();
        let comparison = match (
            self.is_optional_column(left_column)?,
            self.is_optional_column(right_column)?,
        ) {
            (true, true) => {
                quote! {
                    #left_ident.as_ref().is_some_and(|#left_ident|
                        #right_ident.as_ref().is_some_and(|#right_ident| #left_ident #violation #right_ident)
                    )
                }
            }
            (true, false) => {
                quote! {
                    #left_ident.as_ref().is_some_and(|#left_ident| #left_ident #violation #right_ident)
                }
            }
            (false, true) => {
                quote! {
                    #right_ident.as_ref().is_some_and(|#right_ident| #left_ident #violation #right_ident)
                }
            }
            (false, false) => quote! { #left_ident #violation #right_ident },
        };
        Ok(Some(quote! {
            if #comparison {
                return Err(::validation_errors::ValidationError::#constructor(
                    #table_name,
                    crate::#table_ident::#left_ident::NAME,
                    crate::#table_ident::#right_ident::NAME,
                ));
            }
        }))
    }

    /// Maps a comparison between a column that cannot be NULL and a literal to
    /// the dedicated error of its operator.
    fn map_value_expr_to_single_field_error(
        &self,
        ident: &str,
        value: &Value,
        op: &BinaryOperator,
    ) -> Result<Option<TokenStream>, Error> {
        let column = self.column(ident)?;
        if self.is_optional_column(column)? {
            return Ok(None);
        }
        let column_ident = column.column_snake_ident();
        let table_ident = self.table()?.table_snake_ident();
        let table_name = self.table_name()?;
        if *op == BinaryOperator::NotEq {
            if !column.is_textual(self.database)
                || *value != Value::SingleQuotedString(String::new())
            {
                return Ok(None);
            }
            return Ok(Some(quote! {
                if #column_ident.is_empty() {
                    return Err(::validation_errors::ValidationError::empty(
                        #table_name,
                        crate::#table_ident::#column_ident::NAME
                    ));
                }
            }));
        }
        let (violation, constructor) = match op {
            BinaryOperator::LtEq => (quote! { > }, quote! { smaller_than_value }),
            BinaryOperator::Lt => (quote! { >= }, quote! { strictly_smaller_than_value }),
            BinaryOperator::Gt => (quote! { <= }, quote! { strictly_greater_than_value }),
            BinaryOperator::GtEq => (quote! { < }, quote! { greater_than_value }),
            _ => return Ok(None),
        };
        let column_value = self.parse_value(value, Some(self.column_type(column)?))?.0;
        let float_value = self.parse_value(value, Some(self.workspace.f64()))?.0;
        Ok(Some(quote! {
            if #column_ident #violation &#column_value {
                return Err(::validation_errors::ValidationError::#constructor(
                    #table_name,
                    crate::#table_ident::#column_ident::NAME,
                    #float_value
                ));
            }
        }))
    }

    /// Translates the literal into a Rust value of the hinted type.
    fn parse_value(
        &self,
        value: &Value,
        type_hint: Option<ExternalTypeRef<'workspace>>,
    ) -> Result<(TokenStream, ExternalTypeRef<'workspace>), Error> {
        match value {
            Value::Boolean(value) => Ok((quote! { #value }, self.workspace.bool())),
            Value::Number(number, _) => {
                let ty = type_hint.ok_or_else(|| {
                    self.unsupported(UnsupportedCheck::UntypedNumber(number.clone()))
                })?;
                let tokens = ty.cast(number).map_err(|_| {
                    self.unsupported(UnsupportedCheck::MixedTypes {
                        left: type_name(ty),
                        right: number.clone(),
                    })
                })?;
                Ok((tokens, ty))
            }
            Value::SingleQuotedString(string) => Ok((quote! { #string }, self.workspace.string())),
            other => Err(self.unsupported(UnsupportedCheck::Expression(other.to_string()))),
        }
    }

    /// Translates the column into the value the check reads.
    fn column_operand(
        &self,
        column: &'db DB::Column,
    ) -> Result<Operand<'workspace, 'db, DB>, Error> {
        let ty = self.column_type(column)?;
        let nullable = self.is_optional_column(column)?;
        let ident = column.column_snake_ident();
        let (tokens, binding, by_reference) = if ty.is_string() {
            let tokens = if nullable {
                quote! { #ident.as_deref() }
            } else {
                quote! { #ident.as_str() }
            };
            (tokens, Binding::Postfix, true)
        } else if ty.supports_copy() {
            (quote! { *#ident }, Binding::Prefix, false)
        } else {
            let tokens = if nullable {
                quote! { #ident.as_ref() }
            } else {
                quote! { #ident }
            };
            (tokens, Binding::Postfix, true)
        };
        Ok(Operand { tokens, binding, nullable, by_reference, ty, columns: vec![column] })
    }

    /// Translates both operands of a binary operator, giving a literal the
    /// type of the other operand.
    fn operand_pair(
        &self,
        left: &Expr,
        right: &Expr,
        type_hint: Option<ExternalTypeRef<'workspace>>,
    ) -> Result<(Operand<'workspace, 'db, DB>, Operand<'workspace, 'db, DB>), Error> {
        let (left, right) = if takes_type_hint(left) && !takes_type_hint(right) {
            let right = self.operand(right, type_hint)?;
            (self.operand(left, Some(right.ty))?, right)
        } else {
            let left = self.operand(left, type_hint)?;
            let right = self.operand(right, Some(left.ty))?;
            (left, right)
        };
        if left.ty != right.ty {
            return Err(self.unsupported(UnsupportedCheck::MixedTypes {
                left: type_name(left.ty),
                right: type_name(right.ty),
            }));
        }
        Ok((left, right))
    }

    /// Combines two operands, returning NULL when either is NULL.
    fn lift(
        left: Operand<'workspace, 'db, DB>,
        right: Operand<'workspace, 'db, DB>,
        ty: ExternalTypeRef<'workspace>,
        combine: impl Fn(&TokenStream, &TokenStream) -> TokenStream,
    ) -> Operand<'workspace, 'db, DB> {
        let nullable = left.nullable || right.nullable;
        let (tokens, binding) = if nullable {
            let (left_option, right_option) = (left.as_option_receiver(), right.as_option());
            let combined = combine(&quote! { left }, &quote! { right });
            (
                quote! { #left_option.zip(#right_option).map(|(left, right)| #combined) },
                Binding::Postfix,
            )
        } else {
            (combine(&left.operand(), &right.operand()), Binding::Loose)
        };
        let mut columns = left.columns;
        columns.extend(right.columns);
        Operand { tokens, binding, nullable, by_reference: false, ty, columns }
    }

    /// Combines two boolean operands with SQL's AND or OR.
    fn logical(
        &self,
        op: &BinaryOperator,
        left: Operand<'workspace, 'db, DB>,
        right: Operand<'workspace, 'db, DB>,
    ) -> Result<Operand<'workspace, 'db, DB>, Error> {
        if !left.ty.is_bool() || !right.ty.is_bool() {
            return Err(self.unsupported(UnsupportedCheck::Operator(op.to_string())));
        }
        let is_and = *op == BinaryOperator::And;
        // FALSE decides an AND and TRUE decides an OR, whatever the other
        // operand, while the other literal leaves the other operand's value.
        let absorbing = !is_and;
        let mut columns = left.columns.clone();
        columns.extend(right.columns.iter().copied());
        let bool_type = self.workspace.bool();
        if left.constant() == Some(absorbing) || right.constant() == Some(absorbing) {
            return Ok(Operand {
                tokens: quote! { #absorbing },
                binding: Binding::Postfix,
                nullable: false,
                by_reference: false,
                ty: bool_type,
                columns,
            });
        }
        if left.constant().is_some() {
            return Ok(Operand { columns, ..right });
        }
        if right.constant().is_some() {
            return Ok(Operand { columns, ..left });
        }
        let nullable = left.nullable || right.nullable;
        let tokens = if nullable {
            let (left_option, right_option) = (left.as_option(), right.as_option());
            quote! {
                match (#left_option, #right_option) {
                    (Some(#absorbing), _) | (_, Some(#absorbing)) => Some(#absorbing),
                    (Some(_), Some(_)) => Some(!#absorbing),
                    _ => None,
                }
            }
        } else {
            let (left_tokens, right_tokens) = (left.operand(), right.operand());
            if is_and {
                quote! { #left_tokens && #right_tokens }
            } else {
                quote! { #left_tokens || #right_tokens }
            }
        };
        Ok(Operand {
            tokens,
            binding: Binding::Loose,
            nullable,
            by_reference: false,
            ty: bool_type,
            columns,
        })
    }

    /// Translates `IS NULL` or `IS NOT NULL`, which is never NULL itself.
    fn null_test(&self, expr: &Expr, is_null: bool) -> Result<Operand<'workspace, 'db, DB>, Error> {
        let operand = self.operand(expr, None)?;
        let tokens = match (operand.nullable, is_null) {
            (true, true) => {
                let tokens = operand.receiver();
                quote! { #tokens.is_none() }
            }
            (true, false) => {
                let tokens = operand.receiver();
                quote! { #tokens.is_some() }
            }
            (false, is_null) => {
                let never_null = !is_null;
                quote! { #never_null }
            }
        };
        Ok(Operand {
            tokens,
            binding: Binding::Postfix,
            nullable: false,
            by_reference: false,
            ty: self.workspace.bool(),
            columns: operand.columns,
        })
    }

    /// Translates the expression into the Rust value it evaluates to.
    fn operand(
        &self,
        expr: &Expr,
        type_hint: Option<ExternalTypeRef<'workspace>>,
    ) -> Result<Operand<'workspace, 'db, DB>, Error> {
        match expr {
            Expr::Nested(inner) => self.operand(inner, type_hint),
            Expr::Cast { kind, expr: inner, format: None, .. }
                if *kind == CastKind::DoubleColon =>
            {
                self.operand(inner, type_hint)
            }
            Expr::Cast { .. } => Err(self.unsupported(UnsupportedCheck::Cast(expr.to_string()))),
            Expr::Identifier(ident) => self.column_operand(self.column(&ident.value)?),
            Expr::Value(ValueWithSpan { value, .. }) => {
                let (tokens, ty) = self.parse_value(value, type_hint)?;
                let by_reference = matches!(value, Value::SingleQuotedString(_));
                Ok(Operand {
                    tokens,
                    binding: Binding::Postfix,
                    nullable: false,
                    by_reference,
                    ty,
                    columns: Vec::new(),
                })
            }
            Expr::IsNull(inner) => self.null_test(inner, true),
            Expr::IsNotNull(inner) => self.null_test(inner, false),
            Expr::Function(function)
                if function.name.to_string().eq_ignore_ascii_case("length") =>
            {
                self.length_operand(function)
            }
            Expr::Function(function) if is_now(expr) => self.now_operand(function, type_hint),
            Expr::Function(function) => self.function_operand(function),
            Expr::BinaryOp { left, op: op @ (BinaryOperator::And | BinaryOperator::Or), right } => {
                let left = self.operand(left, None)?;
                let right = self.operand(right, None)?;
                self.logical(op, left, right)
            }
            Expr::BinaryOp { left, op, right } => {
                if let Some(operator) = comparison_operator(op) {
                    let (left, right) = self.operand_pair(left, right, None)?;
                    return Ok(Self::lift(left, right, self.workspace.bool(), |left, right| {
                        quote! { #left #operator #right }
                    }));
                }
                let Some(operator) = arithmetic_operator(op) else {
                    return Err(self.unsupported(UnsupportedCheck::Operator(op.to_string())));
                };
                let (left, right) = self.operand_pair(left, right, type_hint)?;
                if !left.ty.is_numeric() {
                    return Err(self.unsupported(UnsupportedCheck::Operator(op.to_string())));
                }
                let ty = left.ty;
                Ok(Self::lift(left, right, ty, |left, right| quote! { #left #operator #right }))
            }
            _ => Err(self.unsupported(UnsupportedCheck::Expression(expr.to_string()))),
        }
    }

    /// Translates SQL's `now()` compared with a timestamp.
    fn now_operand(
        &self,
        function: &Function,
        type_hint: Option<ExternalTypeRef<'workspace>>,
    ) -> Result<Operand<'workspace, 'db, DB>, Error> {
        let without_arguments = call_arguments(function).is_empty();
        let Some(ty) = type_hint
            .filter(|ty| without_arguments && type_name(*ty) == ":: rosetta_utc :: TimestampUTC")
        else {
            return Err(self.unsupported(UnsupportedCheck::FunctionCall(function.to_string())));
        };
        Ok(Operand {
            tokens: quote! { ::rosetta_utc::TimestampUTC::now() },
            binding: Binding::Postfix,
            nullable: false,
            by_reference: false,
            ty,
            columns: Vec::new(),
        })
    }

    /// Translates SQL's `length` of a text, which counts its characters.
    fn length_operand(&self, function: &Function) -> Result<Operand<'workspace, 'db, DB>, Error> {
        let [FunctionArg::Unnamed(FunctionArgExpr::Expr(argument))] = call_arguments(function)
        else {
            return Err(self.unsupported(UnsupportedCheck::FunctionCall(function.to_string())));
        };
        let text = self.operand(argument, Some(self.workspace.string()))?;
        if !text.ty.is_string() {
            return Err(self.unsupported(UnsupportedCheck::FunctionCall(function.to_string())));
        }
        let tokens = text.receiver();
        let tokens = if text.nullable {
            quote! { #tokens.map(|text| text.chars().count()) }
        } else {
            quote! { #tokens.chars().count() }
        };
        Ok(Operand {
            tokens,
            binding: Binding::Postfix,
            nullable: text.nullable,
            by_reference: false,
            ty: self.workspace.usize(),
            columns: text.columns,
        })
    }

    /// Resolves the Rust function of a SQL function call and translates its
    /// arguments.
    fn function_call(
        &self,
        function: &Function,
    ) -> Result<(TokenStream, Vec<Operand<'workspace, 'db, DB>>), Error> {
        let Function {
            name,
            uses_odbc_syntax,
            parameters,
            args,
            filter,
            null_treatment,
            over,
            within_group,
        } = function;
        let has_argument_clauses = matches!(
            args,
            FunctionArguments::List(list)
                if list.duplicate_treatment.is_some() || !list.clauses.is_empty()
        );
        if has_argument_clauses
            || *uses_odbc_syntax
            || !matches!(parameters, FunctionArguments::None)
            || filter.is_some()
            || null_treatment.is_some()
            || over.is_some()
            || !within_group.is_empty()
        {
            return Err(self.unsupported(UnsupportedCheck::FunctionCall(function.to_string())));
        }
        let arguments = call_arguments(function);
        let name = name.to_string();
        let sql_function = self.function(&name)?;
        let path = sql_function
            .external_function_ref(self.workspace)
            .ok_or_else(|| self.unsupported(UnsupportedCheck::UnmappedFunction(name.clone())))?
            .path()
            .clone();
        let argument_types =
            sql_function.argument_types(self.workspace, self.database).collect::<Vec<_>>();
        if argument_types.len() != arguments.len() {
            return Err(self.unsupported(UnsupportedCheck::FunctionCall(function.to_string())));
        }
        let mut operands = Vec::with_capacity(arguments.len());
        for (index, (argument, argument_type)) in arguments.iter().zip(argument_types).enumerate() {
            let FunctionArg::Unnamed(FunctionArgExpr::Expr(argument)) = argument else {
                return Err(self.unsupported(UnsupportedCheck::FunctionCall(function.to_string())));
            };
            let argument_type = argument_type.ok_or_else(|| {
                self.unsupported(UnsupportedCheck::FunctionArgumentType {
                    function: name.clone(),
                    index,
                })
            })?;
            operands.push(self.operand(argument, Some(argument_type))?);
        }
        Ok((quote! { #path }, operands))
    }

    /// Builds an expression applying `body` to the call of the function on
    /// its arguments, mapped over the arguments that may be NULL.
    ///
    /// Returns the expression and whether it is mapped over an `Option`.
    fn map_call(
        path: &TokenStream,
        arguments: &[Operand<'workspace, 'db, DB>],
        body: impl Fn(TokenStream) -> TokenStream,
    ) -> (TokenStream, bool) {
        let mut call_arguments = Vec::with_capacity(arguments.len());
        let mut zipped: Option<(TokenStream, TokenStream)> = None;
        for (index, argument) in arguments.iter().enumerate() {
            let value = if argument.nullable {
                let binding = format_ident!("argument{index}");
                zipped = Some(match zipped {
                    None => (quote! { #binding }, argument.receiver()),
                    Some((pattern, option)) => {
                        let tokens = &argument.tokens;
                        (quote! { (#pattern, #binding) }, quote! { #option.zip(#tokens) })
                    }
                });
                quote! { #binding }
            } else if argument.by_reference {
                argument.tokens.clone()
            } else {
                argument.operand()
            };
            call_arguments.push(if argument.by_reference {
                value
            } else {
                quote! { &#value }
            });
        }
        let call = body(quote! { #path(#(#call_arguments),*) });
        match zipped {
            None => (call, false),
            Some((pattern, option)) => (quote! { #option.map(|#pattern| #call) }, true),
        }
    }

    /// Translates a call to a validation function used as a boolean, which is
    /// TRUE when the function accepts its arguments.
    fn function_operand(&self, function: &Function) -> Result<Operand<'workspace, 'db, DB>, Error> {
        let (path, arguments) = self.function_call(function)?;
        let (tokens, nullable) = Self::map_call(&path, &arguments, |call| quote! { #call.is_ok() });
        let columns = arguments.into_iter().flat_map(|argument| argument.columns).collect();
        Ok(Operand {
            tokens,
            binding: Binding::Postfix,
            nullable,
            by_reference: false,
            ty: self.workspace.bool(),
            columns,
        })
    }

    /// Translates a check that is a single call to a validation function,
    /// returning the function's own error.
    fn validation_call(&self, function: &Function) -> Result<TokenStream, Error> {
        let (path, arguments) = self.function_call(function)?;
        let columns = arguments
            .iter()
            .flat_map(|argument| argument.columns.iter().copied())
            .collect::<Vec<_>>();
        let table_name = self.table_name()?;
        let fields = self.column_names(&columns)?;
        let (call, nullable) = Self::map_call(&path, &arguments, |call| {
            quote! {
                #call.map_err(|error| ::validation_errors::ValidationError::generic(
                    #table_name,
                    vec![#(#fields),*],
                    Box::new(error),
                ))
            }
        });
        Ok(if nullable {
            quote! { #call.transpose()?; }
        } else {
            quote! { #call?; }
        })
    }
}

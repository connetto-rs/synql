//! Submodule providing the `TranslateExpression` struct for translating SQL
//! check constraint expressions into Rust code.
use proc_macro2::TokenStream;
use quote::quote;
use sql_traits::{
    errors::{LookupError, ObjectKind},
    structs::IdentifierCase,
    traits::{CheckConstraintLike, ColumnLike, DatabaseLike, FunctionLike, TableLike},
};
use sqlparser::ast::{
    BinaryOperator, Expr, FunctionArg, FunctionArgExpr, FunctionArgumentList, FunctionArguments,
    Ident, Value, ValueWithSpan,
};

use crate::{
    structs::{ExternalFunctionRef, ExternalTypeRef, Workspace},
    traits::{column::ColumnSynLike, function::FunctionSynLike, table::TableSynLike},
};

/// A translated expression, the columns it scopes and the type it returns.
type ParsedExpression<'a, 'workspace, DB> =
    (TokenStream, Vec<&'a <DB as DatabaseLike>::Column>, Option<ExternalTypeRef<'workspace>>);

pub(super) struct TranslateExpression<'workspace, 'db, DB: DatabaseLike> {
    check_constraint: &'db DB::CheckConstraint,
    workspace: &'workspace Workspace,
    contextual_columns: &'workspace [&'db DB::Column],
    database: &'db DB,
}

/// Verifies that the [`CastKind`](sqlparser::ast::CastKind) is supported
///
/// # Arguments
///
/// * `kind` - The [`CastKind`](sqlparser::ast::CastKind) to verify
fn verify_cast_kind(kind: &sqlparser::ast::CastKind) {
    match kind {
        sqlparser::ast::CastKind::DoubleColon => {}
        _ => {
            unimplemented!("Unsupported cast kind: {kind:?}");
        }
    }
}

/// Returns the direction-inverted operator for the provided binary operator.
fn invert_operator(op: &BinaryOperator) -> BinaryOperator {
    match op {
        BinaryOperator::Eq => BinaryOperator::Eq,
        BinaryOperator::NotEq => BinaryOperator::NotEq,
        BinaryOperator::Gt => BinaryOperator::Lt,
        BinaryOperator::Lt => BinaryOperator::Gt,
        BinaryOperator::GtEq => BinaryOperator::LtEq,
        BinaryOperator::LtEq => BinaryOperator::GtEq,
        _ => {
            unimplemented!("Cannot invert unsupported operator: {op:?}");
        }
    }
}

/// Returns the syn version of the provided binary operator.
fn syn_operator(op: &BinaryOperator) -> TokenStream {
    match op {
        BinaryOperator::Eq => quote! { == },
        BinaryOperator::NotEq => quote! { != },
        BinaryOperator::Gt => quote! { > },
        BinaryOperator::Lt => quote! { < },
        BinaryOperator::GtEq => quote! { >= },
        BinaryOperator::LtEq => quote! { <= },
        _ => {
            unimplemented!("Unsupported operator: {op:?}");
        }
    }
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

    /// Maps the provided expression to a validation error, when applicable.
    fn map_expr_to_validation_error(
        &self,
        expr: &Expr,
    ) -> Result<Option<TokenStream>, LookupError> {
        Ok(match expr {
            Expr::BinaryOp { left, right, op } => {
                match (left.as_ref(), right.as_ref()) {
                    (
                        Expr::Identifier(Ident { value: ident, .. }),
                        Expr::Value(ValueWithSpan { value, .. }),
                    ) => Some(self.map_value_expr_to_single_field_error(ident, value, op)?),
                    (Expr::Identifier(Ident { value: ident, .. }), Expr::Function(func))
                        if func.name.to_string() == "NOW" =>
                    {
                        let column = self.column(ident)?;
                        let column_ident = column.column_snake_ident();
                        let table_ident = self.table()?.table_snake_ident();

                        assert!(matches!(op, BinaryOperator::LtEq | BinaryOperator::Lt));

                        let operator = syn_operator(&invert_operator(op));

                        Some(quote! {
                            if *#column_ident #operator ::rosetta_utc::TimestampUTC::now() {
                                return Err(::validation_errors::ValidationError::in_the_future(
                                    <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME,
                                    crate::#table_ident::#column_ident::NAME,
                                ));
                            }
                        })
                    }
                    (Expr::Function(func), Expr::Value(ValueWithSpan { value, .. }))
                        if func.name.to_string() == "length" =>
                    {
                        let string_type = self.workspace.string();
                        let (parsed_arguments, columns) =
                            self.parse_function_arguments(&func.args, &[string_type])?;
                        assert_eq!(columns.len(), 1);
                        let column = columns[0];
                        let parsed_argument = &parsed_arguments[0];
                        let table_ident = self.table()?.table_snake_ident();
                        let column_ident = column.column_snake_ident();
                        let value_usize = self.parse_value(value, Some(self.workspace.usize())).0;
                        let operator = syn_operator(&invert_operator(op));
                        Some(quote! {
                            if #parsed_argument.len() #operator #value_usize {
                                return Err(::validation_errors::ValidationError::exceeds_max_length(
                                    <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME,
                                    crate::#table_ident::#column_ident::NAME,
                                    #value_usize
                                ));
                            }
                        })
                    }
                    (
                        Expr::Value(ValueWithSpan { value, .. }),
                        Expr::Identifier(Ident { value: ident, .. }),
                    ) => {
                        Some(self.map_value_expr_to_single_field_error(
                            ident,
                            value,
                            &invert_operator(op),
                        )?)
                    }
                    (
                        Expr::Identifier(Ident { value: left_ident, .. }),
                        Expr::Identifier(Ident { value: right_ident, .. }),
                    ) => Some(self.map_expr_to_double_field_error(left_ident, right_ident, op)?),
                    _ => None,
                }
            }
            _ => None,
        })
    }

    fn is_contextual_column(&self, column: &DB::Column) -> bool {
        self.contextual_columns.contains(&column)
    }

    fn map_expr_to_double_field_error(
        &self,
        left: &str,
        right: &str,
        op: &BinaryOperator,
    ) -> Result<TokenStream, LookupError> {
        let left_column = self.column(left)?;
        let right_column = self.column(right)?;
        let table_ident = self.table()?.table_snake_ident();
        let left_column_ident = left_column.column_snake_ident();
        let right_column_ident = right_column.column_snake_ident();
        let l_name = quote! { crate::#table_ident::#left_column_ident::NAME };
        let r_name = quote! { crate::#table_ident::#right_column_ident::NAME };
        let validation_error = quote! { ::validation_errors::ValidationError };
        let left_optional =
            left_column.is_nullable(self.database)? && !self.is_contextual_column(left_column);
        let right_optional =
            right_column.is_nullable(self.database)? && !self.is_contextual_column(right_column);
        let compare_op = |op: TokenStream| {
            match (left_optional, right_optional) {
                (true, true) => {
                    quote! {
                        #left_column_ident.as_ref().is_some_and(|#left_column_ident|
                            #right_column_ident.as_ref().is_some_and(|#right_column_ident|
                                #left_column_ident #op #right_column_ident
                            )
                        )
                    }
                }
                (true, false) => {
                    quote! {
                        #left_column_ident.as_ref().is_some_and(|#left_column_ident|
                            #left_column_ident #op #right_column_ident
                        )
                    }
                }
                (false, true) => {
                    quote! {
                        #right_column_ident.as_ref().is_some_and(|#right_column_ident|
                            #left_column_ident #op #right_column_ident
                        )
                    }
                }
                (false, false) => {
                    quote! {
                        #left_column_ident #op #right_column_ident
                    }
                }
            }
        };
        Ok(match op {
            BinaryOperator::NotEq => {
                let compare_op = compare_op(quote! {==});
                quote! {
                    if #compare_op {
                        return Err(#validation_error::equal(<crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME, #l_name, #r_name));
                    }
                }
            }
            BinaryOperator::LtEq => {
                let compare_op = compare_op(quote! {>});
                quote! {
                    if #compare_op {
                        return Err(#validation_error::smaller_than(<crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME, #l_name, #r_name));
                    }
                }
            }
            BinaryOperator::Lt => {
                let compare_op = compare_op(quote! {>=});
                quote! {
                    if #compare_op {
                        return Err(#validation_error::strictly_smaller_than(<crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME, #l_name, #r_name));
                    }
                }
            }
            BinaryOperator::Gt => {
                let compare_op = compare_op(quote! {<=});
                quote! {
                    if #compare_op {
                        return Err(#validation_error::strictly_greater_than(<crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME, #l_name, #r_name));
                    }
                }
            }
            BinaryOperator::GtEq => {
                let compare_op = compare_op(quote! {<});
                quote! {
                    if #compare_op {
                        return Err(#validation_error::greater_than(<crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME, #l_name, #r_name));
                    }
                }
            }
            _ => {
                unimplemented!("Operator {op:?} not supported for double field error mapping");
            }
        })
    }

    fn map_value_expr_to_single_field_error(
        &self,
        ident: &str,
        value: &Value,
        op: &BinaryOperator,
    ) -> Result<TokenStream, LookupError> {
        let column = self.column(ident)?;
        let column_ident = column.column_snake_ident();
        let table_ident = self.table()?.table_snake_ident();
        Ok(match op {
            BinaryOperator::NotEq => {
                if column.is_textual(self.database)
                    && value == &Value::SingleQuotedString(String::new())
                {
                    quote! {
                        if #column_ident.is_empty() {
                            return Err(::validation_errors::ValidationError::empty(
                                <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME,
                                crate::#table_ident::#column_ident::NAME
                            ));
                        }
                    }
                } else {
                    unimplemented!("Operator {op:?} not supported for single field error mapping");
                }
            }
            BinaryOperator::LtEq => {
                let column_value = self.parse_column_value(column, value).0;
                let float_value = self.parse_value(value, Some(self.workspace.f64())).0;
                quote! {
                    if #column_ident > &#column_value {
                        return Err(::validation_errors::ValidationError::smaller_than_value(
                            <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME,
                            crate::#table_ident::#column_ident::NAME,
                            #float_value
                        ));
                    }
                }
            }
            BinaryOperator::Lt => {
                let column_value = self.parse_column_value(column, value).0;
                let float_value = self.parse_value(value, Some(self.workspace.f64())).0;
                quote! {
                    if #column_ident >= &#column_value {
                        return Err(::validation_errors::ValidationError::strictly_smaller_than_value(
                            <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME,
                            crate::#table_ident::#column_ident::NAME,
                            #float_value
                        ));
                    }
                }
            }
            BinaryOperator::Gt => {
                let column_value = self.parse_column_value(column, value).0;
                let float_value = self.parse_value(value, Some(self.workspace.f64())).0;
                quote! {
                    if #column_ident <= &#column_value {
                        return Err(::validation_errors::ValidationError::strictly_greater_than_value(
                            <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME,
                            crate::#table_ident::#column_ident::NAME,
                            #float_value
                        ));
                    }
                }
            }
            BinaryOperator::GtEq => {
                let column_value = self.parse_column_value(column, value).0;
                let float_value = self.parse_value(value, Some(self.workspace.f64())).0;
                quote! {
                    if #column_ident < &#column_value {
                        return Err(::validation_errors::ValidationError::greater_than_value(
                            <crate::#table_ident::table as ::diesel_builders::TableExt>::TABLE_NAME,
                            crate::#table_ident::#column_ident::NAME,
                            #float_value
                        ));
                    }
                }
            }
            _ => {
                unimplemented!("Operator {op:?} not supported for single field error mapping");
            }
        })
    }

    /// Returns reference to the table of the check constraint.
    fn table(&self) -> Result<&DB::Table, LookupError> {
        self.check_constraint.table(self.database)
    }

    /// Returns reference to the requested function by name.
    fn function(&self, name: &str) -> Result<&DB::Function, LookupError> {
        self.check_constraint
            .function(self.database, name)?
            .ok_or_else(|| ObjectKind::Function.not_in_database(name))
    }

    /// Returns reference to the requested involved column by name.
    fn column(&self, name: &str) -> Result<&DB::Column, LookupError> {
        self.check_constraint
            .column(self.database, name, IdentifierCase::Exact)?
            .ok_or_else(|| ObjectKind::Column.not_in_database(name))
    }

    /// Translates the provided function argument to a
    /// [`TokenStream`]
    fn parse_function_argument_expr(
        &self,
        arg: &FunctionArgExpr,
        arg_type: ExternalTypeRef<'workspace>,
    ) -> Result<(TokenStream, Option<&'_ DB::Column>), LookupError> {
        match arg {
            FunctionArgExpr::Expr(expr) => {
                let (token_stream, mut scoped_columns, _returning_type) =
                    self.inner_parse(expr, Some(arg_type))?;
                if scoped_columns.len() > 1 {
                    unimplemented!("Multiple scoped columns not supported");
                }
                Ok((token_stream, scoped_columns.pop()))
            }
            FunctionArgExpr::QualifiedWildcard(_) => {
                unimplemented!("QualifiedWildcard not supported");
            }
            FunctionArgExpr::Wildcard | FunctionArgExpr::WildcardWithOptions(_) => {
                unimplemented!("Wildcard not supported");
            }
        }
    }

    /// Translates the provided function argument to a
    /// [`TokenStream`]
    fn parse_function_argument(
        &self,
        arg: &FunctionArg,
        arg_type: ExternalTypeRef<'workspace>,
    ) -> Result<(TokenStream, Option<&'_ DB::Column>), LookupError> {
        match arg {
            FunctionArg::Named { .. } => {
                unimplemented!("Named arguments not supported");
            }
            FunctionArg::ExprNamed { .. } => {
                unimplemented!("ExprNamed arguments not supported");
            }
            FunctionArg::Unnamed(arg) => self.parse_function_argument_expr(arg, arg_type),
        }
    }

    /// Translates the provided list of function arguments to a
    /// [`TokenStream`]
    fn parse_function_argument_list(
        &self,
        args: &FunctionArgumentList,
        argument_types: &[ExternalTypeRef<'workspace>],
    ) -> Result<(Vec<TokenStream>, Vec<&'_ DB::Column>), LookupError> {
        let mut token_stream = Vec::with_capacity(args.args.len());
        let mut columns = Vec::new();
        assert_eq!(args.args.len(), argument_types.len());
        for (arg, arg_type) in args.args.iter().zip(argument_types.iter().copied()) {
            let (column_token_stream, column) = self.parse_function_argument(arg, arg_type)?;
            token_stream.push(column_token_stream);
            columns.extend(column);
        }
        Ok((token_stream, columns))
    }

    /// Translates the provided function arguments to a
    /// [`TokenStream`]
    fn parse_function_arguments(
        &self,
        args: &FunctionArguments,
        argument_types: &[ExternalTypeRef<'workspace>],
    ) -> Result<(Vec<TokenStream>, Vec<&'_ DB::Column>), LookupError> {
        match args {
            FunctionArguments::None => Ok((Vec::new(), Vec::new())),
            FunctionArguments::Subquery(_) => {
                unimplemented!("Subquery arguments not supported");
            }
            FunctionArguments::List(args) => {
                self.parse_function_argument_list(args, argument_types)
            }
        }
    }

    /// Translates the provided SQL function call to a
    /// [`TokenStream`]
    fn parse_function(
        &self,
        sqlparser::ast::Function {
            name,
            uses_odbc_syntax,
            parameters,
            args,
            filter,
            null_treatment,
            over,
            within_group,
        }: &sqlparser::ast::Function,
    ) -> Result<(TokenStream, Option<ExternalTypeRef<'workspace>>), LookupError> {
        if !within_group.is_empty() {
            unimplemented!("WithinGroup not supported");
        }
        if null_treatment.is_some() {
            unimplemented!("NullTreatment not supported");
        }
        if !matches!(parameters, FunctionArguments::None) {
            unimplemented!("Parameters not supported");
        }
        if over.is_some() {
            unimplemented!("Over not supported");
        }
        if filter.is_some() {
            unimplemented!("Filter not supported");
        }
        if *uses_odbc_syntax {
            unimplemented!("ODBC syntax not supported");
        }
        let function = self.function(&name.to_string())?;

        let argument_types = function
            .argument_types(self.workspace, self.database)
            .map(|arg_type| {
                arg_type.unwrap_or_else(|| {
                    panic!("Failed to get type for argument of function `{}`", function.name())
                })
            })
            .collect::<Vec<ExternalTypeRef>>();

        let (args, scoped_columns) = self.parse_function_arguments(args, &argument_types)?;

        let function_ref: ExternalFunctionRef =
            function.external_function_ref(self.workspace).unwrap_or_else(|| {
                panic!(
                    "The function `{}` should have an external function reference",
                    function.name()
                )
            });

        let table_ident = self.table()?.table_snake_ident();

        let attributes = scoped_columns.iter().map(|scoped_column| {
            let column_ident = scoped_column.column_snake_ident();
            quote! { crate::#table_ident::#column_ident::NAME }
        });

        let map_err = match scoped_columns.len() {
            1 => {
                quote! {
                    .map_err(|e| {
                        use validation_errors::prelude::ReplaceFieldName;
                        e.replace_field_name(|_|#(#attributes),* )
                    })
                }
            }
            2 => {
                quote! {
                    .map_err(|e| {
                        use validation_errors::prelude::ReplaceFieldName;
                        e.replace_field_names(|_|#(#attributes),* )
                    })
                }
            }
            _ => {
                unimplemented!("More than two scoped columns not supported");
            }
        };

        Ok((
            quote! {
                #function_ref(#(#args),*)#map_err
            },
            None,
        ))
    }

    /// Parses the provided [`Value`] for the provided
    /// column.
    ///
    /// # Arguments
    ///
    /// * `column` - The column for which the value is being parsed
    /// * `value` - The [`Value`] to
    ///
    /// # Panics
    ///
    /// * If the provided [`Value`] is not supported
    /// * If the type of the provided column cannot be determined
    fn parse_column_value(
        &self,
        column: &DB::Column,
        value: &Value,
    ) -> (proc_macro2::TokenStream, ExternalTypeRef<'workspace>) {
        let column_type =
            column.external_postgres_type(self.workspace, self.database).unwrap_or_else(|| {
                panic!(
                    "Failed to get type for column `{}.{}` ({})",
                    column.table(self.database).table_name(),
                    column.column_name(),
                    column.normalized_data_type(self.database)
                )
            });
        self.parse_value(value, Some(column_type))
    }

    /// Parses the provided [`Value`] to a
    /// [`TokenStream`]
    ///
    /// # Arguments
    ///
    /// * `value` - The [`Value`] to parse
    /// * `type_hint` - The [`ExternalTypeRef`] of the value
    ///
    /// # Panics
    ///
    /// * If the provided [`Value`] is not supported
    fn parse_value(
        &self,
        value: &Value,
        type_hint: Option<ExternalTypeRef<'workspace>>,
    ) -> (proc_macro2::TokenStream, ExternalTypeRef<'workspace>) {
        match value {
            Value::Boolean(value) => (quote! { #value }, self.workspace.bool()),
            Value::Number(value, _) => {
                match type_hint {
                    Some(type_hint) => (type_hint.cast(value).unwrap(), type_hint),
                    None => {
                        unimplemented!(
                            "Number without type hint not supported: {:?}",
                            self.check_constraint
                        );
                    }
                }
            }
            Value::SingleQuotedString(value) => (quote! { #value }, self.workspace.string()),
            other => {
                unimplemented!("Unsupported value: {:?}", other);
            }
        }
    }

    /// Parses the provided [`ValueWithSpan`] to
    /// a [`TokenStream`]
    ///
    /// # Arguments
    ///
    /// * `value` - The [`ValueWithSpan`] to parse
    /// * `type_hint` - The [`ExternalTypeRef`] of the value
    ///
    /// # Panics
    ///
    /// * If the provided [`ValueWithSpan`] is not supported
    fn parse_value_with_span(
        &self,
        value: &sqlparser::ast::ValueWithSpan,
        type_hint: Option<ExternalTypeRef<'workspace>>,
    ) -> (proc_macro2::TokenStream, ExternalTypeRef<'workspace>) {
        self.parse_value(&value.value, type_hint)
    }

    #[allow(clippy::too_many_lines)]
    /// Translates the provided expression to a
    /// [`TokenStream`]
    pub(super) fn parse(&self, expr: &Expr) -> Result<TokenStream, LookupError> {
        if let Some(validation_error_token) = self.map_expr_to_validation_error(expr)? {
            return Ok(validation_error_token);
        }

        let (internal_token, scoped_columns, _returning_type) = self.inner_parse(expr, None)?;

        if !scoped_columns.is_empty() {
            unimplemented!("Scoped columns not supported");
        }

        Ok(quote! {
            #internal_token?;
        })
    }

    #[allow(clippy::too_many_lines)]
    /// Translates the provided expression to a
    /// [`TokenStream`]
    fn inner_parse(
        &self,
        expr: &Expr,
        type_hint: Option<ExternalTypeRef<'workspace>>,
    ) -> Result<ParsedExpression<'_, 'workspace, DB>, LookupError> {
        Ok(match expr {
            Expr::Function(function) => {
                let (token_stream, return_type) = self.parse_function(function)?;
                (token_stream, Vec::new(), return_type)
            }
            Expr::Cast { kind, expr, data_type: _, format } => {
                verify_cast_kind(kind);
                if format.is_some() {
                    unimplemented!("Format not supported");
                }
                self.inner_parse(expr, type_hint)?
            }
            Expr::Nested(expr) => self.inner_parse(expr, type_hint)?,
            Expr::Identifier(ident) => {
                let column = self.column(&ident.value)?;
                let column_ident = column.column_snake_ident();
                (
                    quote! {
                        #column_ident
                    },
                    vec![column],
                    Some(
                        column
                            .external_postgres_type(self.workspace, self.database)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Failed to get type for column `{}.{}` ({})",
                                    column.table(self.database).table_name(),
                                    column.column_name(),
                                    column.normalized_data_type(self.database)
                                )
                            }),
                    ),
                )
            }
            Expr::BinaryOp { left, op, right } => {
                match op {
                    BinaryOperator::And => {
                        let (left, left_scoped_columns, left_returning_type) =
                            self.inner_parse(left, None)?;
                        let (right, right_scoped_columns, right_returning_type) =
                            self.inner_parse(right, None)?;
                        if !left_scoped_columns.is_empty() || !right_scoped_columns.is_empty() {
                            unimplemented!("Scoped columns not supported");
                        }
                        let left_returning_type =
                            left_returning_type.expect("Left side of AND must have a type");
                        let right_returning_type =
                            right_returning_type.expect("Right side of AND must have a type");
                        if left_returning_type.is_bool() && right_returning_type.is_bool() {
                            (
                                match (left.to_string().as_str(), right.to_string().as_str()) {
                                    ("true", "true") => quote! { true },
                                    ("false", _) | (_, "false") => quote! { false },
                                    ("true", _) => quote! { #right },
                                    (_, "true") => quote! { #left },
                                    (_, _) => quote! { #left && #right },
                                },
                                Vec::new(),
                                Some(self.workspace.bool()),
                            )
                        } else {
                            unimplemented!("Unsupported binary operation");
                        }
                    }
                    BinaryOperator::Or => {
                        let (left, left_scoped_columns, left_returning_type) =
                            self.inner_parse(left, None)?;
                        let (right, right_scoped_columns, right_returning_type) =
                            self.inner_parse(right, None)?;
                        if !left_scoped_columns.is_empty() || !right_scoped_columns.is_empty() {
                            unimplemented!("Scoped columns not supported");
                        }
                        let left_returning_type =
                            left_returning_type.expect("Left side of AND must have a type");
                        let right_returning_type =
                            right_returning_type.expect("Right side of AND must have a type");
                        if left_returning_type.is_bool() && right_returning_type.is_bool() {
                            (
                                match (left.to_string().as_str(), right.to_string().as_str()) {
                                    ("false", "false") => quote! { false },
                                    ("true", _) | (_, "true") => quote! { true },
                                    ("false", _) => quote! { #right },
                                    (_, "false") => quote! { #left },
                                    (_, _) => quote! { #left || #right },
                                },
                                Vec::new(),
                                Some(self.workspace.bool()),
                            )
                        } else {
                            unimplemented!("Unsupported binary operation");
                        }
                    }
                    BinaryOperator::NotEq
                    | BinaryOperator::Eq
                    | BinaryOperator::Gt
                    | BinaryOperator::Lt
                    | BinaryOperator::GtEq
                    | BinaryOperator::LtEq => {
                        let (left, _, left_returning_type) = self.inner_parse(left, None)?;
                        let left_returning_type =
                            left_returning_type.expect("Left side of AND must have a type");
                        let (right, _, right_returning_type) =
                            self.inner_parse(right, Some(left_returning_type))?;
                        let right_returning_type =
                            right_returning_type.expect("Right side of AND must have a type");
                        if left_returning_type != right_returning_type {
                            unimplemented!(
                                "Equality between different types not supported: {left_returning_type:?} and {right_returning_type:?}. {:?}",
                                self.check_constraint
                            );
                        }
                        let operator_symbol: syn::BinOp = match op {
                            BinaryOperator::Eq => syn::BinOp::Eq(syn::token::EqEq::default()),
                            BinaryOperator::NotEq => syn::BinOp::Ne(syn::token::Ne::default()),
                            BinaryOperator::Gt => syn::BinOp::Gt(syn::token::Gt::default()),
                            BinaryOperator::Lt => syn::BinOp::Lt(syn::token::Lt::default()),
                            BinaryOperator::GtEq => syn::BinOp::Ge(syn::token::Ge::default()),
                            BinaryOperator::LtEq => syn::BinOp::Le(syn::token::Le::default()),
                            _ => unreachable!(),
                        };
                        (
                            quote! {
                                #left #operator_symbol #right
                            },
                            Vec::new(),
                            Some(self.workspace.bool()),
                        )
                    }
                    BinaryOperator::Plus
                    | BinaryOperator::Minus
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => {
                        let (left, _, left_returning_type) = self.inner_parse(left, type_hint)?;
                        let (right, _, right_returning_type) =
                            self.inner_parse(right, type_hint)?;
                        if left_returning_type != right_returning_type {
                            unimplemented!(
                                "Binary operation between different types not supported: {left_returning_type:?} and {right_returning_type:?}. {:?}",
                                self.check_constraint
                            );
                        }
                        let left_returning_type =
                            left_returning_type.expect("Left side of binary op must have a type");
                        let right_returning_type =
                            right_returning_type.expect("Right side of binary op must have a type");
                        if left_returning_type.is_numeric() && right_returning_type.is_numeric() {
                            let operator_symbol: syn::BinOp = match op {
                                BinaryOperator::Plus => {
                                    syn::BinOp::Add(syn::token::Plus::default())
                                }
                                BinaryOperator::Minus => {
                                    syn::BinOp::Sub(syn::token::Minus::default())
                                }
                                BinaryOperator::Multiply => {
                                    syn::BinOp::Mul(syn::token::Star::default())
                                }
                                BinaryOperator::Divide => {
                                    syn::BinOp::Div(syn::token::Slash::default())
                                }
                                BinaryOperator::Modulo => {
                                    syn::BinOp::Rem(syn::token::Percent::default())
                                }
                                _ => unreachable!(),
                            };
                            (
                                quote! {
                                    #left #operator_symbol #right
                                },
                                Vec::new(),
                                Some(left_returning_type),
                            )
                        } else {
                            unimplemented!(
                                "Unsupported binary operation {} between {:?} and {:?}",
                                op,
                                left_returning_type,
                                right_returning_type
                            );
                        }
                    }
                    operator => {
                        unimplemented!("Unsupported binary operator: {operator:?}");
                    }
                }
            }
            Expr::Value(value) => {
                let (token_stream, returning_type) = self.parse_value_with_span(value, type_hint);
                (token_stream, Vec::new(), Some(returning_type))
            }
            Expr::IsNull(expr) => {
                if let Expr::Identifier(Ident { value: ident, .. }) = expr.as_ref() {
                    let column = self.column(ident)?;
                    if !column.is_nullable(self.database)? {
                        unimplemented!(
                            "IS NULL on non-nullable column `{}` not supported. {:?}",
                            ident,
                            self.check_constraint
                        );
                    }
                    if self.is_contextual_column(column) {
                        (
                            quote! {
                                false
                            },
                            Vec::new(),
                            Some(self.workspace.bool()),
                        )
                    } else {
                        let column_ident = column.column_snake_ident();
                        (
                            quote! {
                                #column_ident.is_none()
                            },
                            Vec::new(),
                            Some(self.workspace.bool()),
                        )
                    }
                } else {
                    let (inner_token, _scoped_columns, _returning_type) =
                        self.inner_parse(expr, None)?;
                    (
                        quote! {
                            #inner_token.is_none()
                        },
                        Vec::new(),
                        Some(self.workspace.bool()),
                    )
                }
            }
            Expr::IsNotNull(expr) => {
                if let Expr::Identifier(Ident { value: ident, .. }) = expr.as_ref() {
                    let column = self.column(ident)?;
                    if !column.is_nullable(self.database)? {
                        unimplemented!(
                            "IS NOT NULL on non-nullable column `{}` not supported. {:?}",
                            ident,
                            self.check_constraint
                        );
                    }
                    if self.is_contextual_column(column) {
                        (
                            quote! {
                                true
                            },
                            Vec::new(),
                            Some(self.workspace.bool()),
                        )
                    } else {
                        let column_ident = column.column_snake_ident();
                        (
                            quote! {
                                #column_ident.is_some()
                            },
                            Vec::new(),
                            Some(self.workspace.bool()),
                        )
                    }
                } else {
                    let (inner_token, _scoped_columns, _returning_type) =
                        self.inner_parse(expr, None)?;
                    (
                        quote! {
                            #inner_token.is_some()
                        },
                        Vec::new(),
                        Some(self.workspace.bool()),
                    )
                }
            }
            _ => {
                unimplemented!(
                    "Unsupported expression: {:?}, from check constraint: {:?}",
                    expr,
                    self.check_constraint
                )
            }
        })
    }
}

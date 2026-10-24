//! Submodule providing error types used in SynQL.

/// Enum representing errors that can occur in SynQL.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "Column type not found for column `{column_name}` in table `{table_name}` with SQL type `{sql_type}`"
    )]
    /// Unknown column type error.
    ColumnTypeNotFound {
        /// Name of the table where the error occurred.
        table_name: String,
        /// Name of the column whose type was not found.
        column_name: String,
        /// SQL type of the column.
        sql_type: String,
    },
    #[error("Function definition not found for function `{function_name}`")]
    /// Function definition not found error.
    FunctionNotFound {
        /// Name of the function whose definition was not found.
        function_name: String,
    },
    #[error("Database contains more than {0} columns, which is unsupported")]
    /// Error indicating the database has more columns than supported.
    TooManyColumns(usize),
    #[error("I/O error: {0}")]
    /// I/O error.
    IO(#[from] std::io::Error),
    #[error("Invalid TOML dependency: {0}")]
    /// Invalid TOML dependency error.
    InvalidTomlDependency(String),
    #[error("Workspace member `{0}` has no path")]
    /// A workspace member was declared without a path.
    WorkspaceMemberWithoutPath(String),
    #[error("Schema lookup failed: {0}")]
    /// A schema object could not be resolved in the database.
    Lookup(#[from] sql_traits::errors::LookupError),
    #[error("Schema analysis failed: {0}")]
    /// The schema could not be analysed, for instance ordered into a DAG.
    Schema(#[from] sql_traits::errors::Error),
    #[error("Check constraint `{check}` cannot be translated: unsupported {reason}")]
    /// A check constraint uses SQL that has no Rust translation.
    UnsupportedCheckConstraint {
        /// SQL expression of the check constraint.
        check: String,
        /// The construct that has no translation.
        reason: UnsupportedCheck,
    },
}

/// SQL constructs of a check constraint that have no Rust translation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UnsupportedCheck {
    /// An expression kind the translator does not handle.
    #[error("expression `{0}`")]
    Expression(String),
    /// A binary operator applied to operands it cannot combine.
    #[error("operator `{0}`")]
    Operator(String),
    /// A cast spelled with something other than `::`.
    #[error("cast `{0}`")]
    Cast(String),
    /// A number with no typed operand to take its type from.
    #[error("untyped number `{0}`")]
    UntypedNumber(String),
    /// Operands of different Rust types.
    #[error("comparison of `{left}` with `{right}`")]
    MixedTypes {
        /// The left operand.
        left: String,
        /// The right operand.
        right: String,
    },
    /// A column whose SQL type has no Rust type.
    #[error("type of column `{0}`")]
    ColumnType(String),
    /// A function with no Rust implementation registered.
    #[error("function `{0}` without a Rust implementation")]
    UnmappedFunction(String),
    /// A function argument whose SQL type has no Rust type.
    #[error("argument {index} of function `{function}`")]
    FunctionArgumentType {
        /// The function.
        function: String,
        /// Zero-based position of the argument.
        index: usize,
    },
    /// A function call clause or argument form other than plain positional
    /// arguments.
    #[error("function call `{0}`")]
    FunctionCall(String),
}

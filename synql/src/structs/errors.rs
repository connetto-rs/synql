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
    #[error(
        "Default `{default}` of column `{column_name}` in table `{table_name}` has no Rust translation"
    )]
    /// A column default with no Rust translation for the type of the column.
    UnsupportedDefault {
        /// Name of the table of the column.
        table_name: String,
        /// Name of the column.
        column_name: String,
        /// SQL expression of the default.
        default: String,
    },
    #[error(
        "Column `{column_name}` in table `{table_name}` is a TIMESTAMP without time zone, which names no instant. Declare it TIMESTAMPTZ"
    )]
    /// A column of type `TIMESTAMP` without time zone, which synql refuses
    /// because it holds a wall clock reading of an unknown time zone.
    TimestampWithoutTimeZone {
        /// Name of the table of the column.
        table_name: String,
        /// Name of the column.
        column_name: String,
    },
    #[error(
        "Generated column `{column_name}` is part of the primary key of table `{table_name}`, whose other key columns the builder sets"
    )]
    /// A stored generated column in a primary key whose other columns the
    /// builder sets.
    GeneratedKeyColumn {
        /// Name of the table of the column.
        table_name: String,
        /// Name of the generated column.
        column_name: String,
    },
    #[error(
        "Column `{column_name}` in table `{table_name}` is a TIME WITH TIME ZONE, an offset without a date that cannot follow daylight saving. Declare it TIME or TIMESTAMPTZ"
    )]
    /// A column of type `TIME WITH TIME ZONE`, which synql refuses because
    /// its offset, lacking a date, cannot follow daylight saving.
    TimeWithTimeZone {
        /// Name of the table of the column.
        table_name: String,
        /// Name of the column.
        column_name: String,
    },
    #[error("The {first} and the {second} would both generate the Rust crate `{crate_ident}`")]
    /// Two generated crates whose names fold to the same Rust crate name, as
    /// the crates of the enum types `"Mood"` and `mood`, or of the enum type
    /// `mood` and the table `public_mood`.
    CrateNameClash {
        /// The table or enum type generated first.
        first: String,
        /// The table or enum type generated second.
        second: String,
        /// The Rust crate name both crates would have.
        crate_ident: String,
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
    #[error("Table grouping failed: {0}")]
    /// The tables could not be grouped into crates, for instance because the
    /// modularity resolution is invalid.
    Grouping(#[from] geometric_traits::traits::ModularityError),
    #[error("Check constraint `{check}` cannot be translated: unsupported {reason}")]
    /// A check constraint uses SQL that has no Rust translation.
    UnsupportedCheckConstraint {
        /// SQL expression of the check constraint.
        check: String,
        /// The construct that has no translation.
        reason: UnsupportedCheck,
    },
    #[error("Code generated for `{}` does not parse as a Rust file: {source}", path.display())]
    /// The code generated for a file, items returned by callbacks included,
    /// does not parse as a Rust file.
    UnparsableCode {
        /// Path of the file the code was generated for.
        path: std::path::PathBuf,
        /// The parse error.
        source: syn::Error,
    },
    #[error("The base workspace description does not parse: {0}")]
    /// A base workspace description that is not the TOML synql writes.
    InvalidBaseDescription(#[from] toml::de::Error),
    #[error(
        "Table `{0}` of the base workspace is missing from the schema, which must apply the base SQL first"
    )]
    /// A table of the base workspace that the schema does not hold, because
    /// the schema was not parsed on top of the base SQL.
    MissingBaseTable(String),
    #[error(
        "Type `{0}` of the base workspace is missing from the schema, which must apply the base SQL first"
    )]
    /// An enum type of the base workspace that the schema does not hold,
    /// because the schema was not parsed on top of the base SQL.
    MissingBaseType(String),
    #[error(
        "Table `{base_table}` of the base workspace depends on table `{generated_table}`, which its crate cannot reach"
    )]
    /// A table of the base workspace altered to depend on a table generated
    /// on top of it, which the published base crate does not know.
    BaseTableDependsOnGeneratedTable {
        /// The table of the base workspace.
        base_table: String,
        /// The table generated on top of the base workspace.
        generated_table: String,
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
    /// An interval whose length depends on the calendar or the time zone, or
    /// whose spelling the translator does not read as PostgreSQL does.
    #[error("interval `{0}`")]
    Interval(String),
    /// A NaN literal, which PostgreSQL orders above every other float and
    /// equal to itself while Rust leaves it unordered.
    #[error("NaN literal `{0}`")]
    NotANumber(String),
}

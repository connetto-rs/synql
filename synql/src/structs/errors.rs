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
    #[error("Schema lookup failed: {0}")]
    /// A schema object could not be resolved in the database.
    Lookup(#[from] sql_traits::errors::LookupError),
    #[error("Schema analysis failed: {0}")]
    /// The schema could not be analysed, for instance ordered into a DAG.
    Schema(#[from] sql_traits::errors::Error),
}

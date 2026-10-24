//! Submodule defining the `SQLWorkspace` trait which allows to generate the
//! `diesel` workspace from a SQL schema, based on `sql_traits`.

use std::path::Path;

use proc_macro2::TokenStream;

mod builder;
mod write_crate_lib;
mod write_crate_toml;
mod write_sink_crate_lib;
mod write_sink_crate_toml;
pub use builder::SynQLBuilder;
use sql_relations::prelude::TableLike;
use time_requirements::{prelude::TimeTracker, task::Task};

use crate::{
    structs::{ExternalCrate, TomlDependency, Workspace, external_crate::MaximalNumberOfColumns},
    traits::{SynQLDatabaseLike, table::TableSynLike},
};

/// Type alias for the callback function used to generate additional code for
/// tables.
pub type Callback<'db, T, D> =
    Box<dyn Fn(&T, &D, &Workspace) -> Result<Option<TokenStream>, crate::Error> + 'db>;

/// Type alias for the callback function used to generate additional
/// dependencies for tables.
pub type TomlCallback<'db, T, D> =
    Box<dyn Fn(&T, &D) -> Result<Option<TomlDependency>, crate::Error> + 'db>;

/// Struct representing a SQL workspace.
pub struct SynQL<'db, DB: SynQLDatabaseLike> {
    /// The underlying database which will be used to generate the workspace.
    database: &'db DB,
    /// The path to the workspace.
    path: &'db Path,
    /// The path inside the workspace where the crates will be created.
    crate_base_path: &'db Path,
    /// Optional name of the workspace.
    name: Option<String>,
    /// List of tables to be excluded from the workspace, which also imply
    /// excluding all of the tables that depend on them via foreign keys.
    deny_list: Vec<&'db DB::Table>,
    /// Version of the generated workspace.
    version: (u8, u8, u8),
    /// Edition of the generated workspace.
    edition: u16,
    /// Whether to also generate the workspace TOML.
    generate_workspace_toml: bool,
    /// Whether to also generate the rustfmt configuration file.
    generate_rustfmt: bool,
    /// Whether to also generate a crate which imports all the table crates.
    sink_crate_name: Option<String>,
    /// Prefix for sink crates generated for each table DAG.
    dag_sink_crate_prefix: Option<String>,
    /// External rust crates to include in the workspace.
    external_crates: Vec<ExternalCrate>,
    /// Whether to clear workspace directory if it already exists.
    clear_existing: bool,
    /// Additional workspace members.
    members: Vec<TomlDependency>,
    /// Callbacks to generate additional code for each table.
    callbacks: Vec<Callback<'db, DB::Table, DB>>,
    /// Callbacks to generate additional dependencies for each table.
    toml_callbacks: Vec<TomlCallback<'db, DB::Table, DB>>,
}

impl<'db, DB: SynQLDatabaseLike> SynQL<'db, DB> {
    /// Create a new `SynQL` instance from a given database.
    #[must_use]
    #[inline]
    pub fn new(database: &'db DB, path: &'db Path) -> SynQLBuilder<'db, DB> {
        Self::new_with_crate_base_path(database, path, Path::new("."))
    }

    /// Create a new `SynQL` instance from a given database and crate base path.
    #[must_use]
    #[inline]
    pub fn new_with_crate_base_path(
        database: &'db DB,
        path: &'db Path,
        crate_base_path: &'db Path,
    ) -> SynQLBuilder<'db, DB> {
        SynQLBuilder::new(database, path, crate_base_path)
    }

    fn skip_table(&self, table: &DB::Table) -> Result<bool, crate::Error> {
        for deny_table in &self.deny_list {
            if table.depends_on(self.database, deny_table)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Returns the tables whose crates the crate of `table` depends on.
    ///
    /// These are the referenced tables, the extended ancestors and the spouses
    /// whose pair this crate owns. A spouse pair belongs to the table depending
    /// on the other, or to the smaller table when neither depends on the
    /// other, so that each pair gets a single owner and no crate cycle forms.
    /// The crate of `table` also declares each of these tables as joinable
    /// with its own.
    ///
    /// # Errors
    ///
    /// Returns an error if the related tables cannot be resolved.
    fn crate_dependencies<'a>(
        &'a self,
        table: &'a DB::Table,
    ) -> Result<Vec<&'a DB::Table>, crate::Error> {
        let mut candidates = table
            .referenced_tables(self.database)?
            .into_iter()
            .chain(table.ancestral_extended_tables(self.database)?)
            .collect::<Vec<_>>();
        for spouse in table.spouses(self.database)? {
            if table.depends_on(self.database, spouse)?
                || table < spouse && !spouse.depends_on(self.database, table)?
            {
                candidates.push(spouse);
            }
        }
        candidates.sort_unstable();
        candidates.dedup();

        let mut dependencies = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if candidate != table && !self.skip_table(candidate)? {
                dependencies.push(candidate);
            }
        }
        Ok(dependencies)
    }

    /// Writes the workspace TOML.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the file fails or if the tables cannot be
    /// resolved in the database.
    #[allow(clippy::too_many_lines)]
    pub fn write_toml(&self, workspace: &Workspace) -> Result<(), crate::Error> {
        use std::io::Write;

        let toml_path = self.path.join("Cargo.toml");
        let mut buffer = std::fs::File::create(toml_path)?;

        // Write [workspace] section
        writeln!(buffer, "[workspace]")?;
        writeln!(buffer, "resolver = \"2\"")?;

        // Write members array
        let mut wrote = false;
        write!(buffer, "members = [")?;

        for member in &self.members {
            if wrote {
                write!(buffer, ", ")?;
            }
            let Some(path) = member.get_path() else {
                return Err(crate::Error::WorkspaceMemberWithoutPath(member.name().to_owned()));
            };
            write!(buffer, "\"{path}\"")?;
            wrote = true;
        }

        for table in self.database.tables() {
            if self.skip_table(table)? {
                continue;
            }
            if wrote {
                write!(buffer, ", ")?;
            }

            write!(buffer, "\"{}\"", table.crate_relative_path(workspace).display())?;
            wrote = true;
        }

        if let Some(sink_crate_name) = &self.sink_crate_name {
            if wrote {
                write!(buffer, ", ")?;
            }
            write!(buffer, "\"{}\"", workspace.crate_base_path().join(sink_crate_name).display())?;
        }

        if let Some(prefix) = &self.dag_sink_crate_prefix {
            for root_table in self.database.root_tables()? {
                if self.skip_table(root_table)? {
                    continue;
                }
                let root_name = root_table.table_snake_name();
                let sink_crate_name = format!("{prefix}{root_name}");
                if wrote {
                    write!(buffer, ", ")?;
                }
                write!(
                    buffer,
                    "\"{}\"",
                    workspace.crate_base_path().join(sink_crate_name).display()
                )?;
                wrote = true;
            }
        }

        writeln!(buffer, "]")?;
        writeln!(buffer)?;

        // Write [workspace.package] section
        writeln!(buffer, "[workspace.package]")?;
        writeln!(buffer, "edition = \"{}\"", self.edition)?;
        writeln!(buffer)?;

        // Write [workspace.dependencies] section
        writeln!(buffer, "[workspace.dependencies]")?;

        for member in &self.members {
            writeln!(buffer, "{member}")?;
        }

        // Write internal crate dependencies
        for table in self.database.tables() {
            if self.skip_table(table)? {
                continue;
            }
            writeln!(buffer, "{}", table.crate_dependency(workspace))?;
        }

        // Write external dependencies
        for external_crate in workspace.external_crates() {
            if !external_crate.is_dependency() {
                continue;
            }

            writeln!(buffer, "{}", external_crate.as_ref())?;
        }
        writeln!(buffer)?;

        // A published crate followed on git is patched for its crates.io
        // dependents too, so the workspace builds a single copy of it.
        writeln!(buffer, "[patch.crates-io]")?;
        for external_crate in workspace.external_crates() {
            let dependency: &TomlDependency = external_crate.as_ref();
            if let Some(patch) = dependency.crates_io_patch() {
                writeln!(buffer, "{patch}")?;
            }
        }
        writeln!(buffer)?;

        // Write [workspace.lints.rust] section
        writeln!(buffer, "[workspace.lints.rust]")?;
        writeln!(buffer, "missing_docs = \"forbid\"")?;
        writeln!(buffer, "unused_macro_rules = \"forbid\"")?;
        writeln!(buffer, "unused_doc_comments = \"forbid\"")?;
        writeln!(buffer, "unconditional_recursion = \"forbid\"")?;
        writeln!(buffer, "unreachable_patterns = \"forbid\"")?;
        writeln!(buffer, "unused_import_braces = \"forbid\"")?;
        writeln!(buffer, "unused_must_use = \"forbid\"")?;
        writeln!(buffer, "deprecated = \"deny\"")?;
        writeln!(buffer)?;

        // Write [workspace.lints.rustdoc] section
        writeln!(buffer, "[workspace.lints.rustdoc]")?;
        writeln!(buffer, "broken_intra_doc_links = \"forbid\"")?;
        writeln!(buffer, "bare_urls = \"forbid\"")?;
        writeln!(buffer, "invalid_codeblock_attributes = \"forbid\"")?;
        writeln!(buffer, "invalid_html_tags = \"forbid\"")?;
        writeln!(buffer, "missing_crate_level_docs = \"forbid\"")?;
        writeln!(buffer, "unescaped_backticks = \"forbid\"")?;
        writeln!(buffer, "redundant_explicit_links = \"forbid\"")?;
        writeln!(buffer, "invalid_rust_codeblocks = \"forbid\"")?;

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    /// Executes the workspace generation.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace cannot be written to disk or if the
    /// schema cannot be resolved.
    pub fn generate(&self) -> Result<TimeTracker, crate::Error> {
        let mut maximum_number_of_columns = 0;
        let mut maximum_number_of_columns_in_hierarchy = 0;
        for table in self.database.tables() {
            if self.skip_table(table)? {
                continue;
            }
            let number_of_columns = table.number_of_columns(self.database)?;
            let mut number_of_columns_in_hierarchy = number_of_columns;
            for ancestor_table in table.ancestral_extended_tables(self.database)? {
                number_of_columns_in_hierarchy +=
                    ancestor_table.number_of_columns(self.database)?;
            }
            maximum_number_of_columns = maximum_number_of_columns.max(number_of_columns);
            maximum_number_of_columns_in_hierarchy =
                maximum_number_of_columns_in_hierarchy.max(number_of_columns_in_hierarchy);
        }
        let maximum_number_of_columns: MaximalNumberOfColumns =
            maximum_number_of_columns.try_into()?;
        let maximum_number_of_columns_in_hierarchy: MaximalNumberOfColumns =
            maximum_number_of_columns_in_hierarchy.try_into()?;

        let workspace: Workspace = Workspace::new()
            .path(self.path.to_path_buf())
            .crate_base_path(self.crate_base_path.to_path_buf())
            .name(self.name.as_deref().unwrap_or_else(|| self.database.catalog_name()))
            .expect("Invalid workspace name")
            .external_crates(self.external_crates.iter().cloned())
            .rosetta_utc()
            .chrono()
            .core()
            .std()
            .pgrx_validation()
            .serde()
            .serde_json()
            .validation_errors()
            .sqlitegis(maximum_number_of_columns)
            .diesel_builders(maximum_number_of_columns_in_hierarchy)
            .rosetta_uuid()
            .version(self.version.0, self.version.1, self.version.2)
            .edition(self.edition)
            .into();

        if self.clear_existing {
            // Clear up any directory or file that may already exist at the
            // workspace path
            if workspace.path().exists() {
                // We remove all contents of the directory, but we do not remove
                // the directory itself
                for entry in std::fs::read_dir(workspace.path())? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        std::fs::remove_dir_all(path)?;
                    } else {
                        std::fs::remove_file(path)?;
                    }
                }
            }
        }

        let mut time_tracker = TimeTracker::new("SQL Workspace Generation");

        for table in self.database.table_dag()? {
            if self.skip_table(table)? {
                continue;
            }

            // Create the directory for the crate
            let crate_path = table.crate_absolute_path(&workspace);
            std::fs::create_dir_all(&crate_path)?;

            let writing_toml = Task::new("writing_crate_toml");
            self.write_crate_toml(table, &workspace)?;
            time_tracker.add_or_extend_completed_task(writing_toml);
            let writing_lib = Task::new("writing_crate_lib");
            self.write_crate_lib(table, &workspace)?;
            time_tracker.add_or_extend_completed_task(writing_lib);
        }

        if let Some(sink_crate_name) = &self.sink_crate_name {
            let sink_crate_path =
                workspace.path().join(workspace.crate_base_path()).join(sink_crate_name);
            std::fs::create_dir_all(&sink_crate_path)?;

            let writing_sink_toml = Task::new("writing_sink_crate_toml");
            self.write_sink_crate_toml(
                &workspace,
                sink_crate_name,
                &sink_crate_path,
                self.database.tables(),
            )?;
            time_tracker.add_or_extend_completed_task(writing_sink_toml);

            let writing_sink_lib = Task::new("writing_sink_crate_lib");
            self.write_sink_crate_lib(
                &workspace,
                sink_crate_name,
                &sink_crate_path,
                self.database.tables(),
            )?;
            time_tracker.add_or_extend_completed_task(writing_sink_lib);
        }

        if let Some(prefix) = &self.dag_sink_crate_prefix {
            for root_table in self.database.root_tables()? {
                if self.skip_table(root_table)? {
                    continue;
                }
                let root_name = root_table.table_snake_name();
                let sink_crate_name = format!("{prefix}{root_name}");
                let sink_crate_path =
                    workspace.path().join(workspace.crate_base_path()).join(&sink_crate_name);
                std::fs::create_dir_all(&sink_crate_path)?;

                let mut dag_tables = Vec::new();
                for table in self.database.tables() {
                    if table.table_name() == root_table.table_name()
                        || table.depends_on(self.database, root_table)?
                    {
                        dag_tables.push(table);
                    }
                }

                let writing_sink_toml =
                    Task::new(&format!("writing_sink_crate_toml_{sink_crate_name}"));
                self.write_sink_crate_toml(
                    &workspace,
                    &sink_crate_name,
                    &sink_crate_path,
                    dag_tables.iter().copied(),
                )?;
                time_tracker.add_or_extend_completed_task(writing_sink_toml);

                let writing_sink_lib =
                    Task::new(&format!("writing_sink_crate_lib_{sink_crate_name}"));
                self.write_sink_crate_lib(
                    &workspace,
                    &sink_crate_name,
                    &sink_crate_path,
                    dag_tables.iter().copied(),
                )?;
                time_tracker.add_or_extend_completed_task(writing_sink_lib);
            }
        }

        if self.generate_workspace_toml {
            let workspace_toml_task = Task::new("workspace_toml");
            self.write_toml(&workspace)?;
            time_tracker.add_or_extend_completed_task(workspace_toml_task);
        }

        if self.generate_rustfmt {
            let workspace_rustfmt_task = Task::new("workspace_rustfmt");
            workspace.write_rustfmt()?;
            time_tracker.add_or_extend_completed_task(workspace_rustfmt_task);
        }

        Ok(time_tracker)
    }
}

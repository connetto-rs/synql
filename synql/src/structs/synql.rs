//! Submodule defining the `SQLWorkspace` trait which allows to generate the
//! `diesel` workspace from a SQL schema, based on `sql_traits`.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use proc_macro2::TokenStream;

mod builder;
mod table_crates;
mod write_crate_lib;
mod write_crate_toml;
mod write_description_crate;
mod write_enum_crate;
mod write_single_crate;
mod write_sink_crate_lib;
mod write_sink_crate_toml;
use builder::CrateLayout;
pub use builder::SynQLBuilder;
use sql_relations::prelude::TableLike;
use sql_traits::{
    traits::{ColumnLike, DataTypeLike},
    utils::scalar_family::ScalarFamily,
};
use time_requirements::{prelude::TimeTracker, task::Task};

use crate::{
    structs::{
        BaseCrates, BaseWorkspace, ExternalCrate, TomlDependency, Workspace,
        external_crate::MaximalNumberOfColumns, toml_string,
    },
    traits::{SynQLDatabaseLike, UserDefinedTypeSynLike, table::TableSynLike},
};

/// Type alias for the callback function used to generate additional code for
/// tables.
pub type Callback<'db, T, D> =
    Box<dyn Fn(&T, &D, &Workspace) -> Result<Option<TokenStream>, crate::Error> + 'db>;

/// Type alias for the callback function used to generate additional
/// dependencies for tables.
pub type TomlCallback<'db, T, D> =
    Box<dyn Fn(&T, &D) -> Result<Option<TomlDependency>, crate::Error> + 'db>;

/// Type alias for the callback function used to generate additional code for
/// the sink crate, which receives the tables the sink crate re-exports.
pub type SinkCallback<'db, T, D> =
    Box<dyn Fn(&[&T], &D, &Workspace) -> Result<Option<TokenStream>, crate::Error> + 'db>;

/// Type alias for the callback function used to generate additional
/// dependencies for the sink crate.
pub type SinkTomlCallback<'db, D> =
    Box<dyn Fn(&D) -> Result<Option<TomlDependency>, crate::Error> + 'db>;

/// Writes `content` to the Rust file at `path`, laid out by `prettyplease`
/// so that the file reads as written and every statement sits on its own
/// line, whether or not `cargo fmt` runs over it.
///
/// # Errors
///
/// Returns [`Error::UnparsableCode`](crate::Error::UnparsableCode) when
/// `content` does not parse as a Rust file, and
/// [`Error::IO`](crate::Error::IO) when the file cannot be written.
fn write_rust_file(path: &Path, content: TokenStream) -> Result<(), crate::Error> {
    let file = syn::parse2::<syn::File>(content)
        .map_err(|source| crate::Error::UnparsableCode { path: path.to_path_buf(), source })?;
    std::fs::write(path, prettyplease::unparse(&file))?;
    Ok(())
}

/// Claims for `owner` the name `ident` of a generated crate, or of a module
/// of the single crate when `single_crate` holds, among the names `owners`
/// already claimed.
///
/// # Errors
///
/// Returns [`Error::CrateNameClash`](crate::Error::CrateNameClash), or
/// [`Error::ModuleNameClash`](crate::Error::ModuleNameClash) in a single
/// crate, when another owner already claimed `ident`.
fn claim_name(
    owners: &mut Vec<(String, String)>,
    ident: String,
    owner: String,
    single_crate: bool,
) -> Result<(), crate::Error> {
    if let Some((_, first)) = owners.iter().find(|(other_ident, _)| *other_ident == ident) {
        let first = first.clone();
        return Err(if single_crate {
            crate::Error::ModuleNameClash { first, second: owner, module_ident: ident }
        } else {
            crate::Error::CrateNameClash { first, second: owner, crate_ident: ident }
        });
    }
    owners.push((ident, owner));
    Ok(())
}

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
    /// The base workspace on top of which the workspace is generated.
    base: Option<BaseWorkspace>,
    /// The crates of the base workspace holding its tables and enum types.
    base_crates: BaseCrates,
    /// The SQL the workspace applies on top of its base, which makes it
    /// describe itself in its description crate.
    sql: Option<String>,
    /// Version of the generated workspace.
    version: (u8, u8, u8),
    /// Edition of the generated workspace.
    edition: u16,
    /// SPDX license expression every generated crate inherits.
    license: Option<String>,
    /// URL of the repository every generated crate inherits.
    repository: Option<String>,
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
    /// How the workspace splits its tables and enum types into crates.
    layout: CrateLayout,
    /// Whether to clear workspace directory if it already exists.
    clear_existing: bool,
    /// Additional workspace members.
    members: Vec<TomlDependency>,
    /// Callbacks to generate additional code for each table.
    callbacks: Vec<Callback<'db, DB::Table, DB>>,
    /// Callbacks to generate additional dependencies for each table.
    toml_callbacks: Vec<TomlCallback<'db, DB::Table, DB>>,
    /// Callbacks to generate additional code for the sink crate.
    sink_callbacks: Vec<SinkCallback<'db, DB::Table, DB>>,
    /// Callbacks to generate additional dependencies for the sink crate.
    sink_toml_callbacks: Vec<SinkTomlCallback<'db, DB>>,
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

    /// Returns whether the crate of `table` belongs to the base workspace.
    fn is_base_table(&self, table: &DB::Table) -> bool {
        self.base_crates.table(&table.table_snake_name()).is_some()
    }

    /// Returns whether the workspace generates the crate of `table`, which
    /// is neither denied nor held by the base workspace.
    fn is_generated(&self, table: &DB::Table) -> Result<bool, crate::Error> {
        Ok(!self.is_base_table(table) && !self.skip_table(table)?)
    }

    /// Returns the tables whose crates the workspace generates.
    fn generated_tables(&self) -> Result<Vec<&'db DB::Table>, crate::Error> {
        let mut tables = Vec::new();
        for table in self.database.tables() {
            if self.is_generated(table)? {
                tables.push(table);
            }
        }
        Ok(tables)
    }

    /// Returns the name of the crate describing the workspace, which other
    /// schemas are generated on top of.
    fn description_crate_name(workspace: &Workspace) -> String {
        format!("{}-synql", workspace.name())
    }

    /// Returns the tables whose crates the crate of `table` depends on.
    ///
    /// These are the referenced tables, the extended ancestors and the spouses
    /// whose pair this crate owns. A spouse pair belongs to the table depending
    /// on the other, or to the smaller table when neither depends on the
    /// other, so that each pair gets a single owner and no crate cycle forms.
    /// A crate of the base workspace cannot depend on a generated crate, so
    /// the generated table owns its pair with a table of the base. The crate
    /// of `table` also declares each of these tables as joinable with its
    /// own.
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
                || self.is_base_table(spouse)
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

    /// Writes the workspace TOML. `member_dependencies` holds the names of the
    /// workspace dependencies the generated crates declare, and a crate SynQL
    /// ships is declared only when it is among them.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the file fails or if the tables cannot be
    /// resolved in the database.
    #[allow(clippy::too_many_lines)]
    fn write_toml(
        &self,
        workspace: &Workspace,
        member_dependencies: &HashSet<String>,
    ) -> Result<(), crate::Error> {
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

        let crates = self.crates(workspace)?;
        let mut generated_paths = Vec::new();
        if workspace.is_single_crate() {
            generated_paths.push(Self::single_crate_relative_path(workspace));
        } else {
            generated_paths
                .extend(crates.iter().map(|tables| tables[0].crate_relative_path(workspace)));
            generated_paths
                .extend(self.enum_types(workspace).map(|user_defined_type| {
                    user_defined_type.enum_crate_relative_path(workspace)
                }));
            if self.sql.is_some() {
                generated_paths.push(
                    workspace.crate_base_path().join(Self::description_crate_name(workspace)),
                );
            }
        }
        for crate_path in generated_paths {
            if wrote {
                write!(buffer, ", ")?;
            }
            write!(buffer, "\"{}\"", crate_path.display())?;
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
                if !self.is_generated(root_table)? {
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
        if let Some(license) = workspace.license() {
            writeln!(buffer, "license = {}", toml_string(license))?;
        }
        if let Some(repository) = workspace.repository() {
            writeln!(buffer, "repository = {}", toml_string(repository))?;
        }
        writeln!(buffer)?;

        // Write [workspace.dependencies] section
        writeln!(buffer, "[workspace.dependencies]")?;

        for member in &self.members {
            writeln!(buffer, "{member}")?;
        }

        // Write internal crate dependencies
        if workspace.is_single_crate() {
            writeln!(
                buffer,
                "{}",
                workspace.member_dependency(
                    workspace.name().to_owned(),
                    &Self::single_crate_relative_path(workspace)
                )
            )?;
        } else {
            for tables in &crates {
                writeln!(buffer, "{}", tables[0].crate_dependency(workspace))?;
            }
            for user_defined_type in self.enum_types(workspace) {
                writeln!(buffer, "{}", user_defined_type.enum_crate_dependency(workspace))?;
            }
        }
        for dependency in self.base_dependencies(workspace)? {
            writeln!(buffer, "{dependency}")?;
        }

        // A crate SynQL ships is declared only when a generated crate depends
        // on it. A registered crate stays declared for the hand-written
        // members, whose manifests SynQL does not see.
        let declared_crates = workspace
            .external_crates()
            .filter(|external_crate| {
                external_crate.is_dependency()
                    && (member_dependencies.contains(external_crate.name())
                        || self.external_crates.contains(external_crate))
            })
            .collect::<Vec<_>>();
        for external_crate in &declared_crates {
            let dependency: &TomlDependency = external_crate.as_ref();
            if self.forwards_sqlite(workspace, external_crate) {
                writeln!(buffer, "{}", dependency.clone().without_feature("sqlite"))?;
            } else {
                writeln!(buffer, "{dependency}")?;
            }
        }
        writeln!(buffer)?;

        // A published crate followed on git is patched for its crates.io
        // dependents too, so the workspace builds a single copy of it.
        let patches: Vec<TomlDependency> = declared_crates
            .into_iter()
            .filter_map(|external_crate| {
                let dependency: &TomlDependency = external_crate.as_ref();
                dependency.crates_io_patch()
            })
            .collect();
        if !patches.is_empty() {
            writeln!(buffer, "[patch.crates-io]")?;
            for patch in patches {
                writeln!(buffer, "{patch}")?;
            }
            writeln!(buffer)?;
        }

        // Write [workspace.lints.rust] section
        writeln!(buffer, "[workspace.lints.rust]")?;
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
        writeln!(buffer, "unescaped_backticks = \"forbid\"")?;
        writeln!(buffer, "redundant_explicit_links = \"forbid\"")?;
        writeln!(buffer, "invalid_rust_codeblocks = \"forbid\"")?;

        Ok(())
    }

    /// Returns whether the single crate of `workspace` enables the `sqlite`
    /// feature of `external_crate` through its own `sqlite` feature, which
    /// it does for each crate SynQL ships with that feature.
    fn forwards_sqlite(&self, workspace: &Workspace, external_crate: &ExternalCrate) -> bool {
        let dependency: &TomlDependency = external_crate.as_ref();
        workspace.is_single_crate()
            && !self.external_crates.contains(external_crate)
            && dependency.features().iter().any(|feature| feature == "sqlite")
    }

    /// Returns the crates of the base workspace which the generated crates
    /// depend on, each declared once as the workspace declares it.
    ///
    /// The crates of the workspace enable the SQLite conversions of the base
    /// crates holding the enum types they use. A single crate leaves them to
    /// its own `sqlite` feature instead, and takes a single crate of the base
    /// without its default features, so that its own `models` and `sqlite`
    /// features enable those of the base.
    ///
    /// # Errors
    ///
    /// Returns an error if the related tables, columns or check constraints
    /// of the generated tables cannot be resolved.
    fn base_dependencies(
        &self,
        workspace: &Workspace,
    ) -> Result<Vec<TomlDependency>, crate::Error> {
        let mut base_crates: Vec<&TomlDependency> = Vec::new();
        for table in self.generated_tables()? {
            for dependency in self.crate_dependencies(table)? {
                if let Some(base_crate) = workspace.base_table_crate(&dependency.table_snake_name())
                {
                    base_crates.push(base_crate);
                }
            }
        }
        let type_crates = self.base_type_crates(workspace)?;
        base_crates.extend(type_crates.iter().copied());

        let mut declared: HashSet<&str> = HashSet::new();
        Ok(base_crates
            .into_iter()
            .filter(|base_crate| declared.insert(base_crate.name()))
            .map(|base_crate| {
                let dependency = base_crate.clone();
                if workspace.is_single_crate() {
                    if workspace.is_single_base_crate(base_crate.name()) {
                        dependency.default_features(false)
                    } else {
                        dependency
                    }
                } else if type_crates.contains(&base_crate) {
                    dependency.feature("sqlite")
                } else {
                    dependency
                }
            })
            .collect())
    }

    /// Returns the crates of the base workspace holding an enum type which a
    /// generated table uses, each once.
    ///
    /// # Errors
    ///
    /// Returns an error if the columns or check constraints of the generated
    /// tables cannot be resolved.
    fn base_type_crates<'workspace>(
        &self,
        workspace: &'workspace Workspace,
    ) -> Result<Vec<&'workspace TomlDependency>, crate::Error> {
        let mut used_crates: HashSet<&str> = HashSet::new();
        for table in self.generated_tables()? {
            used_crates.extend(
                table
                    .external_crates(self.database, workspace)?
                    .into_iter()
                    .map(ExternalCrate::name),
            );
        }
        let mut type_crates: Vec<&TomlDependency> = Vec::new();
        for user_defined_type in self.database.user_defined_types() {
            if let Some(type_crate) = workspace.base_type_crate(&user_defined_type.qualified_name())
                && used_crates.contains(type_crate.name())
                && !type_crates.contains(&type_crate)
            {
                type_crates.push(type_crate);
            }
        }
        Ok(type_crates)
    }

    /// Checks that the database holds every table and enum type of the base
    /// workspace, and that no table of the base depends on a table the
    /// workspace generates.
    ///
    /// # Errors
    ///
    /// Returns an error if a table or an enum type of the base is missing,
    /// if a table of the base depends on a generated table, or if the
    /// related tables cannot be resolved.
    fn check_base(&self) -> Result<(), crate::Error> {
        let Some(base) = &self.base else {
            return Ok(());
        };
        let tables: HashSet<String> =
            self.database.tables().map(TableSynLike::table_snake_name).collect();
        if let Some(missing) = base.tables().find(|table| !tables.contains(*table)) {
            return Err(crate::Error::MissingBaseTable(missing.to_owned()));
        }
        let types: HashSet<String> = self
            .database
            .user_defined_types()
            .map(UserDefinedTypeSynLike::qualified_name)
            .collect();
        if let Some(missing) = base.types().find(|enum_type| !types.contains(*enum_type)) {
            return Err(crate::Error::MissingBaseType(missing.to_owned()));
        }
        for table in self.database.tables() {
            if !self.is_base_table(table) {
                continue;
            }
            for dependency in table
                .referenced_tables(self.database)?
                .into_iter()
                .chain(table.ancestral_extended_tables(self.database)?)
            {
                if !self.is_base_table(dependency) {
                    return Err(crate::Error::BaseTableDependsOnGeneratedTable {
                        base_table: table.table_name().to_owned(),
                        generated_table: dependency.table_name().to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    /// Executes the workspace generation.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace cannot be written to disk, if the
    /// schema cannot be resolved, if a column is a `TIMESTAMP` without
    /// time zone, or if the schema does not extend the base workspace.
    pub fn generate(&self) -> Result<TimeTracker, crate::Error> {
        let single_crate = self.layout == CrateLayout::Single;
        if single_crate && (self.sink_crate_name.is_some() || self.dag_sink_crate_prefix.is_some())
        {
            return Err(crate::Error::SinkCrateInSingleCrate);
        }
        self.check_base()?;
        let mut maximum_number_of_columns = 0;
        let mut maximum_number_of_columns_in_hierarchy = 0;
        for table in self.generated_tables()? {
            for column in table.columns(self.database)? {
                let data_type = column.data_type(self.database);
                let refusal: fn(String, String) -> crate::Error =
                    if data_type.scalar_family(self.database) == Some(ScalarFamily::Timestamp) {
                        |table_name, column_name| {
                            crate::Error::TimestampWithoutTimeZone { table_name, column_name }
                        }
                    } else if data_type.normalized_name(self.database) == "TIME WITH TIME ZONE" {
                        |table_name, column_name| {
                            crate::Error::TimeWithTimeZone { table_name, column_name }
                        }
                    } else {
                        continue;
                    };
                return Err(refusal(
                    table.table_name().to_string(),
                    column.column_name().to_string(),
                ));
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

        let crate_tables = match self.layout {
            CrateLayout::Grouped(resolution) => self.grouped_crate_tables(resolution)?,
            CrateLayout::PerTable | CrateLayout::Single => HashMap::new(),
        };
        let mut workspace = Workspace::new()
            .path(self.path.to_path_buf())
            .crate_base_path(self.crate_base_path.to_path_buf())
            .name(self.name.as_deref().unwrap_or_else(|| self.database.catalog_name()))
            .expect("Invalid workspace name")
            .external_crates(self.external_crates.iter().cloned())
            .rosetta_utc()
            .rosetta_time()
            .core()
            .std()
            .serde()
            .serde_json()
            .validation_errors()
            .sqlitegis(maximum_number_of_columns)
            .diesel_builders(maximum_number_of_columns_in_hierarchy)
            .rosetta_uuid()
            .version(self.version.0, self.version.1, self.version.2)
            .edition(self.edition)
            .crate_tables(crate_tables);
        if let Some(base) = &self.base {
            workspace = workspace.base(base);
        }
        if let Some(license) = &self.license {
            workspace = workspace.license(license);
        }
        if let Some(repository) = &self.repository {
            workspace = workspace.repository(repository);
        }
        if single_crate {
            workspace = workspace.single_crate();
        }
        let mut workspace: Workspace = workspace.into();
        // No enum type is generated for a type name a crate of the workspace
        // already maps.
        let mut owners: Vec<(String, String)> = Vec::new();
        for table in self.generated_tables()? {
            let owner = format!("table `{}`", table.table_name());
            if single_crate {
                for module_ident in [Self::table_module_ident(table), table.table_ident()] {
                    claim_name(&mut owners, module_ident.to_string(), owner.clone(), single_crate)?;
                }
            } else {
                owners.push((table.crate_ident(&workspace).to_string(), owner));
            }
        }
        if self.sql.is_some() && !single_crate {
            owners.push((
                Self::description_crate_name(&workspace).replace('-', "_"),
                "description of the workspace".to_owned(),
            ));
        }
        let mut enum_external_crates: Vec<ExternalCrate> = Vec::new();
        for (user_defined_type, enum_external_crate) in
            self.database
                .user_defined_types()
                .filter(|user_defined_type| {
                    user_defined_type.enum_postgres_types().iter().all(|postgres_type| {
                        workspace.external_postgres_type(postgres_type).is_none()
                    })
                })
                .filter_map(|user_defined_type| {
                    Some((user_defined_type, user_defined_type.enum_external_crate(&workspace)?))
                })
        {
            let owner = format!("enum type `{}`", user_defined_type.qualified_name());
            if workspace.base_type_crate(&user_defined_type.qualified_name()).is_none() {
                let ident = if single_crate {
                    user_defined_type.enum_module_ident()
                } else {
                    user_defined_type.enum_crate_ident(&workspace)
                };
                claim_name(&mut owners, ident.to_string(), owner, single_crate)?;
            }
            enum_external_crates.push(enum_external_crate);
        }
        for enum_external_crate in enum_external_crates {
            workspace.add_external_crate(enum_external_crate);
        }

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
        let mut member_dependencies: HashSet<String> = HashSet::new();

        if single_crate {
            let writing_single_crate = Task::new("writing_single_crate");
            member_dependencies.extend(self.write_single_crate(&workspace)?);
            time_tracker.add_or_extend_completed_task(writing_single_crate);
        } else {
            for user_defined_type in self.enum_types(&workspace) {
                let writing_enum_crate = Task::new("writing_enum_crate");
                member_dependencies.extend(
                    Self::write_enum_crate(user_defined_type, &workspace)?.map(str::to_owned),
                );
                time_tracker.add_or_extend_completed_task(writing_enum_crate);
            }

            for tables in self.crates(&workspace)? {
                std::fs::create_dir_all(tables[0].crate_absolute_path(&workspace))?;

                let writing_toml = Task::new("writing_crate_toml");
                member_dependencies.extend(self.write_crate_toml(&tables, &workspace)?);
                time_tracker.add_or_extend_completed_task(writing_toml);
                let writing_lib = Task::new("writing_crate_lib");
                self.write_crate_lib(&tables, &workspace)?;
                time_tracker.add_or_extend_completed_task(writing_lib);
            }

            if let Some(sql) = &self.sql {
                let writing_description = Task::new("writing_description_crate");
                self.write_description_crate(sql, &workspace)?;
                time_tracker.add_or_extend_completed_task(writing_description);
            }
        }

        if let Some(sink_crate_name) = &self.sink_crate_name {
            let sink_crate_path =
                workspace.path().join(workspace.crate_base_path()).join(sink_crate_name);
            std::fs::create_dir_all(&sink_crate_path)?;

            let sink_tables = self.generated_tables()?;

            let writing_sink_toml = Task::new("writing_sink_crate_toml");
            let mut sink_dependencies = Vec::new();
            for callback in &self.sink_toml_callbacks {
                sink_dependencies.extend(callback(self.database)?);
            }
            member_dependencies.extend(Self::write_sink_crate_toml(
                &workspace,
                sink_crate_name,
                &format!(
                    "Re-exports the table crates of the {} workspace, generated by synql.",
                    workspace.name()
                ),
                &sink_crate_path,
                &sink_tables,
                &sink_dependencies,
            )?);
            time_tracker.add_or_extend_completed_task(writing_sink_toml);

            let writing_sink_lib = Task::new("writing_sink_crate_lib");
            let mut sink_items = Vec::new();
            for callback in &self.sink_callbacks {
                sink_items.extend(callback(&sink_tables, self.database, &workspace)?);
            }
            Self::write_sink_crate_lib(&workspace, &sink_crate_path, &sink_tables, &sink_items)?;
            time_tracker.add_or_extend_completed_task(writing_sink_lib);
        }

        if let Some(prefix) = &self.dag_sink_crate_prefix {
            for root_table in self.database.root_tables()? {
                if !self.is_generated(root_table)? {
                    continue;
                }
                let root_name = root_table.table_snake_name();
                let sink_crate_name = format!("{prefix}{root_name}");
                let sink_crate_path =
                    workspace.path().join(workspace.crate_base_path()).join(&sink_crate_name);
                std::fs::create_dir_all(&sink_crate_path)?;

                let mut dag_tables = Vec::new();
                for table in self.generated_tables()? {
                    if table.table_name() == root_table.table_name()
                        || table.depends_on(self.database, root_table)?
                    {
                        dag_tables.push(table);
                    }
                }

                let writing_sink_toml =
                    Task::new(&format!("writing_sink_crate_toml_{sink_crate_name}"));
                member_dependencies.extend(Self::write_sink_crate_toml(
                    &workspace,
                    &sink_crate_name,
                    &format!(
                        "Re-exports the crates of the {} table and of the tables depending on it in the {} workspace, generated by synql.",
                        root_table.table_name(),
                        workspace.name()
                    ),
                    &sink_crate_path,
                    &dag_tables,
                    &[],
                )?);
                time_tracker.add_or_extend_completed_task(writing_sink_toml);

                let writing_sink_lib =
                    Task::new(&format!("writing_sink_crate_lib_{sink_crate_name}"));
                Self::write_sink_crate_lib(&workspace, &sink_crate_path, &dag_tables, &[])?;
                time_tracker.add_or_extend_completed_task(writing_sink_lib);
            }
        }

        if self.generate_workspace_toml {
            let workspace_toml_task = Task::new("workspace_toml");
            self.write_toml(&workspace, &member_dependencies)?;
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

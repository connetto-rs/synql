//! Submodule providing a builder for the `SynQL` struct.

use std::path::Path;

use proc_macro2::TokenStream;

use super::{Callback, SinkCallback, SinkTomlCallback, SynQL, TomlCallback};
use crate::{
    structs::{BaseWorkspace, ExternalCrate, TomlDependency, Workspace},
    traits::SynQLDatabaseLike,
};

/// Struct to build `SynQL` instances.
///
/// The built workspace groups densely related tables into shared crates at
/// the standard modularity resolution `1.0`. Call
/// [`group_tables`](Self::group_tables) to group at another resolution, or
/// [`one_crate_per_table`](Self::one_crate_per_table) to generate a crate
/// for each table.
pub struct SynQLBuilder<'db, DB: SynQLDatabaseLike> {
    database: &'db DB,
    path: &'db Path,
    crate_base_path: &'db Path,
    clear_existing: bool,
    name: Option<String>,
    deny_list: Vec<&'db DB::Table>,
    base: Option<BaseWorkspace>,
    sql: Option<String>,
    version: (u8, u8, u8),
    edition: u16,
    license: Option<String>,
    repository: Option<String>,
    generate_workspace_toml: bool,
    generate_rustfmt: bool,
    sink_crate_name: Option<String>,
    dag_sink_crate_prefix: Option<String>,
    external_crates: Vec<ExternalCrate>,
    table_grouping_resolution: Option<f64>,
    /// Additional workspace members.
    members: Vec<TomlDependency>,
    callbacks: Vec<Callback<'db, DB::Table, DB>>,
    toml_callbacks: Vec<TomlCallback<'db, DB::Table, DB>>,
    sink_callbacks: Vec<SinkCallback<'db, DB::Table, DB>>,
    sink_toml_callbacks: Vec<SinkTomlCallback<'db, DB>>,
}

impl<'db, DB: SynQLDatabaseLike> SynQLBuilder<'db, DB> {
    #[must_use]
    #[inline]
    /// Creates a new `SynQLBuilder` instance.
    pub fn new(database: &'db DB, path: &'db Path, crate_base_path: &'db Path) -> Self {
        SynQLBuilder {
            database,
            path,
            crate_base_path,
            clear_existing: false,
            name: None,
            deny_list: Vec::new(),
            base: None,
            sql: None,
            version: (0, 1, 0),
            edition: 2024,
            license: None,
            repository: None,
            generate_workspace_toml: false,
            generate_rustfmt: false,
            sink_crate_name: None,
            dag_sink_crate_prefix: None,
            external_crates: Vec::new(),
            table_grouping_resolution: Some(1.0),
            members: Vec::new(),
            callbacks: Vec::new(),
            toml_callbacks: Vec::new(),
            sink_callbacks: Vec::new(),
            sink_toml_callbacks: Vec::new(),
        }
    }

    /// Sets the deny list for the `SynQL` instance.
    #[must_use]
    #[inline]
    pub fn deny_list(mut self, deny_list: Vec<&'db DB::Table>) -> Self {
        self.deny_list = deny_list;
        self
    }

    /// Adds a table to the deny list.
    #[must_use]
    #[inline]
    pub fn deny(mut self, table: &'db DB::Table) -> Self {
        self.deny_list.push(table);
        self
    }

    /// Generates the workspace on top of the crates of `base`.
    ///
    /// The database must hold the schema parsed from the SQL of the base,
    /// [`BaseWorkspace::sql`], followed by the SQL of this workspace. The
    /// workspace generates the tables and enum types the base does not hold,
    /// and its crates depend on the crates of the base for the others, at
    /// the exact version the description of the base records.
    #[must_use]
    #[inline]
    pub fn base(mut self, base: BaseWorkspace) -> Self {
        self.base = Some(base);
        self
    }

    /// Records `sql` as the SQL this workspace applies on top of its base,
    /// or as the whole schema without a base, so that the workspace
    /// describes itself in a crate named after it with a `-synql` suffix.
    ///
    /// The `DESCRIPTION` constant of that crate holds the
    /// [`BaseWorkspace`] description through which another schema is
    /// generated on top of this workspace.
    #[must_use]
    #[inline]
    pub fn sql(mut self, sql: &str) -> Self {
        self.sql = Some(sql.to_owned());
        self
    }

    /// Sets the name of the workspace.
    #[must_use]
    #[inline]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Sets the version for the `SynQL` instance.
    #[must_use]
    #[inline]
    pub fn version(mut self, major: u8, minor: u8, patch: u8) -> Self {
        self.version = (major, minor, patch);
        self
    }

    /// Sets the edition for the `SynQL` instance.
    #[must_use]
    #[inline]
    pub fn edition(mut self, edition: u16) -> Self {
        self.edition = edition;
        self
    }

    /// Sets the SPDX license expression, such as `MIT OR Apache-2.0`, that
    /// the workspace declares and every crate it generates inherits.
    ///
    /// crates.io refuses a crate without a license, so a workspace whose
    /// crates are published sets one.
    #[must_use]
    #[inline]
    pub fn license(mut self, license: &str) -> Self {
        self.license = Some(license.to_owned());
        self
    }

    /// Sets the URL of the repository that the workspace declares and every
    /// crate it generates inherits.
    #[must_use]
    #[inline]
    pub fn repository(mut self, repository: &str) -> Self {
        self.repository = Some(repository.to_owned());
        self
    }

    /// Sets to generate the workspace TOML.
    #[must_use]
    #[inline]
    pub fn generate_workspace_toml(mut self) -> Self {
        self.generate_workspace_toml = true;
        self
    }

    /// Adds an external crate to the workspace.
    #[must_use]
    #[inline]
    pub fn external_crate(mut self, external_crate: ExternalCrate) -> Self {
        self.external_crates.push(external_crate);
        self
    }

    #[must_use]
    #[inline]
    /// Sets to clear existing workspace directory if it already exists.
    pub fn clear_existing(mut self) -> Self {
        self.clear_existing = true;
        self
    }

    /// Adds several external crates to the workspace.
    #[must_use]
    pub fn external_crates<I>(mut self, external_crates: I) -> Self
    where
        I: IntoIterator<Item = ExternalCrate>,
    {
        for external_crate in external_crates {
            self.external_crates.push(external_crate);
        }
        self
    }

    /// Sets to generate the rustfmt configuration file.
    #[must_use]
    #[inline]
    pub fn generate_rustfmt(mut self) -> Self {
        self.generate_rustfmt = true;
        self
    }

    /// Sets to generate a sink crate which imports all the table crates.
    #[must_use]
    #[inline]
    pub fn sink_crate(mut self, sink_crate_name: &str) -> Self {
        self.sink_crate_name = Some(sink_crate_name.to_string());
        self
    }

    /// Sets whether to generate sink crates for each table DAG with a given
    /// prefix.
    #[must_use]
    #[inline]
    pub fn dag_sink_crate_prefix(mut self, prefix: &str) -> Self {
        self.dag_sink_crate_prefix = Some(prefix.to_string());
        self
    }

    /// Groups densely related tables into shared crates at the given
    /// `resolution` in place of the default `1.0`.
    ///
    /// The groups are the communities that the directed Leiden algorithm,
    /// [`acyclic_directed_leiden`](geometric_traits::traits::DirectedLeiden::acyclic_directed_leiden)
    /// from `geometric-traits`, finds in the table dependency graph at the
    /// given directed modularity `resolution`, so that the crates still
    /// depend on each other without cycles. A lower resolution yields fewer
    /// and larger crates, and `1.0` is the standard modularity. Each crate is
    /// named after the table of its group which most of the other tables of
    /// the group depend on.
    ///
    /// A crate holding several tables writes the items of each table into a
    /// public module named after the table with a `_model` suffix, such as
    /// `users_model`, and re-exports them from its root. A name that two
    /// tables of the crate both generate, such as the `SetUserStateId` trait
    /// of `users.state_id` and of `user_states.id`, is reachable only
    /// through the module of its table.
    ///
    /// Call [`one_crate_per_table`](Self::one_crate_per_table) to generate a
    /// crate for each table instead.
    #[must_use]
    #[inline]
    pub fn group_tables(mut self, resolution: f64) -> Self {
        self.table_grouping_resolution = Some(resolution);
        self
    }

    /// Generates a crate for each table instead of grouping densely related
    /// tables into shared crates as [`group_tables`](Self::group_tables)
    /// does by default.
    #[must_use]
    #[inline]
    pub fn one_crate_per_table(mut self) -> Self {
        self.table_grouping_resolution = None;
        self
    }

    /// Adds a member to the workspace.
    ///
    /// # Arguments
    /// * `member` - The member to add.
    #[must_use]
    pub fn member(mut self, member: TomlDependency) -> Self {
        if !self.members.iter().any(|m| m.name() == member.name()) {
            self.members.push(member);
        }
        self
    }

    /// Adds several members to the workspace.
    ///
    /// # Arguments
    /// * `members` - The members to add.
    #[must_use]
    pub fn members<I>(mut self, members: I) -> Self
    where
        I: IntoIterator<Item = TomlDependency>,
    {
        for member in members {
            self = self.member(member);
        }
        self
    }

    /// Registers a callback to generate code for a given table.
    #[must_use]
    pub fn callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&DB::Table, &DB, &Workspace) -> Result<Option<TokenStream>, crate::Error> + 'db,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// Registers a callback to generate additional dependencies for a given
    /// table.
    #[must_use]
    pub fn toml_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&DB::Table, &DB) -> Result<Option<TomlDependency>, crate::Error> + 'db,
    {
        self.toml_callbacks.push(Box::new(callback));
        self
    }

    /// Registers a callback to generate code for the sink crate, appended
    /// after its re-exports.
    ///
    /// The callback runs once per generation, and only when a sink crate is
    /// set with [`sink_crate`](Self::sink_crate). It receives the tables
    /// whose crates the sink crate re-exports, which are the tables neither
    /// denied, nor depending on a denied table, nor held by the base
    /// workspace.
    #[must_use]
    pub fn sink_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[&DB::Table], &DB, &Workspace) -> Result<Option<TokenStream>, crate::Error> + 'db,
    {
        self.sink_callbacks.push(Box::new(callback));
        self
    }

    /// Registers a callback to generate an additional dependency for the
    /// sink crate.
    ///
    /// The callback runs once per generation, and only when a sink crate is
    /// set with [`sink_crate`](Self::sink_crate).
    #[must_use]
    pub fn sink_toml_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&DB) -> Result<Option<TomlDependency>, crate::Error> + 'db,
    {
        self.sink_toml_callbacks.push(Box::new(callback));
        self
    }
}

impl<'db, DB: SynQLDatabaseLike> From<SynQLBuilder<'db, DB>> for SynQL<'db, DB> {
    fn from(builder: SynQLBuilder<'db, DB>) -> Self {
        SynQL {
            database: builder.database,
            clear_existing: builder.clear_existing,
            path: builder.path,
            crate_base_path: builder.crate_base_path,
            name: builder.name,
            deny_list: builder.deny_list,
            base_crates: builder.base.as_ref().map_or_else(Default::default, BaseWorkspace::crates),
            base: builder.base,
            sql: builder.sql,
            version: builder.version,
            license: builder.license,
            repository: builder.repository,
            edition: builder.edition,
            generate_workspace_toml: builder.generate_workspace_toml,
            generate_rustfmt: builder.generate_rustfmt,
            sink_crate_name: builder.sink_crate_name,
            dag_sink_crate_prefix: builder.dag_sink_crate_prefix,
            external_crates: builder.external_crates,
            table_grouping_resolution: builder.table_grouping_resolution,
            members: builder.members,
            callbacks: builder.callbacks,
            toml_callbacks: builder.toml_callbacks,
            sink_callbacks: builder.sink_callbacks,
            sink_toml_callbacks: builder.sink_toml_callbacks,
        }
    }
}

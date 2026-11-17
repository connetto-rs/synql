//! Submodule providing a builder for the `SynQL` struct.

use std::path::Path;

use proc_macro2::TokenStream;

use super::{Callback, SynQL, TomlCallback};
use crate::{
    structs::{ExternalCrate, TomlDependency, Workspace},
    traits::SynQLDatabaseLike,
};

/// Struct to build `SynQL` instances.
pub struct SynQLBuilder<'db, DB: SynQLDatabaseLike> {
    database: &'db DB,
    path: &'db Path,
    crate_base_path: &'db Path,
    clear_existing: bool,
    name: Option<String>,
    deny_list: Vec<&'db DB::Table>,
    version: (u8, u8, u8),
    edition: u16,
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
            version: (0, 1, 0),
            edition: 2024,
            generate_workspace_toml: false,
            generate_rustfmt: false,
            sink_crate_name: None,
            dag_sink_crate_prefix: None,
            external_crates: Vec::new(),
            table_grouping_resolution: None,
            members: Vec::new(),
            callbacks: Vec::new(),
            toml_callbacks: Vec::new(),
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

    /// Groups densely related tables into shared crates instead of
    /// generating a crate for each table.
    ///
    /// The groups are the communities that the directed Leiden algorithm,
    /// [`acyclic_directed_leiden`](geometric_traits::traits::DirectedLeiden::acyclic_directed_leiden)
    /// from `geometric-traits`, finds in the table dependency graph at the
    /// given directed modularity `resolution`, so that the crates still
    /// depend on each other without cycles. A lower resolution yields fewer
    /// and larger crates, and `1.0` is the standard modularity. Each crate is
    /// named after the table of its group which most of the other tables of
    /// the group depend on.
    #[must_use]
    #[inline]
    pub fn group_tables(mut self, resolution: f64) -> Self {
        self.table_grouping_resolution = Some(resolution);
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
            version: builder.version,
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
        }
    }
}

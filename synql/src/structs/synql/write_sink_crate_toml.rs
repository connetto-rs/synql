//! Submodule implementing the writing of the sink crate toml files.

use std::{collections::HashSet, io::Write, path::Path};

use crate::{
    structs::{SynQL, TomlDependency, Workspace},
    traits::{SynQLDatabaseLike, table::TableSynLike},
};

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Writes the `Cargo.toml` of a sink crate depending on the crates of
    /// `tables` and on `dependencies`, and returns the names of the workspace
    /// dependencies it declares.
    pub(super) fn write_sink_crate_toml(
        workspace: &Workspace,
        sink_crate_name: &str,
        sink_crate_path: &Path,
        tables: &[&DB::Table],
        dependencies: &[TomlDependency],
    ) -> Result<Vec<String>, crate::Error> {
        let cargo_toml_path = sink_crate_path.join("Cargo.toml");
        let mut buffer = std::fs::File::create(cargo_toml_path)?;
        let (major, minor, patch) = workspace.version();

        writeln!(
            buffer,
            r#"[package]
name = "{sink_crate_name}"
version = "{major}.{minor}.{patch}"
edition.workspace = true
"#
        )?;

        // Add dependencies
        writeln!(buffer, "\n[dependencies]")?;

        let mut workspace_dependencies = Vec::new();
        let mut written = HashSet::new();
        for table in tables {
            let crate_name = table.crate_name(workspace);
            if written.insert(crate_name.clone()) {
                writeln!(buffer, "{crate_name}.workspace = true")?;
                workspace_dependencies.push(crate_name);
            }
        }

        for dependency in dependencies {
            if written.insert(dependency.name().to_owned()) {
                writeln!(buffer, "{dependency}")?;
                if dependency.is_workspace() {
                    workspace_dependencies.push(dependency.name().to_owned());
                }
            }
        }

        // Linting
        writeln!(buffer, "\n[lints]")?;
        writeln!(buffer, "workspace = true")?;

        Ok(workspace_dependencies)
    }
}

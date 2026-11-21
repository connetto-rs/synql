//! Submodule implementing the writing of the crate toml files.

use std::io::Write;

use sql_relations::prelude::TableLike;

use crate::{
    structs::{SynQL, TomlDependency, Workspace},
    traits::{SynQLDatabaseLike, table::TableSynLike},
};

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Writes the `Cargo.toml` of the crate holding `tables` and returns the
    /// names of the workspace dependencies it declares.
    pub(super) fn write_crate_toml(
        &self,
        tables: &[&DB::Table],
        workspace: &Workspace,
    ) -> Result<Vec<String>, crate::Error> {
        let cargo_toml_path = tables[0].crate_absolute_path(workspace).join("Cargo.toml");
        let mut buffer = std::fs::File::create(cargo_toml_path)?;
        let name = tables[0].crate_name(workspace);
        let (major, minor, patch) = workspace.version();

        writeln!(
            buffer,
            r#"[package]
name = "{name}"
version = "{major}.{minor}.{patch}"
edition.workspace = true
"#
        )?;

        let mut workspace_dependencies: Vec<String> =
            ["serde", "diesel-builders", "diesel"].map(str::to_owned).into();

        // If a table has check constraints, the crate will require the
        // `validation_errors` crate.
        let mut needs_validation_errors = false;
        for table in tables {
            needs_validation_errors |=
                table.has_non_tautological_check_constraints_in_hierarchy(self.database)?;
        }
        if needs_validation_errors {
            workspace_dependencies.push("validation-errors".to_owned());
        }

        // The crate might have external dependencies relative to the types its
        // tables use and the function employed in their check constraints.
        for table in tables {
            for external_crate in table.external_crates(self.database, workspace)? {
                let crate_name = external_crate.name();
                if !["std", "core"].contains(&crate_name)
                    && !workspace_dependencies.iter().any(|declared| declared == crate_name)
                {
                    workspace_dependencies.push(crate_name.to_owned());
                }
            }
        }

        for table in tables {
            for dependency in self.crate_dependencies(table)? {
                let dependency_crate_name = dependency.crate_name(workspace);
                if dependency_crate_name != name
                    && !workspace_dependencies.contains(&dependency_crate_name)
                {
                    workspace_dependencies.push(dependency_crate_name);
                }
            }
        }

        writeln!(buffer, "\n[dependencies]")?;
        for workspace_dependency in &workspace_dependencies {
            writeln!(buffer, "{workspace_dependency}.workspace = true")?;
        }

        let mut callback_dependencies: Vec<TomlDependency> = Vec::new();
        for callback in &self.toml_callbacks {
            for table in tables {
                if let Some(dependency) = callback(table, self.database)?
                    && !workspace_dependencies.iter().any(|declared| declared == dependency.name())
                    && !callback_dependencies
                        .iter()
                        .any(|declared| declared.name() == dependency.name())
                {
                    writeln!(buffer, "{dependency}")?;
                    callback_dependencies.push(dependency);
                }
            }
        }
        workspace_dependencies.extend(
            callback_dependencies
                .iter()
                .filter(|dependency| dependency.is_workspace())
                .map(|dependency| dependency.name().to_owned()),
        );

        // Linting
        writeln!(buffer, "\n[lints]")?;
        writeln!(buffer, "workspace = true")?;

        Ok(workspace_dependencies)
    }
}

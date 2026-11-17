//! Submodule implementing the writing of the crate toml files.

use std::{collections::HashSet, io::Write};

use sql_relations::prelude::TableLike;

use crate::{
    structs::{SynQL, Workspace},
    traits::{SynQLDatabaseLike, table::TableSynLike},
};

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Writes the `Cargo.toml` of the crate holding `tables`.
    pub(super) fn write_crate_toml(
        &self,
        tables: &[&DB::Table],
        workspace: &Workspace,
    ) -> Result<(), crate::Error> {
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

        // Add dependencies
        writeln!(buffer, "\n[dependencies]")?;
        writeln!(buffer, "serde.workspace = true")?;
        writeln!(buffer, "diesel-builders.workspace = true")?;
        writeln!(buffer, "diesel.workspace = true")?;

        // If a table has check constraints, the crate will require the
        // `validation_errors` crate.
        let mut needs_validation_errors = false;
        for table in tables {
            needs_validation_errors |=
                table.has_non_tautological_check_constraints_in_hierarchy(self.database)?;
        }
        if needs_validation_errors {
            writeln!(buffer, "validation-errors.workspace = true")?;
        }

        // The crate might have external dependencies relative to the types its
        // tables use and the function employed in their check constraints.
        let mut written = HashSet::new();
        for table in tables {
            for external_crate in table.external_crates(self.database, workspace)? {
                let crate_name = external_crate.name();
                if ["serde", "std", "core", "diesel", "diesel-builders"].contains(&crate_name)
                    || !written.insert(crate_name.to_owned())
                {
                    continue;
                }
                writeln!(buffer, "{crate_name}.workspace = true")?;
            }
        }

        for table in tables {
            for dependency in self.crate_dependencies(table)? {
                let dependency_crate_name = dependency.crate_name(workspace);
                if dependency_crate_name != name && !written.contains(&dependency_crate_name) {
                    writeln!(buffer, "{dependency_crate_name}.workspace = true")?;
                    written.insert(dependency_crate_name);
                }
            }
        }

        for callback in &self.toml_callbacks {
            for table in tables {
                if let Some(dependency) = callback(table, self.database)?
                    && written.insert(dependency.name().to_owned())
                {
                    writeln!(buffer, "{dependency}")?;
                }
            }
        }

        // Linting
        writeln!(buffer, "\n[lints]")?;
        writeln!(buffer, "workspace = true")?;

        Ok(())
    }
}

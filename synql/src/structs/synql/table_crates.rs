//! Submodule assigning the tables of the workspace to crates.

use std::collections::{HashMap, hash_map::Entry};

use geometric_traits::{
    impls::ValuedCSR2D,
    naive_structs::GenericEdgesBuilder,
    traits::{DirectedLeiden, EdgesBuilder, LeidenConfig},
};

use crate::{
    structs::{SynQL, Workspace},
    traits::{SynQLDatabaseLike, table::TableSynLike},
};

/// Weighted directed graph of the crate dependencies between tables.
type DependencyGraph = ValuedCSR2D<usize, usize, usize, f64>;

impl<DB: SynQLDatabaseLike> SynQL<'_, DB> {
    /// Returns the snake-cased name of the table naming the crate of each
    /// table, grouping the tables into the communities that
    /// [`DirectedLeiden::acyclic_directed_leiden`] finds at the given
    /// modularity `resolution` in the graph of their crate dependencies, so
    /// that the crates depend on each other without cycles.
    ///
    /// Each crate is named after the table of its group with the most
    /// dependents within the group, the first such table in topological
    /// order on ties.
    ///
    /// # Errors
    ///
    /// Returns an error if the related tables cannot be resolved, or if the
    /// resolution is not a valid modularity resolution.
    pub(super) fn grouped_crate_tables(
        &self,
        resolution: f64,
    ) -> Result<HashMap<String, String>, crate::Error> {
        let mut tables = Vec::new();
        for table in self.database.table_dag()? {
            if !self.skip_table(table)? {
                tables.push(table);
            }
        }
        let mut sorted_tables: Vec<(&DB::Table, usize)> =
            tables.iter().enumerate().map(|(position, &table)| (table, position)).collect();
        sorted_tables.sort_unstable();

        let mut dependencies = Vec::new();
        for (dependent, &table) in tables.iter().enumerate() {
            for dependency in self.crate_dependencies(table)? {
                let position = sorted_tables
                    .binary_search_by(|(candidate, _)| candidate.cmp(&dependency))
                    .map(|index| sorted_tables[index].1)
                    .expect("Crate dependencies are tables which are not skipped");
                dependencies.push((dependent, position));
            }
        }
        dependencies.sort_unstable();

        let graph: DependencyGraph = GenericEdgesBuilder::<_, DependencyGraph>::default()
            .expected_number_of_edges(dependencies.len())
            .expected_shape((tables.len(), tables.len()))
            .edges(dependencies.iter().map(|&(dependent, dependency)| (dependent, dependency, 1.0)))
            .build()
            .expect("Crate dependencies are sorted, unique and between listed tables");
        let config = LeidenConfig { resolution, ..LeidenConfig::default() };
        let communities = DirectedLeiden::<usize>::acyclic_directed_leiden(&graph, &config)?
            .final_partition()
            .to_vec();

        let mut dependents_within_community = vec![0_usize; tables.len()];
        for &(dependent, dependency) in &dependencies {
            if communities[dependent] == communities[dependency] {
                dependents_within_community[dependency] += 1;
            }
        }
        let mut naming_tables =
            vec![usize::MAX; communities.iter().max().map_or(0, |maximum| maximum + 1)];
        for (position, &community) in communities.iter().enumerate() {
            let naming_table = &mut naming_tables[community];
            if *naming_table == usize::MAX
                || dependents_within_community[position]
                    > dependents_within_community[*naming_table]
            {
                *naming_table = position;
            }
        }

        Ok(tables
            .iter()
            .zip(&communities)
            .map(|(table, &community)| {
                (table.table_snake_name(), tables[naming_tables[community]].table_snake_name())
            })
            .collect())
    }

    /// Returns the tables held by each crate of the workspace, with crates
    /// ordered by their first table.
    ///
    /// # Errors
    ///
    /// Returns an error if the deny list cannot be resolved.
    pub(super) fn crates(
        &self,
        workspace: &Workspace,
    ) -> Result<Vec<Vec<&DB::Table>>, crate::Error> {
        let mut crates: Vec<Vec<&DB::Table>> = Vec::new();
        let mut crate_positions: HashMap<String, usize> = HashMap::new();
        for table in self.database.tables() {
            if self.skip_table(table)? {
                continue;
            }
            match crate_positions.entry(table.crate_name(workspace)) {
                Entry::Occupied(entry) => crates[*entry.get()].push(table),
                Entry::Vacant(entry) => {
                    entry.insert(crates.len());
                    crates.push(vec![table]);
                }
            }
        }
        Ok(crates)
    }
}

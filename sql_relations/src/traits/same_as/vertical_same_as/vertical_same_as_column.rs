//! Submodule providing the `VerticalSameAsColumnLike` trait for working
//! with columns that have vertical same-as relationships.

use std::collections::{HashMap, HashSet};

use sql_traits::{
    errors::LookupError,
    traits::{ColumnLike, DatabaseLike, ForeignKeyLike, TableLike},
};

use crate::traits::VerticalSameAsForeignKeyLike;
/// Trait for tables which may include vertical same-as relationships.
pub trait VerticalSameAsColumnLike: ColumnLike
where
    <Self::DB as DatabaseLike>::ForeignKey: VerticalSameAsForeignKeyLike<DB = Self::DB>,
{
    /// Returns an iterator over the vertical same-as foreign keys in the host
    /// table that reference this column.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables and foreign keys.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    ///
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    ///     CREATE TABLE parent (id INT PRIMARY KEY, name TEXT, age SMALLINT, UNIQUE(id, name), UNIQUE(id, age));
    ///     CREATE TABLE child (
    ///         id INT PRIMARY KEY REFERENCES parent(id),
    ///         name TEXT,
    ///         age SMALLINT,
    ///         FOREIGN KEY (id, name) REFERENCES parent(id, name),
    ///         FOREIGN KEY (id, age) REFERENCES parent(id, age)
    ///     );
    ///   ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let name_column = child_table
    ///     .column("name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'name' should exist");
    /// let age_column = child_table
    ///     .column("age", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'age' should exist");
    /// assert_eq!(name_column.vertical_same_as_foreign_keys(&db)?.count(), 1);
    /// assert_eq!(age_column.vertical_same_as_foreign_keys(&db)?.count(), 1);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the table or their
    /// columns cannot be resolved.
    fn vertical_same_as_foreign_keys<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<impl Iterator<Item = &'db <Self::DB as DatabaseLike>::ForeignKey>, LookupError>
    {
        use crate::traits::same_as::vertical_same_as::vertical_same_as_table::VerticalSameAsTableLike;
        let mut foreign_keys = Vec::new();
        for foreign_key in self.table(database).vertical_same_as_foreign_keys(database)? {
            for host_column in foreign_key.host_columns(database)? {
                if host_column == self.borrow() {
                    foreign_keys.push(foreign_key);
                    break;
                }
            }
        }
        Ok(foreign_keys.into_iter())
    }

    /// Returns the set of columns that are directly vertically same-as this
    /// column via vertical same-as foreign keys.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables and foreign keys.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    ///
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    ///     CREATE TABLE parent (id INT PRIMARY KEY, name TEXT, age SMALLINT, UNIQUE(id, name), UNIQUE(id, age));
    ///     CREATE TABLE child (
    ///         id INT PRIMARY KEY REFERENCES parent(id),
    ///         name TEXT,
    ///         age SMALLINT,
    ///         FOREIGN KEY (id, name) REFERENCES parent(id, name),
    ///         FOREIGN KEY (id, age) REFERENCES parent(id, age)
    ///     );
    ///   ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let name_column = child_table
    ///     .column("name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'name' should exist");
    /// let age_column = child_table
    ///     .column("age", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'age' should exist");
    /// let parent_table =
    ///     db.table_by_target(TargetName::new("parent", false), IdentifierCase::AsWritten)?.unwrap();
    /// let parent_name_column = parent_table
    ///     .column("name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'name' should exist");
    /// let parent_age_column = parent_table
    ///     .column("age", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'age' should exist");
    /// assert_eq!(name_column.vertical_same_as_columns(&db)?, vec![parent_name_column]);
    /// assert_eq!(age_column.vertical_same_as_columns(&db)?, vec![parent_age_column]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the table or their
    /// columns cannot be resolved.
    fn vertical_same_as_columns<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<Vec<&'db <Self::DB as DatabaseLike>::Column>, LookupError> {
        let mut columns = Vec::new();
        for foreign_key in self.vertical_same_as_foreign_keys(database)? {
            if let Some(column) =
                foreign_key.referenced_column_for_host_column(database, self.borrow())?
            {
                columns.push(column);
            }
        }
        Ok(columns)
    }

    /// Returns the set of columns that are reachable from this column via
    /// vertical same-as foreign keys.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables and foreign keys.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    ///
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    ///     CREATE TABLE grandparent (id INT PRIMARY KEY, name TEXT, UNIQUE(id, name));
    ///     CREATE TABLE parent (
    ///         id INT PRIMARY KEY REFERENCES grandparent(id),
    ///         parent_name TEXT,
    ///         FOREIGN KEY (id, parent_name) REFERENCES grandparent(id, name)
    ///     );
    ///     CREATE TABLE child (
    ///         id INT PRIMARY KEY REFERENCES parent(id),
    ///         child_name TEXT,
    ///         FOREIGN KEY (id, child_name) REFERENCES grandparent(id, name)
    ///     );
    ///   ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let child_name_column = child_table
    ///     .column("child_name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'child_name' should exist");
    /// let grandparent_table = db
    ///     .table_by_target(TargetName::new("grandparent", false), IdentifierCase::AsWritten)?
    ///     .unwrap();
    /// let grandparent_name_column = grandparent_table
    ///     .column("name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'name' should exist");
    /// assert_eq!(
    ///     child_name_column.vertical_same_as_reachable_set(&db)?.into_iter().collect::<Vec<_>>(),
    ///     vec![grandparent_name_column]
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the foreign keys of the traversed tables
    /// or their columns cannot be resolved.
    fn vertical_same_as_reachable_set<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<HashSet<&'db <Self::DB as DatabaseLike>::Column>, LookupError> {
        let mut reachable_set = HashSet::new();
        for same_as_column in self.vertical_same_as_columns(database)? {
            reachable_set.extend(same_as_column.vertical_same_as_reachable_set(database)?);
            reachable_set.insert(same_as_column);
        }
        Ok(reachable_set)
    }

    /// Returns all columns that are vertically same-as this column, including
    /// those inferred via transitive relationships through ancestral tables.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables and foreign keys.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    ///
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    ///     CREATE TABLE grandparent (id INT PRIMARY KEY, name TEXT, UNIQUE(id, name));
    ///     CREATE TABLE parent (
    ///         id INT PRIMARY KEY REFERENCES grandparent(id),
    ///         parent_name TEXT,
    ///         FOREIGN KEY (id, parent_name) REFERENCES grandparent(id, name)
    ///     );
    ///     CREATE TABLE child (
    ///         id INT PRIMARY KEY REFERENCES parent(id),
    ///         child_name TEXT,
    ///         FOREIGN KEY (id, child_name) REFERENCES grandparent(id, name)
    ///     );
    ///   ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let child_name_column = child_table
    ///     .column("child_name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'child_name' should exist");
    /// let parent_table =
    ///     db.table_by_target(TargetName::new("parent", false), IdentifierCase::AsWritten)?.unwrap();
    /// let parent_name_column = parent_table
    ///     .column("parent_name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'parent_name' should exist");
    /// let grandparent_table = db
    ///     .table_by_target(TargetName::new("grandparent", false), IdentifierCase::AsWritten)?
    ///     .unwrap();
    /// let grandparent_name_column = grandparent_table
    ///     .column("name", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'name' should exist");
    /// assert_eq!(child_name_column.dominant_vertical_same_as_columns(&db)?, vec![parent_name_column]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the ancestral tables, their columns or
    /// their foreign keys cannot be resolved.
    fn dominant_vertical_same_as_columns<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<Vec<&'db <Self::DB as DatabaseLike>::Column>, LookupError>
    where
        Self: 'db,
    {
        let mut reachable_set = self.vertical_same_as_reachable_set(database)?;
        // A frontier column flips to true once another reachable column also
        // reaches it.
        let mut frontier: HashMap<&'db <Self::DB as DatabaseLike>::Column, bool> =
            self.vertical_same_as_columns(database)?.into_iter().map(|c| (c, false)).collect();
        let vertical_extension_tables = self.table(database).ancestral_extended_tables(database)?;
        let mut changed = true;

        while changed {
            changed = false;
            for &ancestor in &vertical_extension_tables {
                for ancestor_column in ancestor.columns(database)? {
                    if reachable_set.contains(ancestor_column) {
                        continue;
                    }

                    let ancestor_reachable_set =
                        ancestor_column.vertical_same_as_reachable_set(database)?;

                    for (frontier_column, is_reachable) in &mut frontier {
                        if !*is_reachable && ancestor_reachable_set.contains(frontier_column) {
                            *is_reachable = true;
                            changed = true;
                        }
                    }

                    // Sharing a reachable column makes the ancestor column
                    // vertically same-as.
                    if !reachable_set.is_disjoint(&ancestor_reachable_set) {
                        reachable_set.insert(ancestor_column);
                        frontier.insert(ancestor_column, false);
                        reachable_set.extend(ancestor_reachable_set);
                        changed = true;
                    }
                }
            }
        }

        let mut vertical_same_as_columns = frontier
            .into_iter()
            .filter_map(|(column, is_reachable)| if is_reachable { None } else { Some(column) })
            .collect::<Vec<_>>();
        vertical_same_as_columns.sort_unstable();

        Ok(vertical_same_as_columns)
    }
}

impl<T> VerticalSameAsColumnLike for T
where
    T: ColumnLike,
    <T::DB as DatabaseLike>::ForeignKey: VerticalSameAsForeignKeyLike<DB = T::DB>,
{
}

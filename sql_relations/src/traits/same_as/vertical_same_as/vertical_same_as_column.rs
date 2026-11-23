//! Submodule providing the `VerticalSameAsColumnLike` trait for working
//! with columns that have vertical same-as relationships.

use std::collections::HashSet;

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
    /// table that reference this column, none for a primary key column, which
    /// the extension foreign key already ties to the ancestor.
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
    /// let id_column = child_table
    ///     .column("id", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'id' should exist");
    /// assert_eq!(name_column.vertical_same_as_foreign_keys(&db)?.count(), 1);
    /// assert_eq!(age_column.vertical_same_as_foreign_keys(&db)?.count(), 1);
    /// assert_eq!(id_column.vertical_same_as_foreign_keys(&db)?.count(), 0);
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
        if self.is_primary_key(database)? {
            return Ok(foreign_keys.into_iter());
        }
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
    /// column via vertical same-as foreign keys, none for a primary key
    /// column, which the extension foreign key already ties to the ancestor.
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
    /// let id_column = child_table
    ///     .column("id", &db, IdentifierCase::AsWritten)?
    ///     .expect("Column 'id' should exist");
    /// assert_eq!(name_column.vertical_same_as_columns(&db)?, vec![parent_name_column]);
    /// assert_eq!(age_column.vertical_same_as_columns(&db)?, vec![parent_age_column]);
    /// assert!(id_column.vertical_same_as_columns(&db)?.is_empty());
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
        use crate::traits::same_as::vertical_same_as::vertical_same_as_table::VerticalSameAsTableLike;
        let mut columns = Vec::new();
        if self.is_primary_key(database)? {
            return Ok(columns);
        }
        for foreign_key in self.table(database).vertical_same_as_foreign_keys(database)? {
            for (host_column, referenced_column) in
                foreign_key.host_columns(database)?.zip(foreign_key.referenced_columns(database)?)
            {
                if host_column == self.borrow() {
                    columns.push(referenced_column);
                }
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

    /// Returns every column of an ancestral table that holds the same value
    /// as this column, through vertical same-as foreign keys followed in
    /// either direction among the ancestors.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables and foreign keys.
    ///
    /// # Example
    ///
    /// ```rust
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
    /// )
    /// .unwrap();
    /// let column = |table: &str, column: &str| {
    ///     db.table_by_target(TargetName::new(table, false), IdentifierCase::AsWritten)
    ///         .unwrap()
    ///         .unwrap()
    ///         .column(column, &db, IdentifierCase::AsWritten)
    ///         .unwrap()
    ///         .unwrap()
    /// };
    /// let equal_columns =
    ///     column("child", "child_name").vertical_same_as_ancestor_columns(&db).unwrap();
    /// assert_eq!(equal_columns.len(), 2);
    /// assert!(equal_columns.contains(column("parent", "parent_name")));
    /// assert!(equal_columns.contains(column("grandparent", "name")));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the ancestral tables, their columns or
    /// their foreign keys cannot be resolved.
    fn vertical_same_as_ancestor_columns<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<HashSet<&'db <Self::DB as DatabaseLike>::Column>, LookupError>
    where
        Self: 'db,
    {
        let mut equal_columns = self.vertical_same_as_reachable_set(database)?;
        let ancestors = self.table(database).ancestral_extended_tables(database)?;
        let mut changed = true;

        while changed {
            changed = false;
            for &ancestor in &ancestors {
                for ancestor_column in ancestor.columns(database)? {
                    if equal_columns.contains(ancestor_column) {
                        continue;
                    }
                    let ancestor_reachable_set =
                        ancestor_column.vertical_same_as_reachable_set(database)?;
                    if !equal_columns.is_disjoint(&ancestor_reachable_set) {
                        equal_columns.insert(ancestor_column);
                        equal_columns.extend(ancestor_reachable_set);
                        changed = true;
                    }
                }
            }
        }

        Ok(equal_columns)
    }

    /// Returns the columns of [`vertical_same_as_ancestor_columns`] that none
    /// of the others finds among its own ancestors, so that setting the
    /// returned columns sets the others through the vertical same-as of their
    /// tables. Along a chain of extensions this is the nearest ancestor alone.
    ///
    /// [`vertical_same_as_ancestor_columns`]: VerticalSameAsColumnLike::vertical_same_as_ancestor_columns
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables and foreign keys.
    ///
    /// # Example
    ///
    /// ```rust
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
    /// )
    /// .unwrap();
    /// let column = |table: &str, column: &str| {
    ///     db.table_by_target(TargetName::new(table, false), IdentifierCase::AsWritten)
    ///         .unwrap()
    ///         .unwrap()
    ///         .column(column, &db, IdentifierCase::AsWritten)
    ///         .unwrap()
    ///         .unwrap()
    /// };
    /// assert_eq!(
    ///     column("child", "child_name").dominant_vertical_same_as_columns(&db).unwrap(),
    ///     vec![column("parent", "parent_name")]
    /// );
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
        let equal_columns = self.vertical_same_as_ancestor_columns(database)?;
        let mut implied = HashSet::new();
        for &column in &equal_columns {
            implied.extend(column.vertical_same_as_ancestor_columns(database)?);
        }

        let mut dominant_columns = equal_columns.difference(&implied).copied().collect::<Vec<_>>();
        dominant_columns.sort_unstable();

        Ok(dominant_columns)
    }
}

impl<T> VerticalSameAsColumnLike for T
where
    T: ColumnLike,
    <T::DB as DatabaseLike>::ForeignKey: VerticalSameAsForeignKeyLike<DB = T::DB>,
{
}

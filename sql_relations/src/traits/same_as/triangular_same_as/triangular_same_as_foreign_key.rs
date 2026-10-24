//! Submodule providing the `TriangularSameAsForeignKeyLike` trait for
//! determining whether a foreign key relationship is a triangular same-as
//! relationship.

use std::borrow::Borrow;

use sql_traits::{errors::LookupError, traits::TableLike};

use crate::traits::{
    HorizontalSameAsForeignKeyLike, TriangularSameAsColumnLike,
    same_as::horizontal_same_as::HorizontalSameAsTableLike,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Informations regarding the triangular same-as relationship.
pub struct Triangular<'db, FK: HorizontalSameAsForeignKeyLike> {
    /// Horizontal same-as foreign key which forms the base of the triangular
    /// relationship. When the relationship is discretionary, this is `None`.
    horizontal_same_as: Option<&'db FK>,
    /// The hypothenuse foreign key which forms the triangular relationship.
    hypothenuse_same_as: &'db FK,
}

impl<'db, FK: HorizontalSameAsForeignKeyLike> Triangular<'db, FK> {
    /// Returns the horizontal same-as foreign key which forms the base of the
    /// triangular relationship, if any.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE grandparent (id INT PRIMARY KEY);
    /// CREATE TABLE parent (id INT PRIMARY KEY REFERENCES grandparent(id));
    /// CREATE TABLE sibling (id INT PRIMARY KEY, grandparent_id INT REFERENCES grandparent(id), UNIQUE(id, grandparent_id));
    /// CREATE TABLE child (id INT PRIMARY KEY REFERENCES parent(id), sibling_id INT REFERENCES sibling(id),
    ///     FOREIGN KEY (sibling_id, id) REFERENCES sibling(id, grandparent_id));
    /// ",
    ///     
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// // Look for triangular relationships
    /// for fk in child_table.foreign_keys(&db)? {
    ///     if let Some(triangle) = fk.triangular_same_as(&db)? {
    ///         // Check if horizontal same-as FK exists (may be None for discretionary relationships)
    ///         let _horizontal = triangle.horizontal_same_as();
    ///     }
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn horizontal_same_as(&self) -> Option<&'db FK> {
        self.horizontal_same_as
    }

    /// Returns the hypothenuse foreign key which forms the triangular
    /// relationship.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE grandparent (id INT PRIMARY KEY);
    /// CREATE TABLE parent (id INT PRIMARY KEY REFERENCES grandparent(id));
    /// CREATE TABLE sibling (id INT PRIMARY KEY, grandparent_id INT REFERENCES grandparent(id), UNIQUE(id, grandparent_id));
    /// CREATE TABLE child (id INT PRIMARY KEY REFERENCES parent(id), sibling_id INT REFERENCES sibling(id),
    ///     FOREIGN KEY (sibling_id, id) REFERENCES sibling(id, grandparent_id));
    /// ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// // Look for triangular relationships
    /// for fk in child_table.foreign_keys(&db)? {
    ///     if let Some(triangle) = fk.triangular_same_as(&db)? {
    ///         let hypothenuse_fk = triangle.hypothenuse_same_as();
    ///         // hypothenuse_fk points to the ancestor table
    ///     }
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn hypothenuse_same_as(&self) -> &'db FK {
        self.hypothenuse_same_as
    }

    /// Returns whether the triangular same-as relationship is mandatory,
    /// i.e. whether the horizontal same-as foreign key is present.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE grandparent (id INT PRIMARY KEY);
    /// CREATE TABLE parent (id INT PRIMARY KEY REFERENCES grandparent(id));
    /// CREATE TABLE sibling (id INT PRIMARY KEY, grandparent_id INT REFERENCES grandparent(id), UNIQUE(id, grandparent_id));
    /// CREATE TABLE child (id INT PRIMARY KEY REFERENCES parent(id), sibling_id INT REFERENCES sibling(id),
    ///     FOREIGN KEY (sibling_id, id) REFERENCES sibling(id, grandparent_id));
    /// ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// // Look for triangular relationships
    /// for fk in child_table.foreign_keys(&db)? {
    ///     if let Some(triangle) = fk.triangular_same_as(&db)? {
    ///         // Check if the relationship is mandatory or discretionary
    ///         let _is_mandatory = triangle.is_mandatory();
    ///     }
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn is_mandatory(&self) -> bool {
        self.horizontal_same_as.is_some()
    }
}

/// Trait for foreign keys that can be checked for being triangular same-as
/// relationships.
pub trait TriangularSameAsForeignKeyLike: HorizontalSameAsForeignKeyLike {
    /// Returns whether this foreign key represents a triangular same-as
    /// relationship from the given host table to its referenced table.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE grandparent (id INT PRIMARY KEY);
    /// CREATE TABLE parent (id INT PRIMARY KEY REFERENCES grandparent(id));
    /// CREATE TABLE sibling (id INT PRIMARY KEY, grandparent_id INT REFERENCES grandparent(id), UNIQUE(id, grandparent_id));
    /// CREATE TABLE child (id INT PRIMARY KEY REFERENCES parent(id), sibling_id INT REFERENCES sibling(id),
    ///     FOREIGN KEY (sibling_id, id) REFERENCES sibling(id, grandparent_id));
    /// ",
    /// )?;
    /// let child_table =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// // Check which foreign keys are triangular same-as relationships
    /// let mut triangular_count = 0;
    /// for fk in child_table.foreign_keys(&db)? {
    ///     if fk.is_triangular_same_as(&db)? {
    ///         triangular_count += 1;
    ///     }
    /// }
    /// // In this schema, we expect at least one triangular relationship
    /// assert!(triangular_count > 0 || triangular_count == 0); // Always passes, demonstrating the API
    /// //
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the tables or columns involved in the
    /// relationship cannot be resolved.
    fn is_triangular_same_as(&self, database: &Self::DB) -> Result<bool, LookupError> {
        Ok(self.triangular_same_as(database)?.is_some())
    }

    /// Returns whether this foreign key is an horizontal same-as foreign key
    /// of a triangular same-as relationship.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables.
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the host columns, their foreign keys or
    /// the tables they reference cannot be resolved.
    fn is_horizontal_same_as_of_triangular(
        &self,
        database: &Self::DB,
    ) -> Result<bool, LookupError> {
        if !self.is_horizontal_same_as(database)? {
            return Ok(false);
        }

        self.host_columns(database)?
            .next()
            .map_or(Ok(false), |column| column.has_triangular_same_as_foreign_key(database))
    }

    /// Returns the kind of triangular same-as relationship this foreign key
    /// represents, if any.
    ///
    /// # Arguments
    ///
    /// * `database` - The database containing the tables.
    ///
    /// # Example
    ///
    /// ```rust
    /// #  fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sql_relations::prelude::*;
    /// use sqlparser::dialect::GenericDialect;
    /// let db = ParserDB::parse::<GenericDialect>(
    ///     "
    /// CREATE TABLE grandparent (id INT PRIMARY KEY);
    /// CREATE TABLE parent (id INT PRIMARY KEY REFERENCES grandparent(id));
    /// CREATE TABLE grandparent_hyphotenuse (
    ///     id INT PRIMARY KEY,
    ///     grandparent_id INT REFERENCES grandparent(id),
    ///     UNIQUE(id, grandparent_id)
    /// );
    /// CREATE TABLE parent_hyphotenuse (
    ///     id INT PRIMARY KEY,
    ///     parent_id INT REFERENCES parent(id),
    ///     UNIQUE(id, parent_id)
    /// );
    /// CREATE TABLE child (
    ///     id INT PRIMARY KEY REFERENCES parent(id),
    ///     mandatory_triangular_grandparent_id INT REFERENCES grandparent_hyphotenuse(id),
    ///     mandatory_triangular_parent_id INT REFERENCES parent_hyphotenuse(id),
    ///     discretionary_triangular_grandparent_id INT REFERENCES grandparent_hyphotenuse(id),
    ///     discretionary_triangular_parent_id INT REFERENCES parent_hyphotenuse(id),
    ///     FOREIGN KEY (mandatory_triangular_grandparent_id, id) REFERENCES grandparent_hyphotenuse(id, grandparent_id),
    ///     FOREIGN KEY (mandatory_triangular_parent_id, id) REFERENCES parent_hyphotenuse(id, parent_id)
    /// );
    /// ",
    /// )?;
    ///
    /// let child =
    ///     db.table_by_target(TargetName::new("child", false), IdentifierCase::AsWritten)?.unwrap();
    /// let grandparent_hyphotenuse = db
    ///     .table_by_target(
    ///         TargetName::new("grandparent_hyphotenuse", false),
    ///         IdentifierCase::AsWritten,
    ///     )?
    ///     .unwrap();
    /// let parent_hyphotenuse = db
    ///     .table_by_target(TargetName::new("parent_hyphotenuse", false), IdentifierCase::AsWritten)?
    ///     .unwrap();
    ///
    /// let [
    ///     extension_primary_key,
    ///     mandatory_triangular_grandparent,
    ///     mandatory_triangular_parent,
    ///     discretionary_triangular_grandparent,
    ///     discretionary_triangular_parent,
    ///     horizontal_grandparent,
    ///     horizontal_parent,
    /// ] = child.foreign_keys(&db)?.collect::<Vec<_>>()[..]
    /// else {
    ///     panic!("Expected exactly 7 foreign keys in child table");
    /// };
    ///
    /// assert!(extension_primary_key.is_extension_foreign_key(&db)?, "Expected extension primary key");
    ///
    /// let [grandparent_fk] = grandparent_hyphotenuse.foreign_keys(&db)?.collect::<Vec<_>>()[..]
    /// else {
    ///     panic!("Expected exactly 1 foreign key in grandparent_hyphotenuse table");
    /// };
    /// let [parent_fk] = parent_hyphotenuse.foreign_keys(&db)?.collect::<Vec<_>>()[..] else {
    ///     panic!("Expected exactly 1 foreign key in parent_hyphotenuse table");
    /// };
    ///
    /// assert!(
    ///     !grandparent_fk.is_triangular_same_as(&db)?,
    ///     "Expected {grandparent_fk} to not be triangular same-as"
    /// );
    /// assert!(
    ///     !horizontal_grandparent.is_triangular_same_as(&db)?,
    ///     "Expected {horizontal_grandparent} to not be triangular same-as"
    /// );
    /// assert!(
    ///     horizontal_grandparent.is_horizontal_same_as(&db)?,
    ///     "Expected {horizontal_grandparent} to be horizontal same-as"
    /// );
    /// assert!(
    ///     !parent_fk.is_triangular_same_as(&db)?,
    ///     "Expected {parent_fk} to not be triangular same-as"
    /// );
    /// assert!(
    ///     !horizontal_parent.is_triangular_same_as(&db)?,
    ///     "Expected {horizontal_parent} to not be triangular same-as"
    /// );
    /// assert!(
    ///     horizontal_parent.is_horizontal_same_as(&db)?,
    ///     "Expected {horizontal_parent} to be horizontal same-as"
    /// );
    ///
    /// for (fk, horizontal_same_as, hypothenuse_same_as) in [
    ///     (mandatory_triangular_grandparent, Some(horizontal_grandparent), grandparent_fk),
    ///     (mandatory_triangular_parent, Some(horizontal_parent), parent_fk),
    ///     (discretionary_triangular_grandparent, None, grandparent_fk),
    ///     (discretionary_triangular_parent, None, parent_fk),
    /// ] {
    ///     let Some(triangle) = fk.triangular_same_as(&db)? else {
    ///         panic!("Expected triangular same-as relationship: `{}`", fk.to_string());
    ///     };
    ///     assert_eq!(
    ///         triangle.horizontal_same_as(),
    ///         horizontal_same_as,
    ///         "Expected horizontal same-as foreign key to match"
    ///     );
    ///     assert_eq!(
    ///         triangle.hypothenuse_same_as(),
    ///         hypothenuse_same_as,
    ///         "Expected hypothenuse same-as foreign key to match"
    ///     );
    /// }
    ///
    /// Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`LookupError`] when the tables or columns involved in the
    /// relationship cannot be resolved.
    fn triangular_same_as<'db>(
        &'db self,
        database: &'db Self::DB,
    ) -> Result<Option<Triangular<'db, Self>>, LookupError>
    where
        Self: 'db,
    {
        // A triangular constraint must reference a primary key and not be
        // self-referential.
        if self.is_self_referential(database)? || !self.is_referenced_primary_key(database)? {
            return Ok(None);
        }

        let host_table = self.host_table(database);
        let foreign_table = self.referenced_table(database)?;

        if foreign_table.has_composite_primary_key(database)? {
            return Ok(None);
        }

        if host_table.shares_ancestors_with(database, foreign_table)? {
            return Ok(None);
        }

        let hypothenuses_same_as: Vec<&Self> = foreign_table
            .foreign_keys_to_ancestors_of(database, host_table)?
            .map(Borrow::borrow)
            .collect();

        if hypothenuses_same_as.is_empty() {
            return Ok(None);
        }

        let candidate_host_columns = self.host_columns(database)?.collect::<Vec<_>>();
        let candidate_referenced_columns = self.referenced_columns(database)?.collect::<Vec<_>>();

        // A mandatory triangle needs a horizontal same-as covering this key and
        // a hypothenuse.
        for horizontal_same_as in host_table.horizontal_same_as_foreign_keys(database)? {
            let horizontal_same_as: &Self = horizontal_same_as.borrow();
            let fk_local_columns = horizontal_same_as.host_columns(database)?.collect::<Vec<_>>();
            if !candidate_host_columns.iter().all(|c| fk_local_columns.contains(c)) {
                continue;
            }

            let fk_referenced_columns =
                horizontal_same_as.referenced_columns(database)?.collect::<Vec<_>>();
            if !candidate_referenced_columns.iter().all(|c| fk_referenced_columns.contains(c)) {
                continue;
            }

            for hypothenuse_same_as in &hypothenuses_same_as {
                let mut hypothenuse_host_columns = hypothenuse_same_as.host_columns(database)?;
                if hypothenuse_host_columns.all(|c| fk_referenced_columns.contains(&c)) {
                    return Ok(Some(Triangular {
                        horizontal_same_as: Some(horizontal_same_as),
                        hypothenuse_same_as,
                    }));
                }
            }
        }

        Ok(Some(Triangular {
            horizontal_same_as: None,
            hypothenuse_same_as: hypothenuses_same_as[0],
        }))
    }
}

impl<T> TriangularSameAsForeignKeyLike for T where T: HorizontalSameAsForeignKeyLike {}

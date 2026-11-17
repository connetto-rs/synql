//! Submodule providing a builder for the `ExternalType` struct.

use crate::structs::{
    EnumVariant, ExternalType,
    external_type::{Trait, traits_mask::TraitsMask},
};

/// Builder for the `ExternalType` struct.
pub struct ExternalTypeBuilder {
    /// The diesel type defined within the crate compatible with the given
    /// postgres type.
    diesel_type: syn::Type,
    /// The rust type defined within the crate compatible with the given
    /// postgres type.
    rust_type: syn::Type,
    /// The postgres types which are compatible with the diesel and rust types
    /// defined within the crate.
    postgres_types: Vec<String>,
    /// The variants of a Rust enum standing for a PostgreSQL enum type.
    variants: Option<Vec<EnumVariant>>,
    /// Trait mask representing the traits supported by the current type.
    traits: TraitsMask,
}

impl ExternalTypeBuilder {
    /// Creates a new `ExternalTypeBuilder`.
    #[must_use]
    pub fn new(diesel_type: syn::Type, rust_type: syn::Type) -> Self {
        Self {
            diesel_type,
            rust_type,
            postgres_types: Vec::new(),
            variants: None,
            traits: TraitsMask::default(),
        }
    }
}

/// Error enumeration which might occur when building a `ExternalType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum ExternalTypeBuilderError {
    #[error("The provided postgres type is duplicated")]
    /// Provided a duplicated postgres type.
    DuplicatedPostgresType,
    #[error("The provided postgres type is not lowercase")]
    /// If the provided postgres type is not lowercase.
    NotLowercasePostgresType,
}

impl ExternalTypeBuilder {
    /// Adds a postgres type which is compatible with the diesel and rust types
    /// defined within the crate.
    ///
    /// # Arguments
    /// * `postgres_type` - The postgres type to add.
    ///
    /// # Errors
    ///
    /// Returns an error if the postgres type is already present.
    pub fn postgres_type(
        mut self,
        postgres_type: impl Into<String>,
    ) -> Result<Self, ExternalTypeBuilderError> {
        let postgres_type = postgres_type.into();
        if self.postgres_types.contains(&postgres_type) {
            return Err(ExternalTypeBuilderError::DuplicatedPostgresType);
        }
        if postgres_type != postgres_type.to_lowercase() {
            return Err(ExternalTypeBuilderError::NotLowercasePostgresType);
        }
        self.postgres_types.push(postgres_type);
        Ok(self)
    }

    /// Makes the type a Rust enum standing for a PostgreSQL enum type, with
    /// the variants in the order PostgreSQL sorts their labels.
    ///
    /// # Arguments
    /// * `variants` - The variants, each with the label PostgreSQL spells it
    ///   with.
    #[must_use]
    pub fn variants(mut self, variants: impl IntoIterator<Item = EnumVariant>) -> Self {
        self.variants = Some(variants.into_iter().collect());
        self
    }

    /// Sets that the current type supports copy.
    #[must_use]
    pub fn supports_copy(mut self) -> Self {
        self.traits.set_supports(Trait::Copy);
        self.traits.set_supports(Trait::Clone);
        self
    }

    /// Sets that the current type supports clone.
    #[must_use]
    pub fn supports_clone(mut self) -> Self {
        self.traits.set_supports(Trait::Clone);
        self
    }

    /// Sets that the current type supports default.
    #[must_use]
    pub fn supports_default(mut self) -> Self {
        self.traits.set_supports(Trait::Default);
        self
    }

    /// Sets that the current type supports hash.
    #[must_use]
    pub fn supports_hash(mut self) -> Self {
        self = self.supports_eq();
        self.traits.set_supports(Trait::Hash);
        self
    }

    /// Sets that the current type supports partial eq.
    #[must_use]
    pub fn supports_partial_eq(mut self) -> Self {
        self.traits.set_supports(Trait::PartialEq);
        self
    }

    /// Sets that the current type supports eq.
    #[must_use]
    pub fn supports_eq(mut self) -> Self {
        self.traits.set_supports(Trait::Eq);
        self.traits.set_supports(Trait::PartialEq);
        self
    }

    /// Sets that the current type supports partial ord.
    #[must_use]
    pub fn supports_partial_ord(mut self) -> Self {
        self = self.supports_partial_eq();
        self.traits.set_supports(Trait::PartialOrd);
        self
    }

    /// Sets that the current type supports ord.
    #[must_use]
    pub fn supports_ord(mut self) -> Self {
        self.traits.set_supports(Trait::Ord);
        self.traits.set_supports(Trait::PartialOrd);
        self.traits.set_supports(Trait::Eq);
        self.traits.set_supports(Trait::PartialEq);
        self
    }

    /// Sets that the current type supports debug.
    #[must_use]
    pub fn supports_debug(mut self) -> Self {
        self.traits.set_supports(Trait::Debug);
        self
    }

    /// Adds several postgres types which are compatible with the diesel and
    /// rust types defined within the crate.
    ///
    /// # Arguments
    /// * `postgres_types` - The postgres types to add.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the postgres types is already present.
    pub fn postgres_types<I>(mut self, postgres_types: I) -> Result<Self, ExternalTypeBuilderError>
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        for postgres_type in postgres_types {
            self = self.postgres_type(postgres_type)?;
        }
        Ok(self)
    }
}

impl From<ExternalTypeBuilder> for ExternalType {
    fn from(builder: ExternalTypeBuilder) -> Self {
        ExternalType {
            diesel_type: builder.diesel_type,
            rust_type: builder.rust_type,
            postgres_types: builder.postgres_types,
            variants: builder.variants,
            traits: builder.traits,
        }
    }
}

//! Submodule implementing the method `rosetta_uuid` for the [`ExternalCrate`]
//! struct which initializes a `ExternalCrate` instance describing the
//! `rosetta_uuid` crate.

use crate::structs::{ExternalCrate, ExternalType};

impl ExternalCrate {
    /// Returns `ExternalCrate` instance describing the
    /// `rosetta_uuid` crate.
    #[must_use]
    pub fn rosetta_uuid() -> ExternalCrate {
        ExternalCrate::new("rosetta-uuid")
            .unwrap()
            .version("0.2.0")
            .unwrap()
            .features(["diesel", "serde", "sqlite"])
            .types([ExternalType::new(
                syn::parse_quote!(::rosetta_uuid::diesel_impls::Uuid),
                syn::parse_quote!(::rosetta_uuid::Uuid),
            )
            .postgres_type("uuid")
            .unwrap()
            .supports_debug()
            .supports_copy()
            .supports_ord()
            .supports_hash()
            .into()])
            .unwrap()
            .into()
    }
}

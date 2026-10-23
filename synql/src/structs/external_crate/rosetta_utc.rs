//! Submodule implementing the method `rosetta_utc` for the
//! [`ExternalCrate`] struct which initializes a `ExternalCrate` instance
//! describing the `rosetta_utc` crate.

use crate::structs::{ExternalCrate, ExternalType};

impl ExternalCrate {
    /// Returns `ExternalCrate` instance describing the
    /// `rosetta_utc` crate.
    #[must_use]
    pub fn rosetta_utc() -> ExternalCrate {
        ExternalCrate::new("rosetta-utc")
            .unwrap()
            .version("0.1.0")
            .unwrap()
            .features(["diesel", "serde", "sqlite"])
            .types([ExternalType::new(
                syn::parse_quote!(::rosetta_utc::diesel_impls::TimestampUTC),
                syn::parse_quote!(::rosetta_utc::TimestampUTC),
            )
            .postgres_types(["timestamp with time zone", "timestamptz"])
            .unwrap()
            .supports_debug()
            .supports_copy()
            .supports_eq()
            .supports_ord()
            .supports_hash()
            .into()])
            .unwrap()
            .into()
    }
}

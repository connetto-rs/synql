//! Submodule implementing the method `rosetta_time` for the
//! [`ExternalCrate`] struct which initializes a `ExternalCrate` instance
//! describing the `rosetta_time` crate.

use crate::structs::{ExternalCrate, ExternalType};

impl ExternalCrate {
    /// Returns `ExternalCrate` instance describing the
    /// `rosetta_time` crate.
    #[must_use]
    pub fn rosetta_time() -> ExternalCrate {
        ExternalCrate::new("rosetta-time")
            .unwrap()
            .version("0.1.1")
            .unwrap()
            .features(["diesel", "serde", "sqlite"])
            .types([
                ExternalType::new(
                    syn::parse_quote!(::rosetta_time::diesel_impls::Date),
                    syn::parse_quote!(::rosetta_time::Date),
                )
                .postgres_type("date")
                .unwrap()
                .supports_debug()
                .supports_copy()
                .supports_eq()
                .supports_ord()
                .supports_hash()
                .into(),
                ExternalType::new(
                    syn::parse_quote!(::rosetta_time::diesel_impls::Time),
                    syn::parse_quote!(::rosetta_time::Time),
                )
                .postgres_types(["time", "time without time zone"])
                .unwrap()
                .supports_debug()
                .supports_copy()
                .supports_eq()
                .supports_ord()
                .supports_hash()
                .into(),
                ExternalType::new(
                    syn::parse_quote!(::rosetta_time::diesel_impls::Interval),
                    syn::parse_quote!(::rosetta_time::Span),
                )
                .postgres_type("interval")
                .unwrap()
                .supports_debug()
                .supports_copy()
                .supports_eq()
                .supports_ord()
                .supports_hash()
                .into(),
            ])
            .unwrap()
            .into()
    }
}

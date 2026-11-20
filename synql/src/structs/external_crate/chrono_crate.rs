//! Submodule implementing the method `chrono` for the [`ExternalCrate`] struct
//! which initializes a `ExternalCrate` instance describing the `chrono` crate.

use crate::structs::{ExternalCrate, ExternalType};

impl ExternalCrate {
    /// Returns the `ExternalCrate` instance describing the `chrono`
    /// crate.
    #[must_use]
    pub fn chrono() -> ExternalCrate {
        ExternalCrate::new("chrono")
            .unwrap()
            .version("0.4.42")
            .unwrap()
            .feature("serde")
            .types([
                ExternalType::new(
                    syn::parse_quote!(::diesel::sql_types::Timestamp),
                    syn::parse_quote!(::chrono::NaiveDateTime),
                )
                .postgres_types(["timestamp", "timestamp without time zone"])
                .unwrap()
                .supports_debug()
                .supports_copy()
                .supports_ord()
                .supports_hash()
                .into(),
                ExternalType::new(
                    syn::parse_quote!(::diesel::sql_types::Date),
                    syn::parse_quote!(::chrono::NaiveDate),
                )
                .postgres_type("date")
                .unwrap()
                .supports_debug()
                .supports_copy()
                .supports_ord()
                .supports_hash()
                .into(),
                ExternalType::new(
                    syn::parse_quote!(::diesel::sql_types::Time),
                    syn::parse_quote!(::chrono::NaiveTime),
                )
                .postgres_type("time")
                .unwrap()
                .supports_debug()
                .supports_copy()
                .supports_ord()
                .supports_hash()
                .into(),
                ExternalType::new(
                    syn::parse_quote!(::diesel::sql_types::Interval),
                    syn::parse_quote!(::chrono::Duration),
                )
                .postgres_type("interval")
                .unwrap()
                .supports_debug()
                .supports_copy()
                .supports_ord()
                .supports_hash()
                .into(),
            ])
            .unwrap()
            .into()
    }
}

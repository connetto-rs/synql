//! Submodule implementing the method `sqlitegis` for the [`ExternalCrate`]
//! struct which initializes a `ExternalCrate` instance describing the
//! `sqlitegis` crate.

use crate::structs::{ExternalCrate, ExternalType};

impl ExternalCrate {
    /// Returns `ExternalCrate` instance describing the `sqlitegis` crate.
    #[must_use]
    pub fn sqlitegis() -> ExternalCrate {
        ExternalCrate::new("sqlitegis")
            .unwrap()
            .features(["diesel-postgres", "serde"])
            .types([ExternalType::geometry(), ExternalType::geography()])
            .unwrap()
            .version("0.3.1")
            .unwrap()
            .into()
    }
}

impl ExternalType {
    /// Every PostGIS geometry column, whatever its subtype, read as EWKB.
    fn geometry() -> Self {
        ExternalType::new(
            syn::parse_quote!(::sqlitegis::diesel::Geometry),
            syn::parse_quote!(::sqlitegis::diesel::GeoValue),
        )
        .postgres_types([
            "geometry",
            "point",
            "linestring",
            "polygon",
            "multipoint",
            "multilinestring",
            "multipolygon",
            "geometrycollection",
            "geometry(point, 4326)",
        ])
        .unwrap()
        .supports_clone()
        .supports_debug()
        .supports_partial_eq()
        .into()
    }

    /// Every PostGIS geography column, whose SRID must be 4326.
    fn geography() -> Self {
        ExternalType::new(
            syn::parse_quote!(::sqlitegis::diesel::Geography),
            syn::parse_quote!(::sqlitegis::diesel::GeoValue),
        )
        .postgres_types(["geography", "geography(point, 4326)"])
        .unwrap()
        .supports_clone()
        .supports_debug()
        .supports_partial_eq()
        .into()
    }
}

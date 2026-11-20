//! Submodule registering validation errors for external crates.

use crate::structs::ExternalCrate;

impl ExternalCrate {
    /// Returns `ExternalCrate` instance describing the
    /// `validation_errors` crate.
    #[must_use]
    pub fn validation_errors() -> ExternalCrate {
        ExternalCrate::new("validation-errors").unwrap().version("0.1.2").unwrap().into()
    }
}

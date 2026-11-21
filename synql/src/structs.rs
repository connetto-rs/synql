//! Submodule defining structs used to generate Rust code from SQL schema.

mod base_workspace;
pub use base_workspace::BaseWorkspace;
pub(crate) use base_workspace::{BaseCrates, DescribedCrate};
mod synql;
pub use synql::{SynQL, SynQLBuilder};
mod workspace;
pub use workspace::{Workspace, WorkspaceBuilder};
mod external_crate;
pub use external_crate::{ExternalCrate, ExternalCrateBuilder};
mod enum_variant;
pub use enum_variant::EnumVariant;
pub(crate) use enum_variant::rust_type_name;
mod external_type;
pub use external_type::{ExternalType, ExternalTypeBuilder, Trait};
mod external_type_ref;
pub use external_type_ref::ExternalTypeRef;
mod external_function_ref;
pub use external_function_ref::ExternalFunctionRef;
mod external_function;
pub use external_function::ExternalFunction;
mod errors;
pub use errors::{Error, UnsupportedCheck};
mod toml_dependency;
pub use toml_dependency::TomlDependency;

//! Submodule providing the variant of a Rust enum standing for a label of a
//! PostgreSQL enum type.

use heck::ToUpperCamelCase;
use syn::Ident;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// The variant of a Rust enum standing for a label of a PostgreSQL enum type.
pub struct EnumVariant {
    /// The label PostgreSQL spells the value with.
    label: String,
    /// The identifier of the Rust variant.
    ident: Ident,
}

impl EnumVariant {
    /// Returns the variants standing for the labels, in their order, each
    /// named after its label in upper camel case.
    ///
    /// A name that is not a Rust identifier, as one of a label starting with
    /// a digit or made only of punctuation, is prefixed with `Label`. A name
    /// taken by an earlier label gets the smallest number from 2 up that makes
    /// it unique.
    ///
    /// # Example
    ///
    /// ```rust
    /// use synql::prelude::EnumVariant;
    ///
    /// let variants = EnumVariant::from_labels(["signed_in", "1st", "", "a-b", "a_b"]);
    /// let names: Vec<String> = variants.iter().map(|variant| variant.ident().to_string()).collect();
    /// assert_eq!(names, ["SignedIn", "Label1st", "Label", "AB", "AB2"]);
    /// assert_eq!(variants[3].label(), "a-b");
    /// ```
    pub fn from_labels<'label>(labels: impl IntoIterator<Item = &'label str>) -> Vec<Self> {
        let mut variants: Vec<Self> = Vec::new();
        for label in labels {
            let name = rust_type_name(label);
            let taken = |candidate: &str| variants.iter().any(|variant| variant.ident == candidate);
            let unique_name = if taken(&name) {
                (2..=variants.len() + 2)
                    .map(|suffix| format!("{name}{suffix}"))
                    .find(|candidate| !taken(candidate))
                    .expect("More candidates than variants leave one free")
            } else {
                name
            };
            variants.push(Self {
                label: label.to_owned(),
                ident: Ident::new(&unique_name, proc_macro2::Span::call_site()),
            });
        }
        variants
    }

    /// Returns the label PostgreSQL spells the value with.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the identifier of the Rust variant.
    #[must_use]
    pub fn ident(&self) -> &Ident {
        &self.ident
    }
}

/// Returns the upper camel case Rust name of a SQL name, prefixed with
/// `Label` when the upper camel case is not a Rust identifier.
pub(crate) fn rust_type_name(sql_name: &str) -> String {
    let name = sql_name.to_upper_camel_case();
    if syn::parse_str::<Ident>(&name).is_ok() { name } else { format!("Label{name}") }
}

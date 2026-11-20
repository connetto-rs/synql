//! Submodule providing a struct defining the type required by some type found
//! in the postgres database schema.

mod builder;
mod traits_mask;
use std::{fmt::Debug, hash::Hash};

pub use builder::ExternalTypeBuilder;
use quote::{ToTokens, quote};
pub use traits_mask::Trait;

use crate::structs::EnumVariant;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Struct defining the type required by some type found in the postgres
/// database schema.
pub struct ExternalType {
    /// The diesel type defined within the crate compatible with the given
    /// postgres type.
    diesel_type: syn::Type,
    /// The rust type defined within the crate compatible with the given
    /// postgres type.
    rust_type: syn::Type,
    /// The postgres types which are compatible with the diesel and rust types
    /// defined within the crate.
    postgres_types: Vec<String>,
    /// The variants of a Rust enum standing for a PostgreSQL enum type, each
    /// with the label PostgreSQL spells it with, `None` for every other type.
    variants: Option<Vec<EnumVariant>>,
    /// The traits supported by the current type.
    traits: traits_mask::TraitsMask,
}

impl ExternalType {
    /// Inizializes a new `ExternalTypeBuilder`.
    #[must_use]
    pub fn new(diesel_type: syn::Type, rust_type: syn::Type) -> ExternalTypeBuilder {
        ExternalTypeBuilder::new(diesel_type, rust_type)
    }

    /// Returns the diesel type defined within the crate compatible with the
    /// given postgres type.
    #[must_use]
    pub fn diesel_type(&self) -> &syn::Type {
        &self.diesel_type
    }

    /// Returns the rust type defined within the crate compatible with the given
    /// postgres type.
    #[must_use]
    pub fn rust_type(&self) -> &syn::Type {
        &self.rust_type
    }

    /// Returns a reference over the postgres types which are compatible with
    /// the diesel and rust types defined within the crate.
    #[must_use]
    pub fn postgres_types(&self) -> &[String] {
        &self.postgres_types
    }

    /// Returns the variants of the Rust enum standing for a PostgreSQL enum
    /// type, in the order PostgreSQL sorts their labels, or an empty slice
    /// for every other type.
    #[must_use]
    pub fn variants(&self) -> &[EnumVariant] {
        self.variants.as_deref().unwrap_or_default()
    }

    /// Returns whether the type is a Rust enum standing for a PostgreSQL enum
    /// type.
    #[must_use]
    pub fn is_enum(&self) -> bool {
        self.variants.is_some()
    }

    /// Returns whether the type is a `Unit` type.
    #[must_use]
    pub fn is_unit(&self) -> bool {
        self.rust_type.to_token_stream().to_string() == "()"
    }

    /// Returns whether the Rust type associated with the current `ExternalType`
    /// supports the given trait.
    ///
    /// # Arguments
    ///
    /// * `trait_ref` - The trait to check support for.
    #[must_use]
    pub fn supports(&self, trait_ref: Trait) -> bool {
        self.traits.supports(trait_ref)
    }

    /// Returns whether the current `ExternalType` is compatible with the given
    /// postgres type.
    ///
    /// # Arguments
    /// * `postgres_type` - The postgres type to check compatibility with.
    #[must_use]
    pub fn is_compatible_with(&self, postgres_type: &str) -> bool {
        self.postgres_types.iter().any(|t| t.eq_ignore_ascii_case(postgres_type))
    }

    /// Casts a value to the external type, the label of a PostgreSQL enum
    /// type to its variant.
    pub(crate) fn cast(&self, value: &str) -> Result<proc_macro2::TokenStream, syn::Error> {
        let string_rust_type = self.rust_type.to_token_stream().to_string();

        // Reusable error factory for parse failures in the match arms.
        let mk_err = || {
            syn::Error::new_spanned(
                self.rust_type.to_token_stream(),
                format!("Cannot cast value to external type: {self:?}"),
            )
        };

        if self.is_enum() {
            let variant = self
                .variants()
                .iter()
                .find(|variant| variant.label() == value)
                .ok_or_else(mk_err)?;
            let rust_type = &self.rust_type;
            let ident = variant.ident();
            return Ok(quote! { #rust_type::#ident });
        }

        Ok(match string_rust_type.as_str() {
            "String" | "str" => {
                quote! { #value }
            }
            "bool" => {
                let casted: bool = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            // Numeric parsing arms. Keep them explicit (simple and clear)
            // while still using the shared `mk_err` closure above to
            // avoid duplicated error construction.
            "i16" => {
                let casted: i16 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "u16" => {
                let casted: u16 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "i32" => {
                let casted: i32 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "u32" => {
                let casted: u32 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "i64" => {
                let casted: i64 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "u64" => {
                let casted: u64 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "usize" => {
                let casted: usize = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "isize" => {
                let casted: isize = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "f32" => float_tokens::<f32>(value, &self.rust_type).ok_or_else(mk_err)?,
            "f64" => float_tokens::<f64>(value, &self.rust_type).ok_or_else(mk_err)?,
            "u128" => {
                let casted: u128 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            "i128" => {
                let casted: i128 = value.parse().map_err(|_| mk_err())?;
                quote! { #casted }
            }
            _ => {
                return Err(mk_err());
            }
        })
    }

    /// Returns whether the type is a Rust float and the value is PostgreSQL's
    /// spelling of NaN for it.
    pub(crate) fn is_nan_literal(&self, value: &str) -> bool {
        matches!(self.rust_type.to_token_stream().to_string().as_str(), "f32" | "f64")
            && matches!(FloatInput::read(value), FloatInput::NotANumber)
    }

    /// Returns true if the `ExternalType` is of boolean type.
    #[must_use]
    pub fn is_bool(&self) -> bool {
        self.rust_type().to_token_stream().to_string() == "bool"
    }

    /// Returns true if the `ExternalType` is of numeric type.
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self.rust_type().to_token_stream().to_string().as_str(),
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "f32"
                | "f64"
        )
    }
}

/// A float literal as PostgreSQL's float input reads it.
enum FloatInput<'text> {
    /// A number, without the whitespace PostgreSQL ignores around it.
    Number(&'text str),
    /// `Infinity`, `inf` or either with a `+` sign, in any case.
    Infinity,
    /// `-Infinity` or `-inf`, in any case.
    NegativeInfinity,
    /// `NaN`, in any case and with either sign.
    NotANumber,
}

impl<'text> FloatInput<'text> {
    /// Reads the text as PostgreSQL's float input does.
    fn read(text: &'text str) -> Self {
        let text = text.trim_matches(|character| {
            matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}')
        });
        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(unsigned) => (true, unsigned),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        if unsigned.eq_ignore_ascii_case("infinity") || unsigned.eq_ignore_ascii_case("inf") {
            if negative { Self::NegativeInfinity } else { Self::Infinity }
        } else if unsigned.eq_ignore_ascii_case("nan") {
            Self::NotANumber
        } else {
            Self::Number(text)
        }
    }
}

/// Returns the tokens of the value read as a PostgreSQL float of the Rust
/// float type `F`, spelled `rust_type`, or `None` when the value is NaN, which
/// PostgreSQL orders above every other float while Rust leaves it unordered,
/// or is not a number `F` holds.
fn float_tokens<F>(value: &str, rust_type: &syn::Type) -> Option<proc_macro2::TokenStream>
where
    F: core::str::FromStr + ToTokens + Into<f64> + Copy,
{
    Some(match FloatInput::read(value) {
        FloatInput::Infinity => quote! { #rust_type::INFINITY },
        FloatInput::NegativeInfinity => quote! { #rust_type::NEG_INFINITY },
        FloatInput::NotANumber => return None,
        FloatInput::Number(number) => {
            let number: F = number.parse().ok()?;
            if !number.into().is_finite() {
                return None;
            }
            quote! { #number }
        }
    })
}

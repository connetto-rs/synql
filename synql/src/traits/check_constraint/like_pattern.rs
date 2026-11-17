//! Submodule compiling a literal SQL `LIKE` pattern into a Rust expression
//! matching a sequence of characters against it.
//!
//! The pattern splits at its `%` wildcards into segments of fixed length,
//! the first anchored at the start of the text and the last at its end. The
//! segments in between are each searched for leftmost, which finds a match
//! whenever one exists because every segment matches a fixed number of
//! characters.

use proc_macro2::TokenStream;
use quote::quote;

/// One character position of a `LIKE` pattern.
#[derive(Clone, Copy)]
enum Position {
    /// `_`, matching any single character.
    Any,
    /// A character matching itself.
    Literal(char),
}

/// A literal `LIKE` pattern split at its `%` wildcards.
pub(super) struct LikePattern {
    /// The runs of positions between the `%` wildcards, one more than the
    /// wildcards.
    segments: Vec<Vec<Position>>,
}

/// Returns the conditions matching the segment against the next characters
/// of the iterator `characters`, advancing it.
fn segment_conditions(segment: &[Position], characters: &TokenStream) -> Vec<TokenStream> {
    segment
        .iter()
        .map(|position| {
            match position {
                Position::Any => quote! { #characters.next().is_some() },
                Position::Literal(character) => quote! { #characters.next() == Some(#character) },
            }
        })
        .collect()
}

/// Returns the loop searching `rest` for the leftmost start of the segment,
/// running `found` with `candidate` past the match, and leaving the
/// `'like` block with FALSE when no start remains.
///
/// With `at_end`, the match must also end the text.
fn segment_search(segment: &[Position], at_end: bool, found: &TokenStream) -> TokenStream {
    let candidate = quote! { candidate };
    let mut conditions = segment_conditions(segment, &candidate);
    if at_end {
        conditions.push(quote! { candidate.next().is_none() });
    }
    quote! {
        loop {
            let mut candidate = rest.clone();
            if #(#conditions)&&* {
                #found
            }
            if rest.next().is_none() {
                break 'like false;
            }
        }
    }
}

impl LikePattern {
    /// Parses the pattern with PostgreSQL's default escape character, the
    /// backslash, lowercasing its literal characters when
    /// `case_insensitive`.
    ///
    /// Returns `None` for a pattern ending with the escape character, which
    /// PostgreSQL rejects.
    pub(super) fn parse(pattern: &str, case_insensitive: bool) -> Option<Self> {
        let mut segments = Vec::new();
        let mut segment = Vec::new();
        let mut characters = pattern.chars();
        while let Some(character) = characters.next() {
            let literal = match character {
                '%' => {
                    segments.push(std::mem::take(&mut segment));
                    continue;
                }
                '_' => {
                    segment.push(Position::Any);
                    continue;
                }
                '\\' => characters.next()?,
                other => other,
            };
            if case_insensitive {
                segment.extend(literal.to_lowercase().map(Position::Literal));
            } else {
                segment.push(Position::Literal(literal));
            }
        }
        segments.push(segment);
        Some(Self { segments })
    }

    /// Returns the boolean expression matching the characters yielded by
    /// `characters`, a cloneable iterator of `char`, against the pattern.
    pub(super) fn matcher(&self, characters: &TokenStream) -> TokenStream {
        let (first, after_first) =
            self.segments.split_first().expect("A pattern has a first segment");
        let Some((last, middle)) = after_first.split_last() else {
            let conditions = segment_conditions(first, &quote! { characters });
            return quote! {{
                let mut characters = #characters;
                #(#conditions &&)* characters.next().is_none()
            }};
        };
        let middle = middle.iter().filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
        if first.is_empty() && middle.is_empty() && last.is_empty() {
            return quote! {{
                let _ = #characters;
                true
            }};
        }
        let prefix = (!first.is_empty()).then(|| {
            let conditions = segment_conditions(first, &quote! { rest });
            quote! {
                if !(#(#conditions)&&*) {
                    break 'like false;
                }
            }
        });
        let advance = quote! {
            rest = candidate;
            break;
        };
        let accept = quote! { break 'like true; };
        let (searches, tail) = match middle.split_last() {
            Some((last_middle, earlier)) if last.is_empty() => {
                (
                    earlier
                        .iter()
                        .map(|segment| segment_search(segment, false, &advance))
                        .collect(),
                    segment_search(last_middle, false, &accept),
                )
            }
            _ if last.is_empty() => (Vec::new(), quote! { true }),
            _ => {
                (
                    middle.iter().map(|segment| segment_search(segment, false, &advance)).collect(),
                    segment_search(last, true, &accept),
                )
            }
        };
        quote! {
            'like: {
                let mut rest = #characters;
                #prefix
                #(#searches)*
                #tail
            }
        }
    }
}

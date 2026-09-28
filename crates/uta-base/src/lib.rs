//! Identifiers whose source is outside the core.
//!
//! Integration ids and program ids are written by Alice into the shared
//! configuration files (design core/core-process/design.md §4.6); stream names and write lane keys come
//! from integration declarations and program values (core/core-process/integration-session.md §3.3, core/core-process/program-host-element.md §3.1). The core
//! never mints them. They enter the core exactly once, through `parse`
//! (or serde, which is routed through the same parser), and are passed around
//! as cheap `Arc<str>` clones afterwards.
//!
//! Identifiers minted by the core itself (instance ids, stream epochs,
//! sequence numbers, session epochs) are defined in the crate that mints them,
//! with private constructors, so that no other crate can fabricate one.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Longest identifier accepted from a configuration file.
pub const MAX_ID_LEN: usize = 128;

/// Why a configuration identifier was rejected at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    #[error("identifier is empty")]
    Empty,
    #[error("identifier is {len} bytes, longer than {MAX_ID_LEN}")]
    TooLong { len: usize },
    #[error("identifier contains {ch:?}; allowed are ASCII letters, digits, '.', '_', '-'")]
    InvalidChar { ch: char },
}

fn validate(raw: &str) -> Result<(), IdError> {
    if raw.is_empty() {
        return Err(IdError::Empty);
    }
    if raw.len() > MAX_ID_LEN {
        return Err(IdError::TooLong { len: raw.len() });
    }
    match raw
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
    {
        Some(ch) => Err(IdError::InvalidChar { ch }),
        None => Ok(()),
    }
}

macro_rules! config_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Arc<str>);

        impl $name {
            /// Parses an identifier read from a configuration file.
            pub fn parse(raw: &str) -> Result<Self, IdError> {
                validate(raw)?;
                Ok(Self(Arc::from(raw)))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        /// Serialized as the plain string, without an intermediate copy.
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        /// Deserialized through [`Self::parse`], from the deserializer's
        /// (possibly borrowed) string: the only copy is the `Arc<str>`.
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct Visitor;
                impl serde::de::Visitor<'_> for Visitor {
                    type Value = $name;
                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str(concat!("a ", stringify!($name), " string"))
                    }
                    fn visit_str<E: serde::de::Error>(self, raw: &str) -> Result<$name, E> {
                        $name::parse(raw).map_err(E::custom)
                    }
                }
                deserializer.deserialize_str(Visitor)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($name), &*self.0)
            }
        }
    };
}

config_id! {
    /// An integration registration id from the integration registry file.
    /// Alice keeps an id bound to one integration forever (design core/core-process/integration-session.md §4.7).
    IntegrationId
}

config_id! {
    /// A program id from the program load manifest (design core/core-process/program-host-element.md §4.8).
    ProgramId
}

config_id! {
    /// A stream name: declared by an integration (`StreamDecl`, core/core-process/integration-session.md §3.3) or taken
    /// from a program value's `outputs` (core/core-process/program-host-element.md §3.1).
    StreamName
}

config_id! {
    /// A write lane key (`WriteScope.key`, core/core-process/integration-session.md §3.3): declared by the integration,
    /// compared by the core as an opaque value.
    WriteLaneKey
}

/// The source of an observation stream (core/core-process/design.md §3.1): integration ids and program ids
/// are separate namespaces, so the source carries the tag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    Integration(IntegrationId),
    Program(ProgramId),
}

/// What a child process of the core is, as recorded in the process table
/// (design core/core-process/storage.md §4.3 "进程表"). Integration ids and program ids are separate
/// namespaces, so the role carries the namespace tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProcessRole {
    Integration(IntegrationId),
    ProgramHost(ProgramId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_what_the_file_format_forbids() {
        assert_eq!(IntegrationId::parse(""), Err(IdError::Empty));
        assert_eq!(
            IntegrationId::parse("ibkr main"),
            Err(IdError::InvalidChar { ch: ' ' })
        );
        let long = "a".repeat(MAX_ID_LEN + 1);
        assert_eq!(
            ProgramId::parse(&long),
            Err(IdError::TooLong {
                len: MAX_ID_LEN + 1
            })
        );
        assert_eq!(
            IntegrationId::parse("ibkr-main_1.0").unwrap().as_str(),
            "ibkr-main_1.0"
        );
    }

    #[test]
    fn serde_goes_through_the_parser() {
        let ok: IntegrationId = serde_json::from_str("\"okx\"").unwrap();
        assert_eq!(ok.as_str(), "okx");
        assert!(serde_json::from_str::<IntegrationId>("\"bad/id\"").is_err());
        assert_eq!(serde_json::to_string(&ok).unwrap(), "\"okx\"");
    }
}

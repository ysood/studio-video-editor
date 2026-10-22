//! Typed identifiers. Each entity kind has its own type so a `ClipId` can never be passed
//! where a `MediaId` is expected.

use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS,
        )]
        #[serde(transparent)]
        #[ts(export, type = "string")]
        // A uuid is a string on the wire, and describing it as one keeps the schema
        // readable: `format` says what kind of string without turning every id into a
        // definition of its own.
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "schema", schemars(with = "String", extend("format" = "uuid")))]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Read one back from the text `Display` writes, so a caller that was given
            /// an id as text can turn it into the typed form. Surrounding space is
            /// tolerated; anything that is not a uuid is `None` rather than an error,
            /// because the usual caller is deciding whether this is an id at all.
            pub fn parse(text: &str) -> Option<Self> {
                Uuid::parse_str(text.trim()).ok().map(Self)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

define_id!(ProjectId);
define_id!(SequenceId);
define_id!(MediaId);
define_id!(TrackId);
define_id!(ClipId);
define_id!(LinkId);

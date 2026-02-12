use std::fmt;
use std::str::FromStr;

use snafu::ResultExt;
use uuid::Uuid;

use super::error::{InvalidIdSnafu, StorageError, StorageResult};

// Macro keeps all ID wrappers structurally identical, so future migrations stay predictable.
macro_rules! define_storage_id {
    ($name:ident, $id_type:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new(raw: Uuid) -> Self {
                Self(raw)
            }

            pub fn new_v7() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn parse(raw: &str) -> StorageResult<Self> {
                let parsed = Uuid::parse_str(raw).context(InvalidIdSnafu {
                    stage: "parse-storage-id",
                    id_type: $id_type,
                    raw: raw.to_string(),
                })?;
                Ok(Self(parsed))
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl FromStr for $name {
            type Err = StorageError;

            fn from_str(raw: &str) -> StorageResult<Self> {
                Self::parse(raw)
            }
        }
    };
}

define_storage_id!(SessionId, "session-id");
define_storage_id!(MessageId, "message-id");
define_storage_id!(BranchId, "branch-id");
define_storage_id!(MediaRefId, "media-ref-id");
define_storage_id!(AgentEventId, "agent-event-id");

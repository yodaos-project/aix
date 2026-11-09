//! Versioned package metadata for Progressive Asset references.

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

pub const PREFIX: &str = "META-INF/assets/progressive_assets/";
pub const INDEX_PATH: &str = "META-INF/assets/progressive_assets/index.json";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExtension {
    pub schema_version: u32,
    pub index_path: String,
    pub index_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Audio,
    Video,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub schema_version: u32,
    pub assets: BTreeMap<String, IndexEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "delivery", rename_all = "snake_case")]
pub enum IndexEntry {
    Embedded {
        kind: AssetKind,
        path: String,
    },
    Progressive {
        kind: AssetKind,
        path: String,
        sha256: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Descriptor {
    pub schema_version: u32,
    pub asset_version: String,
    pub kind: AssetKind,
    pub delivery: String,
    #[serde(flatten)]
    pub profile: Profile,
    pub locators: BTreeMap<String, RemoteLocator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub profile: String,
    pub required_capabilities: Vec<String>,
    pub local_metadata: BTreeMap<String, Content>,
    pub objects: BTreeMap<String, Content>,
    pub layout: TimelineLayout,
}

impl Profile {
    /// Stable content bytes. Object member order and locator metadata do not
    /// contribute to the version.
    pub fn canonical_bytes(&self) -> serde_json::Result<Vec<u8>> {
        let mut value = serde_json::to_value(self)?;
        value.sort_all_objects();
        serde_json::to_vec(&value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineLayout {
    pub init: String,
    #[serde(with = "track_map")]
    pub tracks: BTreeMap<u32, TimelineTrack>,
    pub fragments: Vec<Fragment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineTrack {
    pub kind: AssetKind,
    pub timescale: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fragment {
    pub object: String,
    #[serde(with = "track_map")]
    pub ranges: BTreeMap<u32, TimeRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeRange {
    pub start: u64,
    pub end: u64,
    pub presentation_start: i64,
    pub presentation_end: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteLocator {
    pub url: String,
}

// JSON object names are strings. Explicit conversion also works through
// the Web SDK's object serializer.
mod track_map {
    use alloc::{
        collections::BTreeMap,
        string::{String, ToString},
    };
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, V: Serialize>(
        tracks: &BTreeMap<u32, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        tracks
            .iter()
            .map(|(id, value)| (id.to_string(), value))
            .collect::<BTreeMap<_, _>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<u32, V>, D::Error> {
        BTreeMap::<String, V>::deserialize(deserializer)?
            .into_iter()
            .map(|(id, value)| {
                let parsed: u32 = id.parse().map_err(serde::de::Error::custom)?;
                if id != parsed.to_string() {
                    return Err(serde::de::Error::custom("invalid timeline track key"));
                }
                Ok((parsed, value))
            })
            .collect()
    }
}

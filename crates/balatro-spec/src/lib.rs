use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("failed to read bundle from {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse bundle json from {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceHashes {
    pub game_lua_sha256: String,
    pub wiki_jokers_sha256: String,
    pub love_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourcePaths {
    pub love_path: String,
    pub game_lua_entry: String,
    pub wiki_jokers_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleMetadata {
    pub version: String,
    pub generated_at: String,
    pub source_hashes: SourceHashes,
    pub source_paths: SourcePaths,
    pub sprite_defaults: BTreeMap<String, SpriteDefaults>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpriteDefaults {
    pub atlas: String,
    pub frame_w: u32,
    pub frame_h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpriteRef {
    pub atlas: String,
    pub x: i32,
    pub y: i32,
    pub frame_w: u32,
    pub frame_h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandSpec {
    pub key: String,
    pub name: String,
    pub base_chips: i32,
    pub base_mult: i32,
    pub level_chips: i32,
    pub level_mult: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlindSpec {
    pub id: String,
    pub name: String,
    pub order: i32,
    pub dollars: i32,
    pub mult: f32,
    pub boss: bool,
    pub showdown: bool,
    pub min_ante: Option<i32>,
    pub max_ante: Option<i32>,
    pub debuff: BTreeMap<String, serde_json::Value>,
    pub sprite: Option<SpriteRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StakeSpec {
    pub id: String,
    pub name: String,
    pub order: i32,
    pub stake_level: i32,
    pub unlocked: bool,
    pub sprite: Option<SpriteRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsumableSpec {
    pub id: String,
    pub name: String,
    pub set: String,
    pub order: i32,
    pub cost: i32,
    pub config: BTreeMap<String, serde_json::Value>,
    pub sprite: Option<SpriteRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JokerSpec {
    pub id: String,
    pub order: i32,
    pub name: String,
    pub set: String,
    pub cost: i32,
    pub rarity: i32,
    pub effect: Option<String>,
    pub config: BTreeMap<String, serde_json::Value>,
    pub unlocked: bool,
    pub blueprint_compat: bool,
    pub perishable_compat: bool,
    pub eternal_compat: bool,
    pub sprite: Option<SpriteRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopWeights {
    pub common: f32,
    pub uncommon: f32,
    pub rare: f32,
    pub legendary: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RulesetBundle {
    pub metadata: BundleMetadata,
    pub hand_specs: Vec<HandSpec>,
    pub ante_base_scores: Vec<i32>,
    pub blinds: Vec<BlindSpec>,
    pub stakes: Vec<StakeSpec>,
    pub jokers: Vec<JokerSpec>,
    pub consumables: Vec<ConsumableSpec>,
    pub sprite_manifest: BTreeMap<String, String>,
    pub shop_weights: ShopWeights,
}

impl RulesetBundle {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, BundleError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|source| BundleError::Read {
            path: path.display().to_string(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| BundleError::Parse {
            path: path.display().to_string(),
            source,
        })
    }

    pub fn stake_by_level(&self, level: i32) -> Option<&StakeSpec> {
        self.stakes.iter().find(|stake| stake.stake_level == level)
    }

    pub fn blind_by_id(&self, blind_id: &str) -> Option<&BlindSpec> {
        self.blinds.iter().find(|blind| blind.id == blind_id)
    }

    pub fn joker_by_id(&self, joker_id: &str) -> Option<&JokerSpec> {
        self.jokers.iter().find(|joker| joker.id == joker_id)
    }
}

#[cfg(test)]
mod tests {
    use super::RulesetBundle;
    use std::path::PathBuf;

    fn fixture_bundle() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/ruleset/balatro-1.0.1o-full.json")
    }

    #[test]
    fn bundle_fixture_loads() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("fixture bundle");
        assert_eq!(bundle.metadata.version, "1.0.1o-FULL");
        assert!(bundle.jokers.len() >= 150);
        assert!(bundle.blinds.len() >= 30);
        assert_eq!(bundle.stakes.len(), 8);
        assert_eq!(bundle.ante_base_scores.len(), 8);
    }
}

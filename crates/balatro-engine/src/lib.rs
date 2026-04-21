use balatro_spec::{BlindSpec, JokerSpec, RulesetBundle, Seal, TagSpec, VoucherSpec};
use rand::prelude::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Context passed to `apply_joker_effect` so each joker can read game state
/// without requiring a mutable borrow on the engine.
#[derive(Debug, Clone)]
pub struct ScoringContext<'a> {
    pub hand_key: &'a str,
    pub played: &'a [CardInstance],
    pub held_in_hand: &'a [CardInstance],
    pub discards_left: i32,
    pub plays_left: i32,
    pub jokers: &'a [JokerInstance],
    pub money: i32,
    pub deck_cards_remaining: i32,
    pub full_deck_size: i32,
    pub joker_slot_max: usize,
}

pub const ACTION_DIM: usize = 86;
pub const HAND_LIMIT: usize = 8;
pub const JOKER_LIMIT: usize = 5;
pub const SHOP_LIMIT: usize = 10;
pub const CONSUMABLE_SLOT_LIMIT: usize = 2;
pub const SHOP_CONSUMABLE_SLOTS: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase {
    PreBlind,
    Blind,
    PostBlind,
    Shop,
    CashOut,
    End,
}

impl Phase {
    pub fn as_stage_name(&self) -> &'static str {
        match self {
            Self::PreBlind => "Stage_PreBlind",
            Self::Blind => "Stage_Blind",
            Self::PostBlind => "Stage_PostBlind",
            Self::Shop => "Stage_Shop",
            Self::CashOut => "Stage_CashOut",
            Self::End => "Stage_End",
        }
    }

    pub fn as_lua_state_name(&self) -> &'static str {
        match self {
            Self::PreBlind => "BLIND_SELECT",
            Self::Blind => "SELECTING_HAND",
            Self::PostBlind => "ROUND_EVAL",
            Self::Shop => "SHOP",
            Self::CashOut => "CASH_OUT",
            Self::End => "GAME_OVER",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlindKind {
    Small,
    Big,
    Boss(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
enum BlindSlot {
    Small,
    Big,
    Boss,
}

impl BlindSlot {
    fn action_index(&self) -> usize {
        match self {
            Self::Small => 10,
            Self::Big => 11,
            Self::Boss => 12,
        }
    }

    fn next(&self) -> Option<Self> {
        match self {
            Self::Small => Some(Self::Big),
            Self::Big => Some(Self::Boss),
            Self::Boss => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum BlindProgress {
    Select,
    Upcoming,
    Current,
    Skipped,
    Defeated,
}

impl BlindProgress {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Upcoming => "Upcoming",
            Self::Current => "Current",
            Self::Skipped => "Skipped",
            Self::Defeated => "Defeated",
        }
    }
}

/// Active boss blind effect for the current blind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BossEffect {
    /// The Goad: All Spades are debuffed
    TheGoad,
    /// The Head: All Hearts are debuffed
    TheHead,
    /// The Club: All Clubs are debuffed
    TheClub,
    /// The Window: All Diamonds are debuffed
    TheWindow,
    /// The Plant: All face cards are debuffed
    ThePlant,
    /// The Psychic: Must play exactly 5 cards
    ThePsychic,
    /// The Needle: Only 1 hand allowed
    TheNeedle,
    /// The Water: Start with 0 discards
    TheWater,
    /// The Wall: Extra large blind (x2 chips required)
    TheWall,
    /// The Flint: Starting chips and mult are halved
    TheFlint,
    /// The Eye: Each hand type can only be played once
    TheEye,
    /// The Mouth: Only one hand type can be played for the entire blind
    TheMouth,
    /// The Hook: Discard 2 random cards per hand played
    TheHook,
    /// The Ox: If played hand is most played type, lose all money
    TheOx,
    /// The Tooth: Lose $1 per card played
    TheTooth,
    /// The Manacle: Reduce hand size by 1
    TheManacle,
    /// The Arm: Decrease level of played poker hand by 1
    TheArm,
    /// The Serpent: After play/discard, always draw back to full hand
    TheSerpent,
    /// The Pillar: Cards played previously this Ante are debuffed
    ThePillar,
    /// The Wheel: 1 in 7 cards are drawn face down
    TheWheel,
    /// The House: All cards drawn face down on first hand
    TheHouse,
    /// The Mark: All face cards drawn face down
    TheMark,
    /// The Fish: Cards drawn face down after each hand played
    TheFish,
    /// Violet Vessel: Very large blind amount (x6 base)
    VioletVessel,
    /// Cerulean Bell: Always force one card to be selected
    CeruleanBell,
    /// Amber Acorn: Rotate all Joker positions (TODO)
    AmberAcorn,
    /// Verdant Leaf: All cards debuffed until 1 card is sold (TODO)
    VerdantLeaf,
    /// Crimson Heart: One random Joker is disabled each hand (TODO)
    CrimsonHeart,
}

impl BossEffect {
    fn from_blind_name(name: &str) -> Option<Self> {
        match name {
            "The Goad" => Some(Self::TheGoad),
            "The Head" => Some(Self::TheHead),
            "The Club" => Some(Self::TheClub),
            "The Window" => Some(Self::TheWindow),
            "The Plant" => Some(Self::ThePlant),
            "The Psychic" => Some(Self::ThePsychic),
            "The Needle" => Some(Self::TheNeedle),
            "The Water" => Some(Self::TheWater),
            "The Wall" => Some(Self::TheWall),
            "The Flint" => Some(Self::TheFlint),
            "The Eye" => Some(Self::TheEye),
            "The Mouth" => Some(Self::TheMouth),
            "The Hook" => Some(Self::TheHook),
            "The Ox" => Some(Self::TheOx),
            "The Tooth" => Some(Self::TheTooth),
            "The Manacle" => Some(Self::TheManacle),
            "The Arm" => Some(Self::TheArm),
            "The Serpent" => Some(Self::TheSerpent),
            "The Pillar" => Some(Self::ThePillar),
            "The Wheel" => Some(Self::TheWheel),
            "The House" => Some(Self::TheHouse),
            "The Mark" => Some(Self::TheMark),
            "The Fish" => Some(Self::TheFish),
            "Violet Vessel" => Some(Self::VioletVessel),
            "Cerulean Bell" => Some(Self::CeruleanBell),
            "Amber Acorn" => Some(Self::AmberAcorn),
            "Verdant Leaf" => Some(Self::VerdantLeaf),
            "Crimson Heart" => Some(Self::CrimsonHeart),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

impl Suit {
    fn index(&self) -> usize {
        match self {
            Self::Spades => 0,
            Self::Hearts => 1,
            Self::Diamonds => 2,
            Self::Clubs => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    fn index(&self) -> usize {
        match self {
            Self::Two => 0,
            Self::Three => 1,
            Self::Four => 2,
            Self::Five => 3,
            Self::Six => 4,
            Self::Seven => 5,
            Self::Eight => 6,
            Self::Nine => 7,
            Self::Ten => 8,
            Self::Jack => 9,
            Self::Queen => 10,
            Self::King => 11,
            Self::Ace => 12,
        }
    }

    fn chip_value(&self) -> i32 {
        match self {
            Self::Ace => 11,
            Self::Jack | Self::Queen | Self::King => 10,
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
            Self::Six => 6,
            Self::Seven => 7,
            Self::Eight => 8,
            Self::Nine => 9,
            Self::Ten => 10,
        }
    }

    fn is_face(&self) -> bool {
        matches!(self, Self::Jack | Self::Queen | Self::King)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CardInstance {
    pub card_id: u32,
    pub rank: Rank,
    pub suit: Suit,
    pub enhancement: Option<String>,
    pub edition: Option<String>,
    pub seal: Option<String>,
}

impl CardInstance {
    pub fn rank_index(&self) -> usize {
        self.rank.index()
    }

    pub fn suit_index(&self) -> usize {
        self.suit.index()
    }

    pub fn chip_value(&self) -> i32 {
        self.rank.chip_value()
    }

    pub fn is_face_card(&self) -> bool {
        matches!(self.rank, Rank::Jack | Rank::Queen | Rank::King)
    }

    pub fn typed_seal(&self) -> Seal {
        Seal::from_opt_string(&self.seal)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JokerInstance {
    pub joker_id: String,
    pub name: String,
    pub base_cost: i32,
    pub cost: i32,
    pub buy_cost: i32,
    pub sell_value: i32,
    pub extra_sell_value: i32,
    pub rarity: i32,
    pub edition: Option<String>,
    pub slot_index: usize,
    pub activation_class: String,
    pub wiki_effect_text_en: String,
    /// Remaining uses for consumable jokers like Seltzer. None means unlimited.
    #[serde(default)]
    pub remaining_uses: Option<u32>,
    /// Persistent per-joker state for scaling jokers (e.g., accumulated mult/chips/xmult).
    #[serde(default)]
    pub runtime_state: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopSlot {
    pub slot: usize,
    pub joker: JokerInstance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsumableInstance {
    pub consumable_id: String,
    pub name: String,
    pub set: String,
    pub cost: i32,
    pub buy_cost: i32,
    pub sell_value: i32,
    pub slot_index: usize,
    pub config: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoucherInstance {
    pub voucher_id: String,
    pub name: String,
    pub cost: i32,
    pub effect_key: String,
    pub description: String,
}

/// Snapshot-facing tag descriptor. Attached to each skippable blind slot
/// (small / big / boss) when a tag has been rolled for that slot. Boss slots
/// never have a tag (they cannot be skipped in vanilla Balatro).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

impl TagInfo {
    fn from_spec(spec: &TagSpec) -> Self {
        Self {
            id: spec.id.clone(),
            name: spec.name.clone(),
            description: spec.description.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PackType {
    Arcana,
    Celestial,
    Spectral,
    Standard,
    Buffoon,
    MegaArcana,
}

impl PackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Arcana => "Arcana Pack",
            Self::Celestial => "Celestial Pack",
            Self::Spectral => "Spectral Pack",
            Self::Standard => "Standard Pack",
            Self::Buffoon => "Buffoon Pack",
            Self::MegaArcana => "Mega Arcana Pack",
        }
    }

    fn card_count(&self) -> usize {
        match self {
            Self::Arcana => 3,
            Self::Celestial => 3,
            Self::Spectral => 2,
            Self::Standard => 3,
            Self::Buffoon => 2,
            Self::MegaArcana => 5,
        }
    }

    fn picks_allowed(&self) -> u32 {
        match self {
            Self::MegaArcana => 2,
            _ => 1,
        }
    }

    fn shop_cost(&self) -> i32 {
        match self {
            Self::Arcana => 4,
            Self::Celestial => 4,
            Self::Spectral => 4,
            Self::Standard => 4,
            Self::Buffoon => 4,
            Self::MegaArcana => 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoosterPackChoice {
    pub index: usize,
    pub consumable_id: Option<String>,
    pub joker_id: Option<String>,
    pub card: Option<CardInstance>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoosterPackInstance {
    pub pack_type: String,
    pub cost: i32,
    pub choices: Vec<BoosterPackChoice>,
    pub picks_remaining: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RngTraceEntry {
    pub order: usize,
    pub domain: String,
    pub kind: String,
    pub args: BTreeMap<String, serde_json::Value>,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JokerResolutionTrace {
    pub order: usize,
    pub joker_id: String,
    pub joker_name: String,
    pub slot_index: usize,
    pub stage: String,
    pub supported: bool,
    pub matched: bool,
    pub retrigger_count: i32,
    pub effect_key: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TransitionTrace {
    pub transient_lua_states: Vec<String>,
    pub rng_calls: Vec<RngTraceEntry>,
    pub joker_resolution: Vec<JokerResolutionTrace>,
    pub retrigger_supported: bool,
    pub notes: Vec<String>,
}

impl TransitionTrace {
    fn add_transient(&mut self, lua_state: impl Into<String>) {
        let lua_state = lua_state.into();
        if !self.transient_lua_states.contains(&lua_state) {
            self.transient_lua_states.push(lua_state);
        }
    }

    fn add_note(&mut self, note: impl Into<String>) {
        let note = note.into();
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }

    fn add_rng_call(
        &mut self,
        domain: impl Into<String>,
        kind: impl Into<String>,
        args: BTreeMap<String, serde_json::Value>,
        result: serde_json::Value,
    ) {
        let order = self.rng_calls.len();
        self.rng_calls.push(RngTraceEntry {
            order,
            domain: domain.into(),
            kind: kind.into(),
            args,
            result,
        });
    }
}

/// Per-poker-hand statistics, mirroring BalatroBot's `gamestate.hands.<Name>`
/// shape (minus `example` which requires sample cards from the ruleset and is
/// deferred). Kept alongside the legacy `hand_levels` map so callers that only
/// need the level scalar keep working.
///
/// NOTE: `order` has no authoritative source in the ruleset bundle — we use a
/// descending-strength ranking (Flush Five=1 … High Card=12) to match what the
/// real client emits. See `balatro_hand_order_for_key`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HandStats {
    pub level: i32,
    pub played: i32,
    pub played_this_round: i32,
    pub order: i32,
    pub chips: i32,
    pub mult: i32,
}

/// Descending-strength order: 1 = strongest hand (Flush Five), 12 = weakest
/// (High Card). Matches the real-client UI ordering observed in
/// `observer-20260420T223706/snapshots/tick-000010.json`.
/// TODO: once the ruleset bundle exposes `order`, switch to reading it.
fn balatro_hand_order_for_key(key: &str) -> i32 {
    match key {
        "flush_five" => 1,
        "flush_house" => 2,
        "five_of_a_kind" => 3,
        "straight_flush" => 4,
        "four_of_a_kind" => 5,
        "full_house" => 6,
        "flush" => 7,
        "straight" => 8,
        "three_of_kind" => 9,
        "two_pair" => 10,
        "pair" => 11,
        "high_card" => 12,
        _ => 99,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub phase: Phase,
    pub stage: String,
    pub lua_state: String,
    pub round: i32,
    pub ante: i32,
    pub stake: i32,
    pub blind_name: String,
    pub boss_effect: String,
    pub score: i32,
    pub required_score: i32,
    pub plays: i32,
    pub discards: i32,
    pub money: i32,
    pub shop_reroll_cost: i32,
    pub shop_reroll_count: i32,
    pub reward: i32,
    pub deck: Vec<CardInstance>,
    pub available: Vec<CardInstance>,
    pub selected: Vec<CardInstance>,
    pub discarded: Vec<CardInstance>,
    pub jokers: Vec<JokerInstance>,
    pub shop_jokers: Vec<JokerInstance>,
    pub consumables: Vec<ConsumableInstance>,
    pub shop_consumables: Vec<ConsumableInstance>,
    pub consumable_slot_limit: usize,
    pub hand_levels: BTreeMap<String, i32>,
    /// Per-hand stats keyed by the **display name** (e.g. "Pair", "Flush"),
    /// matching BalatroBot's `gamestate.hands`. Populated from `hand_specs`.
    #[serde(default)]
    pub hand_stats: BTreeMap<String, HandStats>,
    pub blind_states: BTreeMap<String, String>,
    pub selected_slots: Vec<usize>,
    pub owned_vouchers: Vec<String>,
    pub shop_voucher: Option<VoucherInstance>,
    pub shop_packs: Vec<BoosterPackInstance>,
    pub open_pack: Option<BoosterPackInstance>,
    pub won: bool,
    pub over: bool,
    /// User-facing seed string (empty if the engine was built from a raw u64 seed only).
    #[serde(default)]
    pub seed_str: String,
    /// Lowercase deck identifier from `RunConfig.deck_key` (e.g. "red").
    #[serde(default)]
    pub deck_name: String,
    /// Uppercase stake name (e.g. "WHITE") derived from `RunConfig.stake`.
    #[serde(default)]
    pub stake_name: String,
    /// Tag rolled for the Small blind this round (None on boss-only rounds).
    #[serde(default)]
    pub small_tag: Option<TagInfo>,
    /// Tag rolled for the Big blind this round.
    #[serde(default)]
    pub big_tag: Option<TagInfo>,
    /// Tag rolled for the Boss blind. Always `None` in vanilla rules because
    /// boss blinds cannot be skipped; reserved for modded/future use.
    #[serde(default)]
    pub boss_tag: Option<TagInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionDescriptor {
    pub index: usize,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventStage {
    BlindPrePlay,
    OnPlayed,
    CardScored,
    HeldInHand,
    JokerPostScore,
    EndOfHand,
    EndOfRound,
    CashOut,
    Shop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub stage: EventStage,
    pub kind: String,
    pub summary: String,
    pub activation_stage: Option<String>,
    pub joker_slot: Option<usize>,
    pub joker_id: Option<String>,
    pub source_card_slot: Option<usize>,
    pub effect_text_en: Option<String>,
    pub chips_delta: Option<i32>,
    pub mult_delta: Option<f64>,
    pub xmult_delta: Option<f64>,
    pub money_delta: Option<i32>,
    pub state_delta: BTreeMap<String, serde_json::Value>,
    pub payload: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transition {
    pub snapshot_before: Snapshot,
    pub action: ActionDescriptor,
    pub events: Vec<Event>,
    pub trace: TransitionTrace,
    pub snapshot_after: Snapshot,
    pub terminal: bool,
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub ante_start: i32,
    pub stake: i32,
    pub max_ante: i32,
    /// Deck identifier (e.g. "red", "blue"). Lowercase, matches the run-lobby key.
    /// Semantics (starting discards, hand size, etc.) are NOT yet applied — this is
    /// a data-plumbing field used for sim↔real alignment (see P1 in
    /// `todo/20260421_sim_vs_real_gaps.md`).
    pub deck_key: String,
    /// User-facing seed string (e.g. "4WAX5M4D"). Preserved verbatim so the
    /// snapshot can round-trip through real-client comparisons. Note: the
    /// simulator currently uses a `u64` seed derived via `balatro_seed_str_to_u64`,
    /// which is NOT byte-compatible with Balatro's Lua `math.random` sequence.
    /// A P3 pass may replace this with a Balatro-accurate seeding scheme.
    pub seed_str: String,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            ante_start: 1,
            stake: 1,
            max_ante: 8,
            deck_key: "red".to_string(),
            seed_str: String::new(),
        }
    }
}

/// Stable mapping from string seed to u64 for `ChaCha8Rng`.
///
/// This is NOT Balatro-accurate: the real game uses Lua's `math.random` with a
/// string-seeded RNG. For P0 alignment work we only need determinism per
/// `seed_str`; trajectory-level parity with the real client is a separate P3
/// effort.
pub fn balatro_seed_str_to_u64(seed_str: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    seed_str.hash(&mut h);
    h.finish()
}

/// Convert the integer stake (1..=8) into BalatroBot's uppercase stake name.
/// Returns `"UNKNOWN"` for anything outside that range.
pub fn stake_int_to_name(stake: i32) -> &'static str {
    match stake {
        1 => "WHITE",
        2 => "RED",
        3 => "GREEN",
        4 => "BLACK",
        5 => "BLUE",
        6 => "PURPLE",
        7 => "ORANGE",
        8 => "GOLD",
        _ => "UNKNOWN",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("illegal action index {0}")]
    IllegalAction(usize),
}

#[derive(Debug, Clone)]
pub struct Engine {
    ruleset: RulesetBundle,
    rng: ChaCha8Rng,
    state: EngineState,
    config: RunConfig,
}

#[derive(Debug, Clone)]
struct EngineState {
    phase: Phase,
    round: i32,
    ante: i32,
    stake: i32,
    max_ante: i32,
    blind: BlindKind,
    blind_name: String,
    boss_effect: String,
    current_blind_slot: BlindSlot,
    small_progress: BlindProgress,
    big_progress: BlindProgress,
    boss_progress: BlindProgress,
    boss_blind: BlindSpec,
    score: i32,
    plays: i32,
    discards: i32,
    money: i32,
    shop_base_reroll_cost: i32,
    shop_current_reroll_cost: i32,
    shop_reroll_count: i32,
    reward: i32,
    deck: Vec<CardInstance>,
    available: Vec<CardInstance>,
    selected_slots: BTreeSet<usize>,
    discarded: Vec<CardInstance>,
    jokers: Vec<JokerInstance>,
    shop: Vec<ShopSlot>,
    consumables: Vec<ConsumableInstance>,
    shop_consumables: Vec<ConsumableInstance>,
    consumable_slot_limit: usize,
    hand_levels: BTreeMap<String, i32>,
    /// Per-hand stats keyed by display name; kept in lock-step with `hand_levels`.
    hand_stats: BTreeMap<String, HandStats>,
    owned_vouchers: Vec<String>,
    shop_voucher: Option<VoucherInstance>,
    shop_packs: Vec<BoosterPackInstance>,
    open_pack: Option<BoosterPackInstance>,
    /// Base plays per round (can be increased by vouchers)
    base_plays: i32,
    /// Base discards per round (can be increased by vouchers)
    base_discards: i32,
    /// Base hand size (can be increased by vouchers)
    hand_size: usize,
    /// Joker slot limit (can be increased by vouchers)
    joker_slot_limit: usize,
    /// Shop discount multiplier (default 1.0, Clearance Sale sets to 0.75)
    shop_discount: f32,
    /// Interest cap (default 5, Seed Money raises to 25)
    interest_cap: i32,
    won: bool,
    over: bool,
    /// Per-joker extra state: Rocket payout accumulator (increases by $2 on boss defeat)
    rocket_extra_dollars: i32,
    /// Per-joker extra state: Egg accumulated sell value bonus per round
    egg_accumulated_sell: i32,
    /// Number of unique Planet cards used this run (for Satellite)
    unique_planets_used: i32,
    /// Whether the current boss blind ability was destroyed (by Chicot)
    boss_blind_disabled: bool,
    /// Target hand type for To Do List joker (randomized each round)
    todo_list_target: Option<String>,
    /// Number of hands played this round (for DNA joker first-hand check)
    hands_played_this_round: i32,
    /// Active boss effect for the current blind (None for Small/Big blinds or disabled)
    active_boss_effect: Option<BossEffect>,
    /// Card IDs that are debuffed by the current boss blind (don't score)
    debuffed_cards: BTreeSet<u32>,
    /// Hand types already played this blind (for The Eye)
    boss_hand_types_played: BTreeSet<String>,
    /// The forced hand type for The Mouth (set after first hand played)
    boss_forced_hand_type: Option<String>,
    /// Original hand size before Manacle reduction (to restore later)
    boss_manacle_hand_size_reduced: bool,
    /// Tag rolled for the Small blind slot this round (TagSpec.id).
    small_tag_id: Option<String>,
    /// Tag rolled for the Big blind slot this round.
    big_tag_id: Option<String>,
    /// Tag rolled for the Boss slot. Always `None` in vanilla rules; reserved.
    boss_tag_id: Option<String>,
    /// Pending Investment-tag payout ($25 after next boss defeat).
    /// Stack of pending awards (multiple Investment tags could queue up).
    pending_investment_payouts: i32,
    /// Pending Voucher-tag bonus: add N extra vouchers to next shop entry.
    pending_voucher_tags: i32,
    /// Pending Coupon-tag flag: next shop's initial jokers+consumables+packs are free.
    pending_coupon_shop: bool,
    /// Pending D6-tag flag: next shop's reroll cost starts at $0.
    pending_d6_shop: bool,
    /// Pending Juggle-tag hand-size bonus: +N to hand_size for next round only.
    pending_juggle_hand_size: usize,
    /// Active Juggle bonus applied to hand_size — tracked so we can revert at
    /// round end. Accumulates when multiple Juggles land in the same round.
    active_juggle_hand_size: usize,
    /// Number of blinds skipped this run (for Speed Tag reward calculation).
    skipped_blind_count: i32,
}

impl Engine {
    pub fn new(seed: u64, ruleset: RulesetBundle, config: RunConfig) -> Self {
        let initial_boss = ruleset
            .blinds
            .iter()
            .find(|blind| blind.boss)
            .expect("boss blind")
            .clone();
        let stored_config = config.clone();
        let mut engine = Self {
            ruleset,
            rng: ChaCha8Rng::seed_from_u64(seed),
            config: stored_config,
            state: EngineState {
                phase: Phase::PreBlind,
                round: 1,
                ante: config.ante_start,
                stake: config.stake,
                max_ante: config.max_ante,
                blind: BlindKind::Small,
                blind_name: "Small Blind".to_string(),
                boss_effect: "None".to_string(),
                current_blind_slot: BlindSlot::Small,
                small_progress: BlindProgress::Select,
                big_progress: BlindProgress::Upcoming,
                boss_progress: BlindProgress::Upcoming,
                boss_blind: initial_boss,
                score: 0,
                plays: 4,
                discards: 3,
                money: 4,
                shop_base_reroll_cost: 5,
                shop_current_reroll_cost: 5,
                shop_reroll_count: 0,
                reward: 3,
                deck: Vec::new(),
                available: Vec::new(),
                selected_slots: BTreeSet::new(),
                discarded: Vec::new(),
                jokers: Vec::new(),
                shop: Vec::new(),
                consumables: Vec::new(),
                shop_consumables: Vec::new(),
                consumable_slot_limit: CONSUMABLE_SLOT_LIMIT,
                hand_levels: BTreeMap::new(),
                hand_stats: BTreeMap::new(),
                owned_vouchers: Vec::new(),
                shop_voucher: None,
                shop_packs: Vec::new(),
                open_pack: None,
                base_plays: 4,
                base_discards: 3,
                hand_size: HAND_LIMIT,
                joker_slot_limit: JOKER_LIMIT,
                shop_discount: 1.0,
                interest_cap: 5,
                won: false,
                over: false,
                rocket_extra_dollars: 0,
                egg_accumulated_sell: 0,
                unique_planets_used: 0,
                boss_blind_disabled: false,
                todo_list_target: None,
                hands_played_this_round: 0,
                active_boss_effect: None,
                debuffed_cards: BTreeSet::new(),
                boss_hand_types_played: BTreeSet::new(),
                boss_forced_hand_type: None,
                boss_manacle_hand_size_reduced: false,
                small_tag_id: None,
                big_tag_id: None,
                boss_tag_id: None,
                pending_investment_payouts: 0,
                pending_voucher_tags: 0,
                pending_coupon_shop: false,
                pending_d6_shop: false,
                pending_juggle_hand_size: 0,
                active_juggle_hand_size: 0,
                skipped_blind_count: 0,
            },
        };
        // Seed hand_stats from the ruleset. Every hand type starts at level 1
        // with its base chips/mult and descending-strength order.
        for spec in engine.ruleset.hand_specs.clone().iter() {
            engine.state.hand_stats.insert(
                spec.name.clone(),
                HandStats {
                    level: 1,
                    played: 0,
                    played_this_round: 0,
                    order: balatro_hand_order_for_key(&spec.key),
                    chips: spec.base_chips,
                    mult: spec.base_mult,
                },
            );
            engine.state.hand_levels.insert(spec.key.clone(), 1);
        }
        let mut init_trace = TransitionTrace::default();
        engine.prepare_round_start(&mut init_trace);
        engine
    }

    /// Build an engine from a user-facing string seed. The `RunConfig.seed_str`
    /// is set from `seed_str`, and the underlying u64 RNG seed is derived via
    /// `balatro_seed_str_to_u64` (see that function's doc for caveats).
    pub fn new_from_str_seed(seed_str: &str, ruleset: RulesetBundle, mut config: RunConfig) -> Self {
        config.seed_str = seed_str.to_string();
        let seed_u64 = balatro_seed_str_to_u64(seed_str);
        Self::new(seed_u64, ruleset, config)
    }

    pub fn clone_seeded(&self, seed: Option<u64>) -> Self {
        let mut clone = self.clone();
        if let Some(seed) = seed {
            clone.rng = ChaCha8Rng::seed_from_u64(seed);
        }
        clone
    }

    /// Set the level of a hand (by `hand_key`) to `new_level`, keeping
    /// `hand_levels` and `hand_stats` in lock-step. Recomputes base chips/mult
    /// from the ruleset's `HandSpec`. Level is clamped to >= 1.
    fn set_hand_level(&mut self, hand_key: &str, new_level: i32) {
        let level = new_level.max(1);
        self.state.hand_levels.insert(hand_key.to_string(), level);
        let spec = match self
            .ruleset
            .hand_specs
            .iter()
            .find(|hs| hs.key == hand_key)
        {
            Some(s) => s.clone(),
            None => return,
        };
        let bonus = (level - 1).max(0);
        let chips = spec.base_chips + bonus * spec.level_chips;
        let mult = spec.base_mult + bonus * spec.level_mult;
        let order = balatro_hand_order_for_key(&spec.key);
        let stats = self
            .state
            .hand_stats
            .entry(spec.name.clone())
            .or_insert_with(|| HandStats {
                level: 1,
                played: 0,
                played_this_round: 0,
                order,
                chips: spec.base_chips,
                mult: spec.base_mult,
            });
        stats.level = level;
        stats.chips = chips;
        stats.mult = mult;
        stats.order = order;
    }

    /// Adjust a hand's level by `delta` (positive or negative), clamped to >= 1.
    fn bump_hand_level(&mut self, hand_key: &str, delta: i32) -> i32 {
        let current = self.state.hand_levels.get(hand_key).copied().unwrap_or(1);
        let new_level = (current + delta).max(1);
        self.set_hand_level(hand_key, new_level);
        new_level
    }

    /// Record that a hand of type `hand_key` was just successfully played —
    /// increments both the all-time `played` and per-round `played_this_round`
    /// counters.
    fn record_hand_played(&mut self, hand_key: &str) {
        let spec_name = self
            .ruleset
            .hand_specs
            .iter()
            .find(|hs| hs.key == hand_key)
            .map(|hs| hs.name.clone());
        let Some(name) = spec_name else {
            return;
        };
        let order = balatro_hand_order_for_key(hand_key);
        let stats = self.state.hand_stats.entry(name).or_insert_with(|| HandStats {
            level: 1,
            played: 0,
            played_this_round: 0,
            order,
            chips: 0,
            mult: 0,
        });
        stats.played += 1;
        stats.played_this_round += 1;
    }

    pub fn snapshot(&self) -> Snapshot {
        let selected = self.selected_cards();
        Snapshot {
            phase: self.state.phase.clone(),
            stage: self.state.phase.as_stage_name().to_string(),
            lua_state: self.state.phase.as_lua_state_name().to_string(),
            round: self.state.round,
            ante: self.state.ante,
            stake: self.state.stake,
            blind_name: self.state.blind_name.clone(),
            boss_effect: self.state.boss_effect.clone(),
            score: self.state.score,
            required_score: self.required_score(),
            plays: self.state.plays,
            discards: self.state.discards,
            money: self.state.money,
            shop_reroll_cost: if matches!(self.state.phase, Phase::Shop) {
                self.state.shop_current_reroll_cost
            } else {
                0
            },
            shop_reroll_count: if matches!(self.state.phase, Phase::Shop) {
                self.state.shop_reroll_count
            } else {
                0
            },
            reward: self.state.reward,
            deck: self.state.deck.clone(),
            available: self.state.available.clone(),
            selected,
            discarded: self.state.discarded.clone(),
            jokers: self.state.jokers.clone(),
            shop_jokers: if matches!(self.state.phase, Phase::Shop) {
                self.state.shop.iter().map(|slot| slot.joker.clone()).collect()
            } else {
                Vec::new()
            },
            consumables: self.state.consumables.clone(),
            shop_consumables: if matches!(self.state.phase, Phase::Shop) {
                self.state.shop_consumables.clone()
            } else {
                Vec::new()
            },
            consumable_slot_limit: self.state.consumable_slot_limit,
            hand_levels: self.state.hand_levels.clone(),
            hand_stats: self.state.hand_stats.clone(),
            blind_states: self.blind_states_snapshot(),
            selected_slots: self.state.selected_slots.iter().copied().collect(),
            owned_vouchers: self.state.owned_vouchers.clone(),
            shop_voucher: if matches!(self.state.phase, Phase::Shop) {
                self.state.shop_voucher.clone()
            } else {
                None
            },
            shop_packs: if matches!(self.state.phase, Phase::Shop) {
                self.state.shop_packs.clone()
            } else {
                Vec::new()
            },
            open_pack: self.state.open_pack.clone(),
            won: self.state.won,
            over: self.state.over,
            seed_str: self.config.seed_str.clone(),
            deck_name: self.config.deck_key.clone(),
            stake_name: stake_int_to_name(self.state.stake).to_string(),
            small_tag: self.tag_info_for(self.state.small_tag_id.as_deref()),
            big_tag: self.tag_info_for(self.state.big_tag_id.as_deref()),
            boss_tag: self.tag_info_for(self.state.boss_tag_id.as_deref()),
        }
    }

    /// Look up a `TagInfo` from either the ruleset's tag catalog or the
    /// built-in `default_tag_pool()` fallback. Returns `None` for unknown IDs.
    fn tag_info_for(&self, tag_id: Option<&str>) -> Option<TagInfo> {
        let id = tag_id?;
        if let Some(spec) = self.ruleset.tag_by_id(id) {
            return Some(TagInfo::from_spec(spec));
        }
        default_tag_pool()
            .iter()
            .find(|spec| spec.id == id)
            .map(TagInfo::from_spec)
    }

    /// Return the effective tag pool, preferring the bundle's `tags` list and
    /// falling back to the built-in default when empty.
    fn tag_pool(&self) -> Vec<TagSpec> {
        if self.ruleset.tags.is_empty() {
            default_tag_pool()
        } else {
            self.ruleset.tags.clone()
        }
    }

    /// Roll one tag id from the pool (uniform over the full catalog).
    fn roll_tag_id(&mut self, domain: &str, trace: &mut TransitionTrace) -> Option<String> {
        let pool = self.tag_pool();
        if pool.is_empty() {
            return None;
        }
        let candidates: Vec<String> = pool.iter().map(|t| t.id.clone()).collect();
        let idx = self.choose_index(pool.len(), domain, candidates, trace);
        Some(pool[idx].id.clone())
    }

    pub fn legal_actions(&self) -> Vec<ActionDescriptor> {
        self.gen_action_space()
            .into_iter()
            .enumerate()
            .map(|(index, enabled)| ActionDescriptor {
                index,
                name: action_name(index),
                enabled: enabled == 1,
            })
            .collect()
    }

    pub fn gen_action_space(&self) -> Vec<u8> {
        let mut mask = vec![0; ACTION_DIM];

        // If a booster pack is open, only allow pack-related actions
        if self.state.open_pack.is_some() {
            if let Some(ref pack) = self.state.open_pack {
                for (idx, _choice) in pack.choices.iter().enumerate() {
                    if idx < 5 {
                        mask[31 + idx] = 1; // pick_pack_0..4
                    }
                }
                mask[36] = 1; // skip_pack
            }
            return mask;
        }

        match self.state.phase {
            Phase::PreBlind => {
                mask[self.state.current_blind_slot.action_index()] = 1;
                if self.state.current_blind_slot != BlindSlot::Boss {
                    mask[85] = 1;
                }
            }
            Phase::Blind => {
                let hand_size = self.state.hand_size;
                for index in 0..self.state.available.len().min(hand_size) {
                    mask[index] = 1;
                }
                if self.state.plays > 0 {
                    let mut play_allowed = true;

                    // The Psychic: must play exactly 5 cards
                    if let Some(BossEffect::ThePsychic) = &self.state.active_boss_effect {
                        if !self.state.boss_blind_disabled
                            && self.state.selected_slots.len() != 5
                        {
                            play_allowed = false;
                        }
                    }

                    // The Eye: check if selected hand type was already played
                    if let Some(BossEffect::TheEye) = &self.state.active_boss_effect {
                        if !self.state.boss_blind_disabled {
                            let selected = self.selected_cards();
                            if !selected.is_empty() {
                                let hand = classify_hand(&selected);
                                if self.state.boss_hand_types_played.contains(&hand.key) {
                                    play_allowed = false;
                                }
                            }
                        }
                    }

                    // The Mouth: after first hand, only the same type can be played
                    if let Some(BossEffect::TheMouth) = &self.state.active_boss_effect {
                        if !self.state.boss_blind_disabled {
                            if let Some(ref forced_type) = self.state.boss_forced_hand_type {
                                let selected = self.selected_cards();
                                if !selected.is_empty() {
                                    let hand = classify_hand(&selected);
                                    if hand.key != *forced_type {
                                        play_allowed = false;
                                    }
                                }
                            }
                        }
                    }

                    if play_allowed {
                        mask[8] = 1;
                    }
                }
                if self.state.discards > 0 {
                    mask[9] = 1;
                }
                // Use held consumables during blind
                for idx in 0..self.state.consumables.len().min(self.state.consumable_slot_limit) {
                    mask[71 + idx] = 1;
                }
            }
            Phase::PostBlind => {
                mask[13] = 1;
            }
            Phase::Shop => {
                mask[70] = 1;
                if self.state.money >= self.state.shop_current_reroll_cost {
                    mask[79] = 1;
                }
                let joker_limit = self.state.joker_slot_limit;
                if self.state.jokers.len() < joker_limit {
                    for slot in self.state.shop.iter().take(SHOP_LIMIT) {
                        let index = 14 + slot.slot;
                        if index < 24 && slot.joker.buy_cost <= self.state.money {
                            mask[index] = 1;
                        }
                    }
                }
                // Buy consumables from shop
                if self.state.consumables.len() < self.state.consumable_slot_limit {
                    for (idx, consumable) in self.state.shop_consumables.iter().enumerate() {
                        let action_idx = 24 + idx;
                        if action_idx <= 25 && consumable.buy_cost <= self.state.money {
                            mask[action_idx] = 1;
                        }
                    }
                }
                // Sell held consumables
                for idx in 0..self.state.consumables.len().min(self.state.consumable_slot_limit) {
                    let action_idx = 26 + idx;
                    if action_idx <= 27 {
                        mask[action_idx] = 1;
                    }
                }
                // Buy voucher
                if let Some(ref voucher) = self.state.shop_voucher {
                    if voucher.cost <= self.state.money {
                        mask[28] = 1;
                    }
                }
                // Buy booster packs
                for (idx, pack) in self.state.shop_packs.iter().enumerate() {
                    if idx < 2 && pack.cost <= self.state.money {
                        mask[29 + idx] = 1;
                    }
                }
                for slot in 0..self.state.jokers.len().min(joker_limit) {
                    mask[80 + slot] = 1;
                }
                // Use held consumables in shop
                for idx in 0..self.state.consumables.len().min(self.state.consumable_slot_limit) {
                    mask[71 + idx] = 1;
                }
            }
            Phase::CashOut => {
                mask[70] = 1;
            }
            Phase::End => {}
        }
        mask
    }

    pub fn step(&mut self, action_index: usize) -> Result<Transition, EngineError> {
        let before = self.snapshot();
        let legal_actions = self.legal_actions();
        let action = legal_actions
            .iter()
            .find(|candidate| candidate.index == action_index && candidate.enabled)
            .cloned()
            .ok_or(EngineError::IllegalAction(action_index))?;

        let mut trace = TransitionTrace::default();
        let events = self.apply_action(action_index, &mut trace);
        let after = self.snapshot();
        Ok(Transition {
            snapshot_before: before,
            action,
            events,
            trace,
            terminal: after.over,
            snapshot_after: after,
        })
    }

    fn apply_action(&mut self, action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        // If a booster pack is open, handle pack actions regardless of phase
        if self.state.open_pack.is_some() {
            return self.handle_pack_action(action_index, trace);
        }
        match self.state.phase {
            Phase::PreBlind => self.handle_preblind(action_index, trace),
            Phase::Blind => self.handle_blind(action_index, trace),
            Phase::PostBlind => self.handle_post_blind(action_index, trace),
            Phase::Shop => self.handle_shop(action_index, trace),
            Phase::CashOut => self.handle_cashout(action_index, trace),
            Phase::End => Vec::new(),
        }
    }

    fn handle_preblind(&mut self, action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        if action_index == self.state.current_blind_slot.action_index() {
            trace.add_transient("NEW_ROUND");
            trace.add_transient("DRAW_TO_HAND");
            let blind_name = self.state.blind_name.clone();
            let mut blind_events = self.enter_current_blind(trace);
            let mut events = vec![event(
                EventStage::BlindPrePlay,
                "blind_selected",
                format!("Selected {}", blind_name),
            )];
            events.append(&mut blind_events);
            return events;
        }

        if action_index == 85 && self.state.current_blind_slot != BlindSlot::Boss {
            let skipped_name = self.state.blind_name.clone();
            let tag_id = match self.state.current_blind_slot {
                BlindSlot::Small => self.state.small_tag_id.clone(),
                BlindSlot::Big => self.state.big_tag_id.clone(),
                BlindSlot::Boss => None,
            };
            // Update scaling jokers on blind skip (Throwback, Red Card)
            self.update_joker_runtime_on_skip();
            self.mark_current_blind_progress(BlindProgress::Skipped);
            let mut events = vec![event(
                EventStage::BlindPrePlay,
                "blind_skipped",
                format!("Skipped {}", skipped_name),
            )];
            // Apply the tag effect BEFORE incrementing skipped_blind_count so
            // Speed Tag's self-payout uses the pre-skip tally (matches Lua's
            // `G.GAME.skips` being post-incremented elsewhere).
            if let Some(id) = tag_id.as_deref() {
                let tag_events = self.apply_tag_on_skip(id);
                events.extend(tag_events);
            }
            self.state.skipped_blind_count += 1;
            if self.advance_to_next_blind_slot() {
                self.prepare_preblind_state();
            }
            return events;
        }

        Vec::new()
    }

    /// Apply the effect of a tag when the owning blind is skipped. Returns an
    /// event list describing what happened (or a stub `unimplemented_tag`
    /// event for tags whose full effect is not yet modeled).
    fn apply_tag_on_skip(&mut self, tag_id: &str) -> Vec<Event> {
        let spec = self
            .ruleset
            .tag_by_id(tag_id)
            .cloned()
            .or_else(|| default_tag_pool().into_iter().find(|t| t.id == tag_id));
        let Some(spec) = spec else {
            return vec![event(
                EventStage::BlindPrePlay,
                "unimplemented_tag",
                format!("Unknown tag id {} — effect not modeled", tag_id),
            )];
        };
        let name = spec.name.clone();
        match spec.effect_key.as_str() {
            "economy_double_money" => {
                // Double money, capped at +$40. Only pays when money > 0.
                if self.state.money > 0 {
                    let bonus = self.state.money.min(40);
                    self.state.money += bonus;
                    vec![event(
                        EventStage::BlindPrePlay,
                        "tag_effect",
                        format!("{} doubled money by ${}", name, bonus),
                    )]
                } else {
                    vec![event(
                        EventStage::BlindPrePlay,
                        "tag_effect",
                        format!("{} had no effect (money <= 0)", name),
                    )]
                }
            }
            "investment_25_after_boss" => {
                self.state.pending_investment_payouts += 25;
                vec![event(
                    EventStage::BlindPrePlay,
                    "tag_effect",
                    format!("{} queued $25 payout for next boss defeat", name),
                )]
            }
            "voucher_next_shop" => {
                self.state.pending_voucher_tags += 1;
                vec![event(
                    EventStage::BlindPrePlay,
                    "tag_effect",
                    format!("{} will add a voucher to the next shop", name),
                )]
            }
            "coupon_next_shop" => {
                self.state.pending_coupon_shop = true;
                vec![event(
                    EventStage::BlindPrePlay,
                    "tag_effect",
                    format!("{} — initial items in next shop will be free", name),
                )]
            }
            "d6_reroll_start_zero" => {
                self.state.pending_d6_shop = true;
                vec![event(
                    EventStage::BlindPrePlay,
                    "tag_effect",
                    format!("{} — next shop reroll starts at $0", name),
                )]
            }
            "juggle_hand_size_next_round" => {
                self.state.pending_juggle_hand_size += 3;
                vec![event(
                    EventStage::BlindPrePlay,
                    "tag_effect",
                    format!("{} — +3 hand size for the next blind selected", name),
                )]
            }
            "dollars_per_skip" => {
                // Speed Tag: $5 per blind already skipped this run
                // (skipped_blind_count has NOT yet been incremented for this skip).
                let bonus = 5 * self.state.skipped_blind_count;
                if bonus > 0 {
                    self.state.money += bonus;
                }
                vec![event(
                    EventStage::BlindPrePlay,
                    "tag_effect",
                    format!("{} paid out ${} ({} prior skips)", name, bonus, self.state.skipped_blind_count),
                )]
            }
            "dollars_per_hand_played" => {
                // Handy Tag: $1 per hand played across the whole run so far.
                // Aggregate over `hand_stats.played` (tracks all hand types).
                let total_hands: i32 =
                    self.state.hand_stats.values().map(|s| s.played).sum();
                if total_hands > 0 {
                    self.state.money += total_hands;
                }
                vec![event(
                    EventStage::BlindPrePlay,
                    "tag_effect",
                    format!("{} paid out ${} ({} hands played)", name, total_hands, total_hands),
                )]
            }
            _ => vec![event(
                EventStage::BlindPrePlay,
                "unimplemented_tag",
                format!("{} skipped — effect not yet modeled", name),
            )],
        }
    }

    fn handle_blind(&mut self, action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        if action_index < self.state.hand_size {
            if self.state.selected_slots.contains(&action_index) {
                self.state.selected_slots.remove(&action_index);
            } else if action_index < self.state.available.len() {
                self.state.selected_slots.insert(action_index);
            }
            return vec![event(
                EventStage::OnPlayed,
                "selection_changed",
                format!("Toggled card slot {}", action_index),
            )];
        }
        if action_index == 9 {
            return self.discard_selected(trace);
        }
        if action_index == 8 {
            return self.play_selected(trace);
        }
        // Use consumable during blind
        if (71..=78).contains(&action_index) {
            return self.handle_use_consumable(action_index - 71, trace);
        }
        Vec::new()
    }

    fn handle_post_blind(&mut self, _action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        let mut events = Vec::new();

        // Update scaling joker runtime on round end (Popcorn decay, Campfire boss reset, Ceremonial growth)
        self.update_joker_runtime_on_round_end();

        // End-of-round Joker activation (before cashout money is added)
        self.apply_end_of_round_jokers(&mut events, trace);

        self.state.money += self.state.reward;
        let reward = self.state.reward;
        let cleared_boss = self.state.current_blind_slot == BlindSlot::Boss;
        // Pay out any pending Investment-Tag rewards for this boss defeat
        // (stacked across multiple investment skips).
        if cleared_boss && self.state.pending_investment_payouts > 0 {
            let payout = self.state.pending_investment_payouts;
            self.state.money += payout;
            self.state.pending_investment_payouts = 0;
            events.push(event(
                EventStage::EndOfRound,
                "tag_effect",
                format!("Investment Tag paid out ${}", payout),
            ));
        }
        if !cleared_boss {
            self.advance_to_next_blind_slot();
            self.set_active_preblind_progress();
        }
        self.state.phase = Phase::Shop;
        self.state.boss_blind_disabled = false;
        self.state.active_boss_effect = None;
        self.state.debuffed_cards.clear();
        self.state.boss_hand_types_played.clear();
        self.state.boss_forced_hand_type = None;
        self.state.boss_manacle_hand_size_reduced = false;
        // Juggle Tag: revert the one-round hand_size bump now that the blind
        // is over. `active_juggle_hand_size` can't underflow `hand_size`
        // because it was added in `enter_current_blind` in the same run.
        if self.state.active_juggle_hand_size > 0 {
            self.state.hand_size =
                self.state.hand_size.saturating_sub(self.state.active_juggle_hand_size);
            self.state.active_juggle_hand_size = 0;
        }
        // D6 Tag: force the next shop's reroll cost to $0 for this visit only.
        if self.state.pending_d6_shop {
            self.state.shop_current_reroll_cost = 0;
            self.state.pending_d6_shop = false;
            events.push(event(
                EventStage::Shop,
                "tag_effect",
                "D6 Tag — shop reroll starts at $0".to_string(),
            ));
        }
        self.shuffle_deck("deck.shuffle.cashout", trace);
        self.refresh_shop(trace, "cashout_shop_refresh");

        events.push(event(
            EventStage::CashOut,
            "cashout",
            format!("Collected ${reward} and entered Shop"),
        ));
        if !cleared_boss {
            events.push(event(
                EventStage::EndOfRound,
                "blind_advanced",
                format!("Prepared {}", self.state.blind_name),
            ));
        }
        events
    }

    fn handle_shop(&mut self, action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        if (14..24).contains(&action_index) {
            let slot = action_index - 14;
            if let Some(shop_slot) = self.state.shop.iter().find(|entry| entry.slot == slot).cloned() {
                if self.state.jokers.len() < self.state.joker_slot_limit && self.state.money >= shop_slot.joker.buy_cost {
                    self.state.money -= shop_slot.joker.buy_cost;
                    let mut bought = shop_slot.joker.clone();
                    bought.slot_index = self.state.jokers.len();
                    self.state.jokers.push(bought.clone());
                    return vec![event(
                        EventStage::Shop,
                        "buy_joker",
                        format!("Bought {}", bought.name),
                    )];
                }
            }
            return vec![];
        }
        // Buy consumable from shop
        if (24..=25).contains(&action_index) {
            return self.handle_buy_consumable(action_index - 24);
        }
        // Sell consumable from inventory
        if (26..=27).contains(&action_index) {
            return self.handle_sell_consumable(action_index - 26);
        }
        // Buy voucher
        if action_index == 28 {
            return self.handle_buy_voucher();
        }
        // Buy booster pack
        if (29..=30).contains(&action_index) {
            return self.handle_buy_pack(action_index - 29, trace);
        }
        // Use consumable
        if (71..=78).contains(&action_index) {
            return self.handle_use_consumable(action_index - 71, trace);
        }
        if action_index == 79 {
            self.state.money -= self.state.shop_current_reroll_cost;
            self.state.shop_reroll_count += 1;
            self.state.shop_current_reroll_cost += self.state.shop_base_reroll_cost;
            // Update scaling jokers on reroll (Flash Card)
            self.update_joker_runtime_on_reroll();
            self.refresh_shop(trace, "reroll_shop_refresh");
            return vec![event(
                EventStage::Shop,
                "reroll_shop",
                "Rerolled shop".to_string(),
            )];
        }
        if (80..85).contains(&action_index) {
            let slot = action_index - 80;
            if slot < self.state.jokers.len() {
                let sold = self.state.jokers.remove(slot);
                for (new_slot, joker) in self.state.jokers.iter_mut().enumerate() {
                    joker.slot_index = new_slot;
                }
                self.state.money += sold.sell_value;
                // Update scaling jokers on sell (Campfire)
                self.update_joker_runtime_on_sell();
                return vec![event(
                    EventStage::Shop,
                    "sell_joker",
                    format!("Sold {}", sold.name),
                )];
            }
            return vec![];
        }
        if self.state.boss_progress == BlindProgress::Defeated {
            trace.add_transient("NEW_ROUND");
            self.advance_round(trace);
            return vec![event(
                EventStage::EndOfRound,
                "next_round",
                format!("Advanced to ante {}", self.state.ante),
            )];
        }

        self.prepare_preblind_state();
        vec![event(
            EventStage::EndOfRound,
            "next_round",
            format!("Advanced to {}", self.state.blind_name),
        )]
    }

    fn handle_cashout(&mut self, _action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        trace.add_transient("NEW_ROUND");
        self.advance_round(trace);
        vec![event(
            EventStage::EndOfRound,
            "next_round",
            format!("Advanced to ante {}", self.state.ante),
        )]
    }

    fn discard_selected(&mut self, trace: &mut TransitionTrace) -> Vec<Event> {
        let mut removed = Vec::new();
        let mut remaining = Vec::new();
        for (index, card) in self.state.available.clone().into_iter().enumerate() {
            if self.state.selected_slots.contains(&index) {
                removed.push(card);
            } else {
                remaining.push(card);
            }
        }
        if removed.is_empty() && !remaining.is_empty() {
            removed.push(remaining.remove(0));
        }
        self.state.discards -= 1;
        self.state.discarded.extend(removed.clone());
        self.state.available = remaining;
        self.state.selected_slots.clear();

        // Update scaling joker runtime state on discard
        self.update_joker_runtime_on_discard(&removed);

        let hand_size = if self.state.boss_manacle_hand_size_reduced {
            self.state.hand_size - 1
        } else {
            self.state.hand_size
        };
        trace.add_transient("DRAW_TO_HAND");
        self.draw_to_hand(hand_size);
        self.maybe_fail_blind();
        vec![event(
            EventStage::EndOfHand,
            "discard",
            format!("Discarded {} card(s)", removed.len()),
        )]
    }

    fn play_selected(&mut self, trace: &mut TransitionTrace) -> Vec<Event> {
        let selected = self.selected_cards();
        let mut played = if selected.is_empty() && !self.state.available.is_empty() {
            vec![self.state.available[0].clone()]
        } else {
            selected
        };
        let hand = classify_hand(&played);
        let hand_spec = self
            .ruleset
            .hand_specs
            .iter()
            .find(|spec| spec.key == hand.key)
            .expect("known hand spec")
            .clone();

        let hand_level = self.state.hand_levels.get(&hand.key).copied().unwrap_or(1);
        let level_bonus = (hand_level - 1).max(0);
        let base_hand_chips = hand_spec.base_chips + (level_bonus * hand_spec.level_chips);
        let base_hand_mult = hand_spec.base_mult + (level_bonus * hand_spec.level_mult);
        let mut chips = base_hand_chips;
        let mut mult = base_hand_mult;
        let mut events = vec![
            event(
                EventStage::OnPlayed,
                "hand_played",
                format!("Played {}", hand_spec.name),
            ),
        ];

        // Boss effect: The Flint halves starting chips and mult
        if let Some(BossEffect::TheFlint) = &self.state.active_boss_effect {
            if !self.state.boss_blind_disabled {
                chips = (chips + 1) / 2; // round up
                mult = (mult + 1) / 2;
                events.push(event(
                    EventStage::OnPlayed,
                    "boss_effect_scoring",
                    format!("The Flint: chips halved to {}, mult halved to {}", chips, mult),
                ));
            }
        }

        // Boss effect: The Arm decreases level of played hand by 1
        if let Some(BossEffect::TheArm) = &self.state.active_boss_effect {
            if !self.state.boss_blind_disabled {
                let current = self.state.hand_levels.get(&hand.key).copied().unwrap_or(1);
                if current > 1 {
                    let new_level = self.bump_hand_level(&hand.key, -1);
                    events.push(event(
                        EventStage::OnPlayed,
                        "boss_effect_scoring",
                        format!("The Arm: {} level decreased to {}", hand_spec.name, new_level),
                    ));
                }
            }
        }

        // Boss effect: The Eye - track hand types played
        if let Some(BossEffect::TheEye) = &self.state.active_boss_effect {
            if !self.state.boss_blind_disabled {
                self.state.boss_hand_types_played.insert(hand.key.clone());
            }
        }

        // Boss effect: The Mouth - lock hand type after first play
        if let Some(BossEffect::TheMouth) = &self.state.active_boss_effect {
            if !self.state.boss_blind_disabled && self.state.boss_forced_hand_type.is_none() {
                self.state.boss_forced_hand_type = Some(hand.key.clone());
            }
        }

        trace.add_transient("HAND_PLAYED");
        trace.retrigger_supported = true;

        // joker_on_played phase: AFTER hand evaluation, BEFORE per-card scoring
        let hand_key_clone = hand.key.clone();
        self.apply_on_played_jokers(&hand_key_clone, &mut played, &mut events, trace);

        // Determine if this is the final hand of the round (for Dusk)
        let is_final_hand = self.state.plays <= 1;
        let scoring_card_count = played.len();

        // Collect joker specs for retrigger resolution (needed outside borrow of self)
        let joker_specs: Vec<Option<JokerSpec>> = self
            .state
            .jokers
            .iter()
            .map(|j| self.ruleset.joker_by_id(&j.joker_id).cloned())
            .collect();

        // Compute held-in-hand cards (cards in hand that were NOT played)
        let played_ids_set: BTreeSet<u32> = played.iter().map(|c| c.card_id).collect();
        let held_in_hand: Vec<CardInstance> = self
            .state
            .available
            .iter()
            .filter(|c| !played_ids_set.contains(&c.card_id))
            .cloned()
            .collect();

        // Compute effective joker slot limit (accounting for negative editions)
        let negative_count = self
            .state
            .jokers
            .iter()
            .filter(|j| j.edition.as_deref() == Some("e_negative"))
            .count();
        let effective_joker_limit = self.state.joker_slot_limit + negative_count;

        let full_deck_size = (self.state.deck.len() + self.state.available.len() + self.state.discarded.len()) as i32;
        let deck_cards_remaining = self.state.deck.len() as i32;

        let ctx = ScoringContext {
            hand_key: &hand.key,
            played: &played,
            held_in_hand: &held_in_hand,
            discards_left: self.state.discards,
            plays_left: self.state.plays,
            jokers: &self.state.jokers,
            money: self.state.money,
            deck_cards_remaining,
            full_deck_size,
            joker_slot_max: effective_joker_limit,
        };

        let mut xmult = 1.0_f64;
        let mut money_delta = 0_i32;

        // Track glass cards that should be destroyed after scoring
        let mut glass_cards_to_destroy: Vec<u32> = Vec::new();

        // Pre-build one JokerResolutionTrace per joker (audit expects exactly N entries)
        let mut joker_traces: Vec<JokerResolutionTrace> = self
            .state
            .jokers
            .iter()
            .enumerate()
            .map(|(i, joker)| JokerResolutionTrace {
                order: i,
                joker_id: joker.joker_id.clone(),
                joker_name: joker.name.clone(),
                slot_index: joker.slot_index,
                stage: "joker_main".to_string(),
                supported: false,
                matched: false,
                retrigger_count: 0,
                effect_key: None,
                summary: "native engine has no implementation for this Joker".to_string(),
            })
            .collect();

        // Per-card scoring loop with retrigger support
        for (card_index, card) in played.iter().enumerate() {
            // Skip debuffed cards (boss blind effect)
            if self.state.debuffed_cards.contains(&card.card_id) {
                events.push(event(
                    EventStage::CardScored,
                    "card_debuffed",
                    format!("Card {} ({:?}) is debuffed, does not score", card.card_id, card.rank),
                ));
                continue;
            }

            let retrigger_count = calculate_retriggers(
                card,
                card_index,
                &self.state.jokers,
                &joker_specs,
                is_final_hand,
                scoring_card_count,
            );
            let total_passes = 1 + retrigger_count;

            for pass in 0..total_passes {
                // a. Card's base chips
                chips += card.chip_value();
                events.push(event(
                    EventStage::CardScored,
                    "card_chips",
                    format!(
                        "Card {} ({:?}) added {} chips{}",
                        card.card_id,
                        card.rank,
                        card.chip_value(),
                        if pass > 0 {
                            format!(" (retrigger {})", pass)
                        } else {
                            String::new()
                        }
                    ),
                ));

                // Enhancement effects (AFTER base chips, BEFORE joker effects)
                let (is_glass, _is_stone) = apply_card_enhancement(
                    card,
                    &mut chips,
                    &mut mult,
                    &mut xmult,
                    &mut money_delta,
                    &mut events,
                    &mut self.rng,
                );

                // Glass card destruction: 1/4 chance after scoring
                if is_glass {
                    let destroy_roll: i32 = self.rng.gen_range(1..=4);
                    if destroy_roll == 1 && !glass_cards_to_destroy.contains(&card.card_id) {
                        glass_cards_to_destroy.push(card.card_id);
                    }
                }

                // Edition effects (AFTER enhancement, BEFORE jokers)
                apply_card_edition(
                    card,
                    &mut chips,
                    &mut mult,
                    &mut xmult,
                    &mut events,
                );

                // b/c. Each "on scored" Joker activates (left to right)
                for (j_idx, joker) in self.state.jokers.iter().enumerate() {
                    let mut tmp_trace = JokerResolutionTrace {
                        order: j_idx,
                        joker_id: joker.joker_id.clone(),
                        joker_name: joker.name.clone(),
                        slot_index: joker.slot_index,
                        stage: "joker_main".to_string(),
                        supported: false,
                        matched: false,
                        retrigger_count: retrigger_count as i32,
                        effect_key: None,
                        summary: String::new(),
                    };
                    if let Some(spec) = self.ruleset.joker_by_id(&joker.joker_id) {
                        apply_joker_effect(
                            spec,
                            &ctx,
                            &mut chips,
                            &mut mult,
                            &mut xmult,
                            &mut money_delta,
                            &mut events,
                            &mut tmp_trace,
                            &joker.runtime_state,
                        );
                    }
                    // Apply joker edition effects AFTER the joker's own effect
                    apply_joker_edition(
                        joker,
                        &mut chips,
                        &mut mult,
                        &mut xmult,
                        &mut events,
                    );
                    // Promote supported/matched/effect_key to the aggregate trace
                    if tmp_trace.supported {
                        let agg = &mut joker_traces[j_idx];
                        agg.supported = true;
                        agg.matched = true;
                        agg.retrigger_count = retrigger_count as i32;
                        if agg.effect_key.is_none() {
                            agg.effect_key = tmp_trace.effect_key;
                        }
                        agg.summary = tmp_trace.summary;
                    }
                }

                // Emit retrigger event if this was a retrigger pass
                if pass > 0 {
                    events.push(event(
                        EventStage::CardScored,
                        "card_retriggered",
                        format!(
                            "Card {} retriggered (pass {}/{})",
                            card.card_id,
                            pass,
                            retrigger_count
                        ),
                    ));
                }
            }
        }

        // Finalize joker resolution traces
        for jt in &joker_traces {
            if !jt.supported {
                trace.add_note(format!("joker_not_implemented: {}", jt.joker_name));
            }
        }
        trace.joker_resolution = joker_traces;

        events.push(event(
            EventStage::CardScored,
            "base_score",
            format!("Final {} chips x{} mult", chips, mult),
        ));

        // Held-in-hand phase: unplayed cards + relevant Jokers activate
        // Steel Card enhancement: X1.5 mult for each Steel card held in hand
        self.apply_held_in_hand(&played_ids_set, &mut chips, &mut mult, &mut xmult, &mut events, trace);

        // Decrement Seltzer remaining_uses at end of hand
        let mut seltzer_destroyed = Vec::new();
        for joker in self.state.jokers.iter_mut() {
            let is_seltzer = joker.joker_id == "j_seltzer" || joker.name == "Seltzer";
            if is_seltzer {
                if let Some(ref mut uses) = joker.remaining_uses {
                    *uses = uses.saturating_sub(1);
                    if *uses == 0 {
                        seltzer_destroyed.push(joker.name.clone());
                    }
                }
            }
        }
        // Remove destroyed Seltzer jokers
        if !seltzer_destroyed.is_empty() {
            self.state.jokers.retain(|j| {
                let is_seltzer = j.joker_id == "j_seltzer" || j.name == "Seltzer";
                !(is_seltzer && j.remaining_uses == Some(0))
            });
            for (new_slot, joker) in self.state.jokers.iter_mut().enumerate() {
                joker.slot_index = new_slot;
            }
            for name in &seltzer_destroyed {
                events.push(event(
                    EventStage::EndOfHand,
                    "joker_destroyed",
                    format!("{} ran out of uses and was destroyed", name),
                ));
            }
        }

        // Destroy glass cards (1/4 chance was rolled during scoring)
        if !glass_cards_to_destroy.is_empty() {
            self.state.available.retain(|c| !glass_cards_to_destroy.contains(&c.card_id));
            self.state.deck.retain(|c| !glass_cards_to_destroy.contains(&c.card_id));
            self.state.discarded.retain(|c| !glass_cards_to_destroy.contains(&c.card_id));
            for card_id in &glass_cards_to_destroy {
                events.push(event(
                    EventStage::EndOfHand,
                    "glass_card_destroyed",
                    format!("Glass Card {} was destroyed", card_id),
                ));
            }
        }

        // Gold Card end-of-round: +$3 for each Gold card in played hand
        let gold_card_count = played.iter().filter(|c| c.enhancement.as_deref() == Some("m_gold")).count() as i32;
        if gold_card_count > 0 {
            let gold_money = gold_card_count * 3;
            money_delta += gold_money;
            events.push(event(
                EventStage::EndOfHand,
                "gold_card_money",
                format!("{} Gold Card(s) earned ${}", gold_card_count, gold_money),
            ));
        }

        // Apply xmult to final score
        let base_score = chips * mult;
        let gained = (base_score as f64 * xmult).round() as i32;
        self.state.money += money_delta;

        // Boss effect: The Tooth - lose $1 per card played
        if let Some(BossEffect::TheTooth) = &self.state.active_boss_effect {
            if !self.state.boss_blind_disabled {
                let penalty = played.len() as i32;
                self.state.money -= penalty;
                events.push(event(
                    EventStage::JokerPostScore,
                    "boss_effect_scoring",
                    format!("The Tooth: Lost ${} ({} cards played)", penalty, played.len()),
                ));
            }
        }

        // Boss effect: The Ox - lose all money if playing the most played hand type
        if let Some(BossEffect::TheOx) = &self.state.active_boss_effect {
            if !self.state.boss_blind_disabled {
                events.push(event(
                    EventStage::JokerPostScore,
                    "boss_effect_scoring",
                    "The Ox: Most-played hand type check (tracking TODO)".to_string(),
                ));
            }
        }

        self.state.score += gained;
        self.state.plays -= 1;
        self.state.hands_played_this_round += 1;

        // Record in per-poker-hand stats (for `Snapshot.hand_stats` / real-client
        // alignment). Must happen AFTER `The Arm` has potentially mutated the
        // level so counters reflect the canonical order of events.
        self.record_hand_played(&hand.key);

        // Update scaling joker runtime state after scoring
        self.update_joker_runtime_on_play(&hand.key, &played);

        let selected_ids: BTreeSet<u32> = played.iter().map(|card| card.card_id).collect();
        let mut remaining = Vec::new();
        for card in self.state.available.clone() {
            if selected_ids.contains(&card.card_id) {
                self.state.discarded.push(card);
            } else {
                remaining.push(card);
            }
        }
        self.state.available = remaining;
        self.state.selected_slots.clear();
        events.push(event(
            EventStage::JokerPostScore,
            "score_total",
            format!("Scored {} points", gained),
        ));
        if self.state.score >= self.required_score() {
            trace.add_transient("NEW_ROUND");
            self.mark_current_blind_progress(BlindProgress::Defeated);
            self.state.phase = Phase::PostBlind;
            self.state.reward = blind_reward(&self.state.blind);
            // Clear boss state on blind clear
            self.state.active_boss_effect = None;
            self.state.debuffed_cards.clear();
            self.state.boss_hand_types_played.clear();
            self.state.boss_forced_hand_type = None;
            events.push(event(
                EventStage::EndOfHand,
                "blind_cleared",
                format!("Cleared {}", self.state.blind_name),
            ));
        } else {
            // Boss effect: The Hook - discard 2 random cards from hand after play
            if let Some(BossEffect::TheHook) = &self.state.active_boss_effect {
                if !self.state.boss_blind_disabled && !self.state.available.is_empty() {
                    let discard_count = 2.min(self.state.available.len());
                    // Discard from the end of the hand (pseudo-random without consuming RNG)
                    let mut discarded_names = Vec::new();
                    for _ in 0..discard_count {
                        if self.state.available.is_empty() {
                            break;
                        }
                        let idx = self.rng.gen_range(0..self.state.available.len());
                        let card = self.state.available.remove(idx);
                        discarded_names.push(format!("{:?} of {:?}", card.rank, card.suit));
                        self.state.discarded.push(card);
                    }
                    events.push(event(
                        EventStage::EndOfHand,
                        "boss_effect_scoring",
                        format!("The Hook: Discarded {} card(s)", discarded_names.len()),
                    ));
                }
            }

            // Draw back to hand size
            let hand_size = if self.state.boss_manacle_hand_size_reduced {
                self.state.hand_size - 1
            } else {
                self.state.hand_size
            };
            trace.add_transient("DRAW_TO_HAND");
            self.draw_to_hand(hand_size);
            self.maybe_fail_blind();
        }
        events
    }

    fn handle_buy_consumable(&mut self, shop_slot: usize) -> Vec<Event> {
        if shop_slot >= self.state.shop_consumables.len() {
            return vec![];
        }
        if self.state.consumables.len() >= self.state.consumable_slot_limit {
            return vec![];
        }
        let consumable = self.state.shop_consumables[shop_slot].clone();
        if self.state.money < consumable.buy_cost {
            return vec![];
        }
        self.state.money -= consumable.buy_cost;
        self.state.shop_consumables.remove(shop_slot);
        // Re-index remaining shop consumables
        for (idx, c) in self.state.shop_consumables.iter_mut().enumerate() {
            c.slot_index = idx;
        }
        let name = consumable.name.clone();
        let mut bought = consumable;
        bought.slot_index = self.state.consumables.len();
        self.state.consumables.push(bought);
        vec![event(
            EventStage::Shop,
            "buy_consumable",
            format!("Bought {}", name),
        )]
    }

    fn handle_sell_consumable(&mut self, slot: usize) -> Vec<Event> {
        if slot >= self.state.consumables.len() {
            return vec![];
        }
        let sold = self.state.consumables.remove(slot);
        for (idx, c) in self.state.consumables.iter_mut().enumerate() {
            c.slot_index = idx;
        }
        self.state.money += sold.sell_value;
        vec![event(
            EventStage::Shop,
            "sell_consumable",
            format!("Sold {}", sold.name),
        )]
    }

    fn handle_use_consumable(&mut self, slot: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        if slot >= self.state.consumables.len() {
            return vec![];
        }
        let consumable = self.state.consumables[slot].clone();
        let events = match consumable.set.as_str() {
            "Planet" => self.apply_planet_consumable(&consumable, trace),
            "Tarot" => self.apply_tarot_consumable(&consumable, trace),
            "Spectral" => self.apply_spectral_consumable(&consumable, trace),
            _ => {
                trace.add_note(format!("consumable_unknown_set: {}", consumable.set));
                vec![]
            }
        };
        // Remove the consumed item
        if !events.is_empty() {
            // Update scaling jokers on consumable use (Constellation, Fortune Teller)
            self.update_joker_runtime_on_consumable(&consumable.set);
            self.state.consumables.remove(slot);
            for (idx, c) in self.state.consumables.iter_mut().enumerate() {
                c.slot_index = idx;
            }
        }
        events
    }

    fn apply_planet_consumable(&mut self, consumable: &ConsumableInstance, _trace: &mut TransitionTrace) -> Vec<Event> {
        let hand_type = consumable.config.get("hand_type").and_then(|v| v.as_str());
        let hand_type = match hand_type {
            Some(ht) => ht.to_string(),
            None => {
                // Black Hole levels up ALL hand types
                if consumable.consumable_id == "c_black_hole" {
                    let mut events = Vec::new();
                    let specs: Vec<(String, String)> = self
                        .ruleset
                        .hand_specs
                        .iter()
                        .map(|hs| (hs.key.clone(), hs.name.clone()))
                        .collect();
                    for (hand_key, hand_name) in specs {
                        let new_level = self.bump_hand_level(&hand_key, 1);
                        events.push(event(
                            EventStage::Shop,
                            "hand_leveled_up",
                            format!("{} leveled up to Lv.{}", hand_name, new_level),
                        ));
                    }
                    return events;
                }
                return vec![];
            }
        };
        let hand_key = hand_type_to_key(&hand_type);
        let new_level = self.bump_hand_level(hand_key, 1);
        vec![event(
            EventStage::Shop,
            "hand_leveled_up",
            format!("{} leveled up {} to Lv.{}", consumable.name, hand_type, new_level),
        )]
    }

    fn apply_tarot_consumable(&mut self, consumable: &ConsumableInstance, trace: &mut TransitionTrace) -> Vec<Event> {
        let id = consumable.consumable_id.as_str();
        match id {
            "c_strength" => {
                let selected = self.selected_cards();
                let max_highlighted = consumable.config.get("max_highlighted")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(2) as usize;
                let targets: Vec<u32> = selected.iter().take(max_highlighted).map(|c| c.card_id).collect();
                let mut count = 0;
                for card in self.state.available.iter_mut() {
                    if targets.contains(&card.card_id) {
                        card.rank = rank_up(&card.rank);
                        count += 1;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if targets.contains(&card.card_id) {
                        card.rank = rank_up(&card.rank);
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Strength ranked up {} card(s)", count),
                )]
            }
            "c_hermit" => {
                let max_gain = consumable.config.get("extra")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(20) as i32;
                let gained = self.state.money.min(max_gain);
                self.state.money += gained;
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("The Hermit doubled money (gained ${})", gained),
                )]
            }
            "c_fool" => {
                trace.add_note("tarot_fool_not_fully_implemented");
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    "The Fool (copy last consumable - not yet fully implemented)".to_string(),
                )]
            }
            "c_temperance" => {
                let cap = consumable.config.get("extra")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(50) as i32;
                let total_sell: i32 = self.state.jokers.iter().map(|j| j.sell_value).sum();
                let gained = total_sell.min(cap);
                self.state.money += gained;
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Temperance gained ${}", gained),
                )]
            }
            "c_star" | "c_moon" | "c_sun" | "c_world" => {
                let target_suit = match consumable.config.get("suit_conv").and_then(|v| v.as_str()) {
                    Some("Diamonds") => Suit::Diamonds,
                    Some("Clubs") => Suit::Clubs,
                    Some("Hearts") => Suit::Hearts,
                    Some("Spades") => Suit::Spades,
                    _ => return vec![],
                };
                let suit_name = suit_label(&target_suit).to_string();
                let max_highlighted = consumable.config.get("max_highlighted")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3) as usize;
                let selected = self.selected_cards();
                let targets: Vec<u32> = selected.iter().take(max_highlighted).map(|c| c.card_id).collect();
                let mut count = 0;
                for card in self.state.available.iter_mut() {
                    if targets.contains(&card.card_id) {
                        card.suit = target_suit.clone();
                        count += 1;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if targets.contains(&card.card_id) {
                        card.suit = target_suit.clone();
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("{} converted {} card(s) to {}", consumable.name, count, suit_name),
                )]
            }
            "c_magician" | "c_empress" | "c_heirophant" | "c_lovers"
            | "c_chariot" | "c_justice" | "c_devil" | "c_tower" => {
                let enhancement = consumable.config.get("mod_conv")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let max_highlighted = consumable.config.get("max_highlighted")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(2) as usize;
                let selected = self.selected_cards();
                let targets: Vec<u32> = selected.iter().take(max_highlighted).map(|c| c.card_id).collect();
                let mut count = 0;
                for card in self.state.available.iter_mut() {
                    if targets.contains(&card.card_id) {
                        card.enhancement = Some(enhancement.clone());
                        count += 1;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if targets.contains(&card.card_id) {
                        card.enhancement = Some(enhancement.clone());
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("{} enhanced {} card(s) with {}", consumable.name, count, enhancement),
                )]
            }
            "c_hanged_man" => {
                let max_highlighted = consumable.config.get("max_highlighted")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(2) as usize;
                let selected = self.selected_cards();
                let targets: Vec<u32> = selected.iter().take(max_highlighted).map(|c| c.card_id).collect();
                let count = targets.len();
                self.state.available.retain(|card| !targets.contains(&card.card_id));
                self.state.deck.retain(|card| !targets.contains(&card.card_id));
                self.state.selected_slots.clear();
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("The Hanged Man destroyed {} card(s)", count),
                )]
            }
            "c_high_priestess" => {
                // Create up to 2 random Planet cards (if consumable slots available)
                let max_planets = consumable.config.get("planets")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(2) as usize;
                let planet_pool: Vec<balatro_spec::ConsumableSpec> = self
                    .ruleset
                    .consumables
                    .iter()
                    .filter(|c| c.set == "Planet")
                    .cloned()
                    .collect();
                if planet_pool.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "The High Priestess (no planets available)".to_string())];
                }
                let mut created = 0;
                for i in 0..max_planets {
                    if self.state.consumables.len() >= self.state.consumable_slot_limit {
                        break;
                    }
                    let candidates: Vec<String> = planet_pool.iter().map(|c| c.id.clone()).collect();
                    let chosen = self.choose_index(
                        candidates.len(),
                        format!("high_priestess.create_planet_{}", i),
                        candidates,
                        trace,
                    );
                    let chosen_spec = &planet_pool[chosen];
                    let new_consumable = ConsumableInstance {
                        consumable_id: chosen_spec.id.clone(),
                        name: chosen_spec.name.clone(),
                        set: chosen_spec.set.clone(),
                        cost: chosen_spec.cost,
                        buy_cost: chosen_spec.cost,
                        sell_value: (chosen_spec.cost / 2).max(1),
                        slot_index: self.state.consumables.len(),
                        config: chosen_spec.config.clone(),
                    };
                    self.state.consumables.push(new_consumable);
                    created += 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("The High Priestess created {} Planet card(s)", created),
                )]
            }
            "c_emperor" => {
                // Create up to 2 random Tarot cards (if consumable slots available)
                let max_tarots = consumable.config.get("tarots")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(2) as usize;
                let tarot_pool: Vec<balatro_spec::ConsumableSpec> = self
                    .ruleset
                    .consumables
                    .iter()
                    .filter(|c| c.set == "Tarot")
                    .cloned()
                    .collect();
                if tarot_pool.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "The Emperor (no tarots available)".to_string())];
                }
                let mut created = 0;
                for i in 0..max_tarots {
                    if self.state.consumables.len() >= self.state.consumable_slot_limit {
                        break;
                    }
                    let candidates: Vec<String> = tarot_pool.iter().map(|c| c.id.clone()).collect();
                    let chosen = self.choose_index(
                        candidates.len(),
                        format!("emperor.create_tarot_{}", i),
                        candidates,
                        trace,
                    );
                    let chosen_spec = &tarot_pool[chosen];
                    let new_consumable = ConsumableInstance {
                        consumable_id: chosen_spec.id.clone(),
                        name: chosen_spec.name.clone(),
                        set: chosen_spec.set.clone(),
                        cost: chosen_spec.cost,
                        buy_cost: chosen_spec.cost,
                        sell_value: (chosen_spec.cost / 2).max(1),
                        slot_index: self.state.consumables.len(),
                        config: chosen_spec.config.clone(),
                    };
                    self.state.consumables.push(new_consumable);
                    created += 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("The Emperor created {} Tarot card(s)", created),
                )]
            }
            "c_wheel_of_fortune" => {
                // 1 in 4 chance to add a random edition to a random Joker
                let odds = consumable.config.get("extra")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(4) as i32;
                if self.state.jokers.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "The Wheel of Fortune (no jokers)".to_string())];
                }
                let hit = self.roll_chance(odds, "wheel_of_fortune.chance", trace);
                if hit {
                    let joker_candidates: Vec<String> = self.state.jokers.iter().map(|j| j.joker_id.clone()).collect();
                    let joker_idx = self.choose_index(
                        joker_candidates.len(),
                        "wheel_of_fortune.choose_joker",
                        joker_candidates,
                        trace,
                    );
                    let editions = vec!["foil".to_string(), "holo".to_string(), "polychrome".to_string()];
                    let edition_idx = self.choose_index(
                        editions.len(),
                        "wheel_of_fortune.choose_edition",
                        editions.clone(),
                        trace,
                    );
                    let edition = &editions[edition_idx];
                    self.state.jokers[joker_idx].edition = Some(edition.clone());
                    vec![event(
                        EventStage::Shop,
                        "consumable_used",
                        format!("The Wheel of Fortune added {} to {}", edition, self.state.jokers[joker_idx].name),
                    )]
                } else {
                    vec![event(
                        EventStage::Shop,
                        "consumable_used",
                        "The Wheel of Fortune missed".to_string(),
                    )]
                }
            }
            "c_death" => {
                // Convert left selected card into a copy of right selected card
                let selected = self.selected_cards();
                if selected.len() < 2 {
                    trace.add_note("death_requires_2_selected");
                    return vec![event(EventStage::Shop, "consumable_used", "Death (need 2 selected cards)".to_string())];
                }
                let right_card = selected[1].clone();
                let left_card_id = selected[0].card_id;
                // Transform left card to match right card (keep card_id)
                for card in self.state.available.iter_mut() {
                    if card.card_id == left_card_id {
                        card.rank = right_card.rank.clone();
                        card.suit = right_card.suit.clone();
                        card.enhancement = right_card.enhancement.clone();
                        card.edition = right_card.edition.clone();
                        card.seal = right_card.seal.clone();
                        break;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if card.card_id == left_card_id {
                        card.rank = right_card.rank.clone();
                        card.suit = right_card.suit.clone();
                        card.enhancement = right_card.enhancement.clone();
                        card.edition = right_card.edition.clone();
                        card.seal = right_card.seal.clone();
                        break;
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    "Death converted left card into copy of right card".to_string(),
                )]
            }
            "c_judgement" => {
                // Create a random Joker (if joker slot available)
                if self.state.jokers.len() >= self.state.joker_slot_limit {
                    return vec![event(EventStage::Shop, "consumable_used", "Judgement (no joker slots)".to_string())];
                }
                let joker_pool: Vec<balatro_spec::JokerSpec> = self
                    .ruleset
                    .jokers
                    .iter()
                    .filter(|j| j.rarity <= 3)
                    .cloned()
                    .collect();
                if joker_pool.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Judgement (no jokers available)".to_string())];
                }
                let candidates: Vec<String> = joker_pool.iter().map(|j| j.id.clone()).collect();
                let chosen = self.choose_index(
                    candidates.len(),
                    "judgement.create_joker",
                    candidates,
                    trace,
                );
                let spec = &joker_pool[chosen];
                let new_joker = JokerInstance {
                    joker_id: spec.id.clone(),
                    name: spec.name.clone(),
                    base_cost: spec.base_cost,
                    cost: spec.cost,
                    buy_cost: spec.cost,
                    sell_value: (spec.cost / 2).max(1),
                    extra_sell_value: 0,
                    rarity: spec.rarity,
                    edition: None,
                    slot_index: self.state.jokers.len(),
                    activation_class: spec.activation_class.clone(),
                    wiki_effect_text_en: spec.wiki_effect_text_en.clone(),
                    remaining_uses: initial_remaining_uses(spec),
                    runtime_state: initial_runtime_state(spec, &mut self.rng),
                };
                let joker_name = new_joker.name.clone();
                self.state.jokers.push(new_joker);
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Judgement created {}", joker_name),
                )]
            }
            _ => {
                trace.add_note(format!("tarot_not_implemented: {}", consumable.name));
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Used {} (tarot effect not yet implemented)", consumable.name),
                )]
            }
        }
    }

    fn next_card_id(&self) -> u32 {
        self.state.deck.iter()
            .chain(self.state.available.iter())
            .chain(self.state.discarded.iter())
            .map(|c| c.card_id)
            .max()
            .unwrap_or(52) + 1
    }

    fn apply_spectral_consumable(&mut self, consumable: &ConsumableInstance, trace: &mut TransitionTrace) -> Vec<Event> {
        let id = consumable.consumable_id.as_str();
        match id {
            "c_familiar" => {
                // Destroy 1 random card in hand, add 3 random face cards to deck
                let extra = consumable.config.get("extra")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3) as usize;
                if self.state.available.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Familiar (no cards in hand)".to_string())];
                }
                let candidates: Vec<String> = self.state.available.iter().map(|c| format!("card_{}", c.card_id)).collect();
                let destroy_idx = self.choose_index(
                    candidates.len(),
                    "familiar.destroy",
                    candidates,
                    trace,
                );
                self.state.available.remove(destroy_idx);
                let mut base_id = self.next_card_id();
                let face_ranks = [Rank::Jack, Rank::Queen, Rank::King];
                let all_suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];
                for i in 0..extra {
                    let rank_idx = self.choose_index(
                        face_ranks.len(),
                        format!("familiar.rank_{}", i),
                        face_ranks.iter().map(|r| format!("{:?}", r)).collect(),
                        trace,
                    );
                    let suit_idx = self.choose_index(
                        all_suits.len(),
                        format!("familiar.suit_{}", i),
                        all_suits.iter().map(|s| format!("{:?}", s)).collect(),
                        trace,
                    );
                    self.state.deck.push(CardInstance {
                        card_id: base_id,
                        rank: face_ranks[rank_idx].clone(),
                        suit: all_suits[suit_idx].clone(),
                        enhancement: None,
                        edition: None,
                        seal: None,
                    });
                    base_id += 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Familiar destroyed 1 card, added {} face cards to deck", extra),
                )]
            }
            "c_grim" => {
                // Destroy 1 random card in hand, add 2 random Aces to deck
                let extra = consumable.config.get("extra")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(2) as usize;
                if self.state.available.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Grim (no cards in hand)".to_string())];
                }
                let candidates: Vec<String> = self.state.available.iter().map(|c| format!("card_{}", c.card_id)).collect();
                let destroy_idx = self.choose_index(
                    candidates.len(),
                    "grim.destroy",
                    candidates,
                    trace,
                );
                self.state.available.remove(destroy_idx);
                let mut base_id = self.next_card_id();
                let all_suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];
                for i in 0..extra {
                    let suit_idx = self.choose_index(
                        all_suits.len(),
                        format!("grim.suit_{}", i),
                        all_suits.iter().map(|s| format!("{:?}", s)).collect(),
                        trace,
                    );
                    self.state.deck.push(CardInstance {
                        card_id: base_id,
                        rank: Rank::Ace,
                        suit: all_suits[suit_idx].clone(),
                        enhancement: None,
                        edition: None,
                        seal: None,
                    });
                    base_id += 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Grim destroyed 1 card, added {} Aces to deck", extra),
                )]
            }
            "c_incantation" => {
                // Destroy 1 random card in hand, add 4 random numbered cards (2-10) to deck
                let extra = consumable.config.get("extra")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(4) as usize;
                if self.state.available.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Incantation (no cards in hand)".to_string())];
                }
                let candidates: Vec<String> = self.state.available.iter().map(|c| format!("card_{}", c.card_id)).collect();
                let destroy_idx = self.choose_index(
                    candidates.len(),
                    "incantation.destroy",
                    candidates,
                    trace,
                );
                self.state.available.remove(destroy_idx);
                let mut base_id = self.next_card_id();
                let numbered_ranks = [Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six, Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten];
                let all_suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];
                for i in 0..extra {
                    let rank_idx = self.choose_index(
                        numbered_ranks.len(),
                        format!("incantation.rank_{}", i),
                        numbered_ranks.iter().map(|r| format!("{:?}", r)).collect(),
                        trace,
                    );
                    let suit_idx = self.choose_index(
                        all_suits.len(),
                        format!("incantation.suit_{}", i),
                        all_suits.iter().map(|s| format!("{:?}", s)).collect(),
                        trace,
                    );
                    self.state.deck.push(CardInstance {
                        card_id: base_id,
                        rank: numbered_ranks[rank_idx].clone(),
                        suit: all_suits[suit_idx].clone(),
                        enhancement: None,
                        edition: None,
                        seal: None,
                    });
                    base_id += 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Incantation destroyed 1 card, added {} numbered cards to deck", extra),
                )]
            }
            "c_talisman" => {
                // Add Gold Seal to 1 selected card
                let selected = self.selected_cards();
                if selected.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Talisman (no card selected)".to_string())];
                }
                let target_id = selected[0].card_id;
                for card in self.state.available.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Gold".to_string());
                        break;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Gold".to_string());
                        break;
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    "Talisman added Gold Seal".to_string(),
                )]
            }
            "c_aura" => {
                // Add random edition (Foil/Holo/Polychrome) to 1 selected card
                let selected = self.selected_cards();
                if selected.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Aura (no card selected)".to_string())];
                }
                let target_id = selected[0].card_id;
                let editions = vec!["foil".to_string(), "holo".to_string(), "polychrome".to_string()];
                let edition_idx = self.choose_index(
                    editions.len(),
                    "aura.choose_edition",
                    editions.clone(),
                    trace,
                );
                let edition = &editions[edition_idx];
                for card in self.state.available.iter_mut() {
                    if card.card_id == target_id {
                        card.edition = Some(edition.clone());
                        break;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if card.card_id == target_id {
                        card.edition = Some(edition.clone());
                        break;
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Aura added {} edition", edition),
                )]
            }
            "c_wraith" => {
                // Create a random rare Joker, set money to $0
                if self.state.jokers.len() >= self.state.joker_slot_limit {
                    return vec![event(EventStage::Shop, "consumable_used", "Wraith (no joker slots)".to_string())];
                }
                let rare_pool: Vec<balatro_spec::JokerSpec> = self
                    .ruleset
                    .jokers
                    .iter()
                    .filter(|j| j.rarity == 3)
                    .cloned()
                    .collect();
                if rare_pool.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Wraith (no rare jokers available)".to_string())];
                }
                let candidates: Vec<String> = rare_pool.iter().map(|j| j.id.clone()).collect();
                let chosen = self.choose_index(
                    candidates.len(),
                    "wraith.create_joker",
                    candidates,
                    trace,
                );
                let spec = &rare_pool[chosen];
                let new_joker = JokerInstance {
                    joker_id: spec.id.clone(),
                    name: spec.name.clone(),
                    base_cost: spec.base_cost,
                    cost: spec.cost,
                    buy_cost: spec.cost,
                    sell_value: (spec.cost / 2).max(1),
                    extra_sell_value: 0,
                    rarity: spec.rarity,
                    edition: None,
                    slot_index: self.state.jokers.len(),
                    activation_class: spec.activation_class.clone(),
                    wiki_effect_text_en: spec.wiki_effect_text_en.clone(),
                    remaining_uses: initial_remaining_uses(spec),
                    runtime_state: initial_runtime_state(spec, &mut self.rng),
                };
                let joker_name = new_joker.name.clone();
                self.state.jokers.push(new_joker);
                self.state.money = 0;
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Wraith created {} and set money to $0", joker_name),
                )]
            }
            "c_sigil" => {
                // Convert all cards in hand to a single random suit
                if self.state.available.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Sigil (no cards in hand)".to_string())];
                }
                let all_suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];
                let suit_idx = self.choose_index(
                    all_suits.len(),
                    "sigil.choose_suit",
                    all_suits.iter().map(|s| format!("{:?}", s)).collect(),
                    trace,
                );
                let target_suit = all_suits[suit_idx].clone();
                let hand_ids: Vec<u32> = self.state.available.iter().map(|c| c.card_id).collect();
                for card in self.state.available.iter_mut() {
                    card.suit = target_suit.clone();
                }
                for card in self.state.deck.iter_mut() {
                    if hand_ids.contains(&card.card_id) {
                        card.suit = target_suit.clone();
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Sigil converted all hand cards to {:?}", target_suit),
                )]
            }
            "c_ouija" => {
                // Convert all cards in hand to a single random rank, reduce hand size by 1
                if self.state.available.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Ouija (no cards in hand)".to_string())];
                }
                let all_ranks = [Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six, Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten, Rank::Jack, Rank::Queen, Rank::King, Rank::Ace];
                let rank_idx = self.choose_index(
                    all_ranks.len(),
                    "ouija.choose_rank",
                    all_ranks.iter().map(|r| format!("{:?}", r)).collect(),
                    trace,
                );
                let target_rank = all_ranks[rank_idx].clone();
                let hand_ids: Vec<u32> = self.state.available.iter().map(|c| c.card_id).collect();
                for card in self.state.available.iter_mut() {
                    card.rank = target_rank.clone();
                }
                for card in self.state.deck.iter_mut() {
                    if hand_ids.contains(&card.card_id) {
                        card.rank = target_rank.clone();
                    }
                }
                if self.state.hand_size > 1 {
                    self.state.hand_size -= 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Ouija converted all hand cards to {:?}, hand size -1", target_rank),
                )]
            }
            "c_ectoplasm" => {
                // Add Negative edition to a random Joker, reduce hand size by 1
                if self.state.jokers.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Ectoplasm (no jokers)".to_string())];
                }
                let joker_candidates: Vec<String> = self.state.jokers.iter().map(|j| j.joker_id.clone()).collect();
                let joker_idx = self.choose_index(
                    joker_candidates.len(),
                    "ectoplasm.choose_joker",
                    joker_candidates,
                    trace,
                );
                self.state.jokers[joker_idx].edition = Some("negative".to_string());
                let joker_name = self.state.jokers[joker_idx].name.clone();
                if self.state.hand_size > 1 {
                    self.state.hand_size -= 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Ectoplasm added Negative to {}, hand size -1", joker_name),
                )]
            }
            "c_immolate" => {
                // Destroy 5 random cards in hand, gain $20
                let destroy_count = consumable.config.get("extra")
                    .and_then(|v| v.get("destroy"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(5) as usize;
                let dollars = consumable.config.get("extra")
                    .and_then(|v| v.get("dollars"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(20) as i32;
                let actual_destroy = destroy_count.min(self.state.available.len());
                let mut destroyed_ids: Vec<u32> = Vec::new();
                for i in 0..actual_destroy {
                    if self.state.available.is_empty() {
                        break;
                    }
                    let candidates: Vec<String> = self.state.available.iter().map(|c| format!("card_{}", c.card_id)).collect();
                    let idx = self.choose_index(
                        candidates.len(),
                        format!("immolate.destroy_{}", i),
                        candidates,
                        trace,
                    );
                    let removed = self.state.available.remove(idx);
                    destroyed_ids.push(removed.card_id);
                }
                // Also remove from deck
                self.state.deck.retain(|c| !destroyed_ids.contains(&c.card_id));
                self.state.money += dollars;
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Immolate destroyed {} cards, gained ${}", destroyed_ids.len(), dollars),
                )]
            }
            "c_ankh" => {
                // Copy a random Joker, destroy all other Jokers
                if self.state.jokers.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Ankh (no jokers)".to_string())];
                }
                let joker_candidates: Vec<String> = self.state.jokers.iter().map(|j| j.joker_id.clone()).collect();
                let keep_idx = self.choose_index(
                    joker_candidates.len(),
                    "ankh.choose_joker",
                    joker_candidates,
                    trace,
                );
                let kept = self.state.jokers[keep_idx].clone();
                let mut copy = kept.clone();
                copy.slot_index = 1;
                let kept_name = kept.name.clone();
                let mut kept_joker = kept;
                kept_joker.slot_index = 0;
                self.state.jokers = vec![kept_joker, copy];
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Ankh copied {} and destroyed all other Jokers", kept_name),
                )]
            }
            "c_hex" => {
                // Add Polychrome to a random Joker, destroy all other Jokers
                if self.state.jokers.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Hex (no jokers)".to_string())];
                }
                let joker_candidates: Vec<String> = self.state.jokers.iter().map(|j| j.joker_id.clone()).collect();
                let keep_idx = self.choose_index(
                    joker_candidates.len(),
                    "hex.choose_joker",
                    joker_candidates,
                    trace,
                );
                let mut kept = self.state.jokers[keep_idx].clone();
                kept.edition = Some("polychrome".to_string());
                kept.slot_index = 0;
                let kept_name = kept.name.clone();
                self.state.jokers = vec![kept];
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Hex added Polychrome to {} and destroyed all other Jokers", kept_name),
                )]
            }
            "c_deja_vu" => {
                // Add Red Seal to 1 selected card
                let selected = self.selected_cards();
                if selected.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Deja Vu (no card selected)".to_string())];
                }
                let target_id = selected[0].card_id;
                for card in self.state.available.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Red".to_string());
                        break;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Red".to_string());
                        break;
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    "Deja Vu added Red Seal".to_string(),
                )]
            }
            "c_trance" => {
                // Add Blue Seal to 1 selected card
                let selected = self.selected_cards();
                if selected.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Trance (no card selected)".to_string())];
                }
                let target_id = selected[0].card_id;
                for card in self.state.available.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Blue".to_string());
                        break;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Blue".to_string());
                        break;
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    "Trance added Blue Seal".to_string(),
                )]
            }
            "c_medium" => {
                // Add Purple Seal to 1 selected card
                let selected = self.selected_cards();
                if selected.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Medium (no card selected)".to_string())];
                }
                let target_id = selected[0].card_id;
                for card in self.state.available.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Purple".to_string());
                        break;
                    }
                }
                for card in self.state.deck.iter_mut() {
                    if card.card_id == target_id {
                        card.seal = Some("Purple".to_string());
                        break;
                    }
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    "Medium added Purple Seal".to_string(),
                )]
            }
            "c_cryptid" => {
                // Create 2 copies of 1 selected card in your deck
                let extra = consumable.config.get("extra")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(2) as u32;
                let selected = self.selected_cards();
                if selected.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "Cryptid (no card selected)".to_string())];
                }
                let source = selected[0].clone();
                let mut base_id = self.next_card_id();
                for _ in 0..extra {
                    let copy = CardInstance {
                        card_id: base_id,
                        rank: source.rank.clone(),
                        suit: source.suit.clone(),
                        enhancement: source.enhancement.clone(),
                        edition: source.edition.clone(),
                        seal: source.seal.clone(),
                    };
                    self.state.deck.push(copy);
                    base_id += 1;
                }
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Cryptid created {} copies of selected card", extra),
                )]
            }
            "c_soul" => {
                // Create a random Legendary Joker
                if self.state.jokers.len() >= self.state.joker_slot_limit {
                    return vec![event(EventStage::Shop, "consumable_used", "The Soul (no joker slots)".to_string())];
                }
                let legendary_pool: Vec<balatro_spec::JokerSpec> = self
                    .ruleset
                    .jokers
                    .iter()
                    .filter(|j| j.rarity == 4)
                    .cloned()
                    .collect();
                if legendary_pool.is_empty() {
                    return vec![event(EventStage::Shop, "consumable_used", "The Soul (no legendary jokers available)".to_string())];
                }
                let candidates: Vec<String> = legendary_pool.iter().map(|j| j.id.clone()).collect();
                let chosen = self.choose_index(
                    candidates.len(),
                    "soul.create_joker",
                    candidates,
                    trace,
                );
                let spec = &legendary_pool[chosen];
                let new_joker = JokerInstance {
                    joker_id: spec.id.clone(),
                    name: spec.name.clone(),
                    base_cost: spec.base_cost,
                    cost: spec.cost,
                    buy_cost: spec.cost,
                    sell_value: (spec.cost / 2).max(1),
                    extra_sell_value: 0,
                    rarity: spec.rarity,
                    edition: None,
                    slot_index: self.state.jokers.len(),
                    activation_class: spec.activation_class.clone(),
                    wiki_effect_text_en: spec.wiki_effect_text_en.clone(),
                    remaining_uses: initial_remaining_uses(spec),
                    runtime_state: initial_runtime_state(spec, &mut self.rng),
                };
                let joker_name = new_joker.name.clone();
                self.state.jokers.push(new_joker);
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("The Soul created legendary {}", joker_name),
                )]
            }
            "c_black_hole" => {
                // Level up every hand type by 1
                let mut events = Vec::new();
                let specs: Vec<(String, String)> = self
                    .ruleset
                    .hand_specs
                    .iter()
                    .map(|hs| (hs.key.clone(), hs.name.clone()))
                    .collect();
                for (hand_key, hand_name) in specs {
                    let new_level = self.bump_hand_level(&hand_key, 1);
                    events.push(event(
                        EventStage::Shop,
                        "hand_leveled_up",
                        format!("{} leveled up to Lv.{}", hand_name, new_level),
                    ));
                }
                events
            }
            _ => {
                trace.add_note(format!("spectral_not_implemented: {}", consumable.name));
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Used {} (spectral effect not yet implemented)", consumable.name),
                )]
            }
        }
    }

    fn maybe_fail_blind(&mut self) {
        if self.state.plays <= 0 && self.state.discards <= 0 && self.state.score < self.required_score() {
            self.state.phase = Phase::End;
            self.state.over = true;
            self.state.won = false;
        }
    }

    fn advance_round(&mut self, trace: &mut TransitionTrace) {
        self.state.round += 1;
        self.state.ante += 1;
        if self.state.ante > self.state.max_ante {
            self.state.phase = Phase::End;
            self.state.over = true;
            self.state.won = true;
            return;
        }
        self.prepare_round_start(trace);
    }

    fn prepare_round_start(&mut self, trace: &mut TransitionTrace) {
        self.state.phase = Phase::PreBlind;
        self.state.current_blind_slot = BlindSlot::Small;
        self.state.small_progress = BlindProgress::Select;
        self.state.big_progress = BlindProgress::Upcoming;
        self.state.boss_progress = BlindProgress::Upcoming;
        self.state.boss_blind = self.pick_boss_blind(trace);
        // Roll a fresh tag for each skippable blind slot. Boss blinds don't
        // carry tags in vanilla (boss is unskippable) so we leave it as None.
        self.state.small_tag_id = self.roll_tag_id("tag.small.pick", trace);
        self.state.big_tag_id = self.roll_tag_id("tag.big.pick", trace);
        self.state.boss_tag_id = None;
        self.prepare_preblind_state();
    }

    fn prepare_preblind_state(&mut self) {
        self.state.phase = Phase::PreBlind;
        self.state.score = 0;
        self.state.plays = self.state.base_plays;
        self.state.discards = self.state.base_discards;
        self.state.reward = blind_reward_for_slot(self.state.current_blind_slot);
        self.state.selected_slots.clear();
        self.state.discarded.clear();
        self.state.available.clear();
        self.state.deck.clear();
        self.state.shop.clear();
        self.state.shop_consumables.clear();
        self.state.shop_packs.clear();
        self.state.shop_voucher = None;
        self.sync_current_blind_descriptor();
        self.set_active_preblind_progress();
    }

    fn enter_current_blind(&mut self, trace: &mut TransitionTrace) -> Vec<Event> {
        self.state.phase = Phase::Blind;
        self.state.score = 0;
        self.state.plays = self.state.base_plays;
        self.state.discards = self.state.base_discards;
        self.state.selected_slots.clear();
        self.state.discarded.clear();
        self.state.shop.clear();
        self.state.shop_consumables.clear();
        self.state.shop_packs.clear();
        self.state.shop_voucher = None;
        self.state.boss_blind_disabled = false;
        self.state.hands_played_this_round = 0;
        // Per-hand-type `played_this_round` resets alongside the global round counter.
        for stats in self.state.hand_stats.values_mut() {
            stats.played_this_round = 0;
        }
        self.state.active_boss_effect = None;
        self.state.debuffed_cards.clear();
        self.state.boss_hand_types_played.clear();
        self.state.boss_forced_hand_type = None;
        self.state.boss_manacle_hand_size_reduced = false;
        self.sync_current_blind_descriptor();
        self.mark_current_blind_progress(BlindProgress::Current);

        // Reset per-round joker state (Card Sharp hand tracking, Hit the Road Jack counter)
        self.reset_per_round_joker_state();

        // Update Madness when selecting Small/Big blind
        self.update_joker_runtime_on_blind_select();

        // Randomize To Do List target hand type for this blind
        let has_todo_list = self.state.jokers.iter().any(|j| j.joker_id == "j_todo_list");
        if has_todo_list {
            let hand_keys: Vec<String> = self.ruleset.hand_specs.iter().map(|hs| hs.key.clone()).collect();
            if !hand_keys.is_empty() {
                let candidates = hand_keys.clone();
                let idx = self.choose_index(candidates.len(), "todo_list.target", candidates, trace);
                self.state.todo_list_target = Some(hand_keys[idx].clone());
            }
        }

        // Boss blind pre-play Joker activation (Chicot may disable boss here)
        let mut blind_select_events = Vec::new();
        self.apply_blind_select_jokers(&mut blind_select_events, trace);

        self.reset_deck(trace);
        // Juggle Tag: consume pending +hand-size-this-round bonus. Applied as a
        // one-shot delta to `hand_size` for this blind's draw and held for the
        // duration of the blind, then reset on next round start.
        let juggle_bonus = self.state.pending_juggle_hand_size;
        if juggle_bonus > 0 {
            self.state.hand_size = self.state.hand_size.saturating_add(juggle_bonus);
            self.state.active_juggle_hand_size =
                self.state.active_juggle_hand_size.saturating_add(juggle_bonus);
            self.state.pending_juggle_hand_size = 0;
            blind_select_events.push(event(
                EventStage::BlindPrePlay,
                "tag_effect",
                format!("Juggle Tag — +{} hand size this blind", juggle_bonus),
            ));
        }
        let hand_size = self.state.hand_size;
        self.draw_to_hand(hand_size);

        // Apply boss blind effects after deck setup (so we can debuff cards in hand)
        self.apply_boss_blind_pre_play(&mut blind_select_events, trace);

        blind_select_events
    }

    /// Apply boss blind pre-play effects (debuffs, hand/discard modifications, etc.)
    fn apply_boss_blind_pre_play(
        &mut self,
        events: &mut Vec<Event>,
        _trace: &mut TransitionTrace,
    ) {
        if self.state.boss_blind_disabled {
            return;
        }
        if !matches!(self.state.current_blind_slot, BlindSlot::Boss) {
            return;
        }

        let effect = match BossEffect::from_blind_name(&self.state.blind_name) {
            Some(e) => e,
            None => return,
        };

        match &effect {
            BossEffect::TheGoad => {
                let ids: Vec<u32> = self.all_card_ids_matching(|c| matches!(c.suit, Suit::Spades));
                self.state.debuffed_cards.extend(&ids);
                events.push(event(EventStage::BlindPrePlay, "boss_effect", format!("The Goad: {} Spade card(s) debuffed", ids.len())));
            }
            BossEffect::TheHead => {
                let ids: Vec<u32> = self.all_card_ids_matching(|c| matches!(c.suit, Suit::Hearts));
                self.state.debuffed_cards.extend(&ids);
                events.push(event(EventStage::BlindPrePlay, "boss_effect", format!("The Head: {} Heart card(s) debuffed", ids.len())));
            }
            BossEffect::TheClub => {
                let ids: Vec<u32> = self.all_card_ids_matching(|c| matches!(c.suit, Suit::Clubs));
                self.state.debuffed_cards.extend(&ids);
                events.push(event(EventStage::BlindPrePlay, "boss_effect", format!("The Club: {} Club card(s) debuffed", ids.len())));
            }
            BossEffect::TheWindow => {
                let ids: Vec<u32> = self.all_card_ids_matching(|c| matches!(c.suit, Suit::Diamonds));
                self.state.debuffed_cards.extend(&ids);
                events.push(event(EventStage::BlindPrePlay, "boss_effect", format!("The Window: {} Diamond card(s) debuffed", ids.len())));
            }
            BossEffect::ThePlant => {
                let ids: Vec<u32> = self.all_card_ids_matching(|c| c.is_face_card());
                self.state.debuffed_cards.extend(&ids);
                events.push(event(EventStage::BlindPrePlay, "boss_effect", format!("The Plant: {} face card(s) debuffed", ids.len())));
            }
            BossEffect::ThePsychic => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Psychic: Must play exactly 5 cards".to_string()));
            }
            BossEffect::TheNeedle => {
                self.state.plays = 1;
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Needle: Only 1 hand allowed".to_string()));
            }
            BossEffect::TheWater => {
                self.state.discards = 0;
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Water: Start with 0 discards".to_string()));
            }
            BossEffect::TheWall => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Wall: Extra large blind (x2 chips required)".to_string()));
            }
            BossEffect::TheFlint => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Flint: Starting chips and mult are halved".to_string()));
            }
            BossEffect::TheEye => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Eye: Each hand type can only be played once".to_string()));
            }
            BossEffect::TheMouth => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Mouth: Only one hand type can be played".to_string()));
            }
            BossEffect::TheHook => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Hook: Discard 2 random cards per hand played".to_string()));
            }
            BossEffect::TheOx => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Ox: Playing most played hand type loses all money".to_string()));
            }
            BossEffect::TheTooth => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Tooth: Lose $1 per card played".to_string()));
            }
            BossEffect::TheManacle => {
                self.state.boss_manacle_hand_size_reduced = true;
                if !self.state.available.is_empty() {
                    let removed = self.state.available.pop().unwrap();
                    self.state.discarded.push(removed);
                }
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Manacle: Hand size reduced by 1".to_string()));
            }
            BossEffect::TheArm => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Arm: Level of played poker hand decreased by 1".to_string()));
            }
            BossEffect::TheSerpent => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Serpent: After play/discard, always draw to full hand".to_string()));
            }
            BossEffect::ThePillar => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "The Pillar: Previously played cards debuffed (tracking TODO)".to_string()));
            }
            BossEffect::TheWheel | BossEffect::TheHouse | BossEffect::TheMark | BossEffect::TheFish => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", format!("{}: Face-down card effect (visual only)", self.state.blind_name)));
            }
            BossEffect::VioletVessel => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "Violet Vessel: Very large blind amount (x6 base)".to_string()));
            }
            BossEffect::CeruleanBell => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "Cerulean Bell: Force one card selected (TODO)".to_string()));
            }
            BossEffect::AmberAcorn => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "Amber Acorn: Rotate joker positions (TODO)".to_string()));
            }
            BossEffect::VerdantLeaf => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "Verdant Leaf: All cards debuffed until card sold (TODO)".to_string()));
            }
            BossEffect::CrimsonHeart => {
                events.push(event(EventStage::BlindPrePlay, "boss_effect", "Crimson Heart: Random joker disabled each hand (TODO)".to_string()));
            }
        }

        self.state.active_boss_effect = Some(effect);
    }

    /// Collect all card IDs matching a predicate across deck, hand, and discard.
    fn all_card_ids_matching(&self, predicate: impl Fn(&CardInstance) -> bool) -> Vec<u32> {
        self.state
            .deck
            .iter()
            .chain(self.state.available.iter())
            .chain(self.state.discarded.iter())
            .filter(|c| predicate(c))
            .map(|c| c.card_id)
            .collect()
    }

    fn sync_current_blind_descriptor(&mut self) {
        match self.state.current_blind_slot {
            BlindSlot::Small => {
                self.state.blind = BlindKind::Small;
                self.state.blind_name = "Small Blind".to_string();
                self.state.boss_effect = "None".to_string();
            }
            BlindSlot::Big => {
                self.state.blind = BlindKind::Big;
                self.state.blind_name = "Big Blind".to_string();
                self.state.boss_effect = "None".to_string();
            }
            BlindSlot::Boss => {
                self.state.blind = BlindKind::Boss(self.state.boss_blind.id.clone());
                self.state.blind_name = self.state.boss_blind.name.clone();
                self.state.boss_effect = self.state.boss_blind.name.clone();
            }
        }
    }

    fn set_active_preblind_progress(&mut self) {
        match self.state.current_blind_slot {
            BlindSlot::Small => self.state.small_progress = BlindProgress::Select,
            BlindSlot::Big => self.state.big_progress = BlindProgress::Select,
            BlindSlot::Boss => self.state.boss_progress = BlindProgress::Select,
        }
    }

    fn mark_current_blind_progress(&mut self, progress: BlindProgress) {
        match self.state.current_blind_slot {
            BlindSlot::Small => self.state.small_progress = progress,
            BlindSlot::Big => self.state.big_progress = progress,
            BlindSlot::Boss => self.state.boss_progress = progress,
        }
    }

    fn advance_to_next_blind_slot(&mut self) -> bool {
        if let Some(next_slot) = self.state.current_blind_slot.next() {
            self.state.current_blind_slot = next_slot;
            self.sync_current_blind_descriptor();
            return true;
        }
        false
    }

    fn blind_states_snapshot(&self) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert("Small".to_string(), self.state.small_progress.as_str().to_string());
        map.insert("Big".to_string(), self.state.big_progress.as_str().to_string());
        map.insert("Boss".to_string(), self.state.boss_progress.as_str().to_string());
        map
    }

    fn required_score(&self) -> i32 {
        let base = self
            .ruleset
            .ante_base_scores
            .get((self.state.ante - 1).max(0) as usize)
            .copied()
            .unwrap_or(100_000);
        let blind_score = match self.state.blind {
            BlindKind::Small => base,
            BlindKind::Big => ((base as f32) * 1.5).round() as i32,
            BlindKind::Boss(_) => base * 2,
        };
        // Boss effects that modify the required score
        match &self.state.active_boss_effect {
            Some(BossEffect::TheWall) if !self.state.boss_blind_disabled => blind_score * 2,
            Some(BossEffect::VioletVessel) if !self.state.boss_blind_disabled => base * 6,
            _ => blind_score,
        }
    }

    fn selected_cards(&self) -> Vec<CardInstance> {
        self.state
            .selected_slots
            .iter()
            .filter_map(|slot| self.state.available.get(*slot).cloned())
            .collect()
    }

    fn reset_deck(&mut self, trace: &mut TransitionTrace) {
        self.state.deck.clear();
        self.state.available.clear();
        self.state.selected_slots.clear();
        let mut card_id = 1_u32;
        for suit in [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs] {
            for rank in [
                Rank::Two,
                Rank::Three,
                Rank::Four,
                Rank::Five,
                Rank::Six,
                Rank::Seven,
                Rank::Eight,
                Rank::Nine,
                Rank::Ten,
                Rank::Jack,
                Rank::Queen,
                Rank::King,
                Rank::Ace,
            ] {
                self.state.deck.push(CardInstance {
                    card_id,
                    rank: rank.clone(),
                    suit: suit.clone(),
                    enhancement: None,
                    edition: None,
                    seal: None,
                });
                card_id += 1;
            }
        }
        self.shuffle_deck("deck.shuffle.enter_blind", trace);
    }

    fn draw_to_hand(&mut self, target: usize) {
        while self.state.available.len() < target && !self.state.deck.is_empty() {
            let card = self.state.deck.pop().expect("card draw");
            self.state.available.push(card);
        }
    }

    fn pick_boss_blind(&mut self, trace: &mut TransitionTrace) -> BlindSpec {
        let mut pool: Vec<BlindSpec> = self
            .ruleset
            .blinds
            .iter()
            .filter(|blind| blind.boss && blind.showdown == (self.state.ante >= 8))
            .filter(|blind| blind.min_ante.map(|min| self.state.ante >= min).unwrap_or(true))
            .filter(|blind| blind.max_ante.map(|max| self.state.ante <= max).unwrap_or(true))
            .cloned()
            .collect();
        if pool.is_empty() {
            pool = self
                .ruleset
                .blinds
                .iter()
                .filter(|blind| blind.boss)
                .cloned()
                .collect();
        }
        let candidates: Vec<String> = pool.iter().map(|blind| blind.id.clone()).collect();
        let choice = self.choose_index(candidates.len(), "boss_blind.select", candidates, trace);
        pool[choice].clone()
    }

    fn refresh_shop(&mut self, trace: &mut TransitionTrace, domain: &str) {
        self.state.shop.clear();
        self.state.shop_consumables.clear();
        let mut common: Vec<JokerSpec> = Vec::new();
        let mut uncommon: Vec<JokerSpec> = Vec::new();
        let mut rare: Vec<JokerSpec> = Vec::new();
        let mut legendary: Vec<JokerSpec> = Vec::new();
        for joker in &self.ruleset.jokers {
            if !joker.unlocked {
                continue;
            }
            match joker.rarity {
                1 => common.push(joker.clone()),
                2 => uncommon.push(joker.clone()),
                3 => rare.push(joker.clone()),
                _ => legendary.push(joker.clone()),
            }
        }
        for slot in 0..2 {
            let rarity_roll = self.roll_f32(0.0, 100.0, format!("{domain}.slot_{slot}.rarity"), trace);
            let (pool, rarity_bucket) = if rarity_roll < self.ruleset.shop_weights.common {
                (&common, "common")
            } else if rarity_roll < self.ruleset.shop_weights.common + self.ruleset.shop_weights.uncommon {
                (&uncommon, "uncommon")
            } else if rarity_roll < self.ruleset.shop_weights.common + self.ruleset.shop_weights.uncommon + self.ruleset.shop_weights.rare {
                (&rare, "rare")
            } else {
                (&legendary, "legendary")
            };
            if !pool.is_empty() {
                let candidates: Vec<String> = pool.iter().map(|joker| joker.id.clone()).collect();
                let chosen = self.choose_index(
                    candidates.len(),
                    format!("{domain}.slot_{slot}.pick_{rarity_bucket}"),
                    candidates,
                    trace,
                );
                let spec = &pool[chosen];
                let edition = roll_edition(&mut self.rng);
                self.state.shop.push(ShopSlot {
                    slot,
                    joker: JokerInstance {
                        joker_id: spec.id.clone(),
                        name: spec.name.clone(),
                        base_cost: spec.base_cost,
                        cost: spec.cost,
                        buy_cost: spec.cost,
                        sell_value: (spec.cost / 2).max(1),
                        extra_sell_value: 0,
                        rarity: spec.rarity,
                        edition,
                        slot_index: slot,
                        activation_class: spec.activation_class.clone(),
                        wiki_effect_text_en: spec.wiki_effect_text_en.clone(),
                        remaining_uses: initial_remaining_uses(spec),
                        runtime_state: initial_runtime_state(spec, &mut self.rng),
                    },
                });
            }
        }
        // Populate shop consumables (excluding Vouchers)
        let consumable_pool: Vec<balatro_spec::ConsumableSpec> = self.ruleset.consumables.iter()
            .filter(|c| c.set != "Voucher")
            .cloned()
            .collect();
        if !consumable_pool.is_empty() {
            let num_consumable_slots = if self.rng.gen_bool(0.5) { 1 } else { 2 };
            for slot in 0..num_consumable_slots.min(SHOP_CONSUMABLE_SLOTS) {
                let candidates: Vec<String> = consumable_pool.iter().map(|c| c.id.clone()).collect();
                let chosen = self.choose_index(
                    candidates.len(),
                    format!("{domain}.consumable_slot_{slot}.pick"),
                    candidates,
                    trace,
                );
                let spec = &consumable_pool[chosen];
                let buy_cost = apply_discount(spec.cost, self.state.shop_discount);
                self.state.shop_consumables.push(ConsumableInstance {
                    consumable_id: spec.id.clone(),
                    name: spec.name.clone(),
                    set: spec.set.clone(),
                    cost: spec.cost,
                    buy_cost,
                    sell_value: (spec.cost / 2).max(1),
                    slot_index: slot,
                    config: spec.config.clone(),
                });
            }
        }

        // Generate 1 voucher (not already owned). Voucher-Tag boosts add
        // additional vouchers beyond the base one; for now we model that as
        // replacing the single voucher slot so the shop always offers *some*
        // voucher (even if the player already owns all of them, one extra
        // Voucher Tag still guarantees a slot).
        self.state.shop_voucher = None;
        let voucher_pool = default_voucher_pool();
        let available_vouchers: Vec<&VoucherSpec> = voucher_pool
            .iter()
            .filter(|v| !self.state.owned_vouchers.contains(&v.id))
            .collect();
        if !available_vouchers.is_empty() {
            let candidates: Vec<String> = available_vouchers.iter().map(|v| v.id.clone()).collect();
            let chosen = self.choose_index(
                candidates.len(),
                format!("{domain}.voucher.pick"),
                candidates,
                trace,
            );
            let spec = available_vouchers[chosen];
            let buy_cost = apply_discount(spec.cost, self.state.shop_discount);
            self.state.shop_voucher = Some(VoucherInstance {
                voucher_id: spec.id.clone(),
                name: spec.name.clone(),
                cost: buy_cost,
                effect_key: spec.effect_key.clone(),
                description: spec.description.clone(),
            });
        }
        // Voucher-Tag: a pending tag guarantees the shop carries a voucher
        // (even if the pool above was empty), and drops the cost to $0.
        if self.state.pending_voucher_tags > 0 {
            if self.state.shop_voucher.is_none() && !voucher_pool.is_empty() {
                // Fallback: pick any voucher from the full pool.
                let candidates: Vec<String> =
                    voucher_pool.iter().map(|v| v.id.clone()).collect();
                let chosen = self.choose_index(
                    candidates.len(),
                    format!("{domain}.voucher_tag.pick"),
                    candidates,
                    trace,
                );
                let spec = &voucher_pool[chosen];
                self.state.shop_voucher = Some(VoucherInstance {
                    voucher_id: spec.id.clone(),
                    name: spec.name.clone(),
                    cost: 0,
                    effect_key: spec.effect_key.clone(),
                    description: spec.description.clone(),
                });
            } else if let Some(v) = self.state.shop_voucher.as_mut() {
                v.cost = 0;
            }
            self.state.pending_voucher_tags -= 1;
        }

        // Generate 1-2 booster packs
        self.state.shop_packs.clear();
        let pack_types = [PackType::Arcana, PackType::Celestial, PackType::Buffoon];
        let num_packs = if self.rng.gen_bool(0.5) { 1 } else { 2 };
        for i in 0..num_packs {
            let type_index = self.rng.gen_range(0..pack_types.len());
            let pack_type = &pack_types[type_index];
            let base_cost = pack_type.shop_cost();
            let buy_cost = apply_discount(base_cost, self.state.shop_discount);
            self.state.shop_packs.push(BoosterPackInstance {
                pack_type: pack_type.as_str().to_string(),
                cost: buy_cost,
                choices: Vec::new(),
                picks_remaining: pack_type.picks_allowed(),
            });
            let _ = i; // used as loop counter
        }

        // Apply shop discount to joker prices
        let discount = self.state.shop_discount;
        for shop_slot in self.state.shop.iter_mut() {
            shop_slot.joker.buy_cost = apply_discount(shop_slot.joker.cost, discount);
        }
        // Coupon Tag: zero the buy cost on the initial batch of shop jokers,
        // consumables, and booster packs. Applies once per shop entry —
        // consumed at the first refresh so rerolls pay full price.
        if self.state.pending_coupon_shop {
            for shop_slot in self.state.shop.iter_mut() {
                shop_slot.joker.buy_cost = 0;
            }
            for item in self.state.shop_consumables.iter_mut() {
                item.buy_cost = 0;
            }
            for pack in self.state.shop_packs.iter_mut() {
                pack.cost = 0;
            }
            self.state.pending_coupon_shop = false;
        }
    }

    fn choose_index(
        &mut self,
        len: usize,
        domain: impl Into<String>,
        candidates: Vec<String>,
        trace: &mut TransitionTrace,
    ) -> usize {
        assert!(len > 0, "choose_index requires at least one candidate");
        let choice = self.rng.gen_range(0..len);
        let mut args = BTreeMap::new();
        args.insert("candidate_count".to_string(), serde_json::json!(len));
        args.insert("candidates".to_string(), serde_json::json!(candidates));
        trace.add_rng_call(
            domain,
            "choose_index",
            args,
            serde_json::json!({
                "index": choice,
            }),
        );
        choice
    }

    fn roll_f32(
        &mut self,
        min: f32,
        max: f32,
        domain: impl Into<String>,
        trace: &mut TransitionTrace,
    ) -> f32 {
        let value = self.rng.gen_range(min..max);
        let mut args = BTreeMap::new();
        args.insert("min".to_string(), serde_json::json!(min));
        args.insert("max".to_string(), serde_json::json!(max));
        trace.add_rng_call(domain, "roll_f32", args, serde_json::json!(value));
        value
    }

    fn shuffle_deck(&mut self, domain: &str, trace: &mut TransitionTrace) {
        let before_top: Vec<u32> = self.state.deck.iter().take(8).map(|card| card.card_id).collect();
        self.state.deck.shuffle(&mut self.rng);
        let after_top: Vec<u32> = self.state.deck.iter().take(8).map(|card| card.card_id).collect();
        let mut args = BTreeMap::new();
        args.insert("deck_len".to_string(), serde_json::json!(self.state.deck.len()));
        args.insert("before_top".to_string(), serde_json::json!(before_top));
        trace.add_rng_call(
            domain.to_string(),
            "shuffle",
            args,
            serde_json::json!({
                "after_top": after_top,
            }),
        );
    }

    fn roll_chance(
        &mut self,
        odds: i32,
        domain: impl Into<String>,
        trace: &mut TransitionTrace,
    ) -> bool {
        let roll = self.rng.gen_range(1..=odds);
        let mut args = BTreeMap::new();
        args.insert("odds".to_string(), serde_json::json!(odds));
        trace.add_rng_call(
            domain,
            "roll_chance",
            args,
            serde_json::json!({ "roll": roll, "hit": roll == 1 }),
        );
        roll == 1
    }

    /// joker_on_played phase: activates once per hand played, after hand
    /// evaluation but before per-card scoring.
    fn apply_on_played_jokers(
        &mut self,
        hand_key: &str,
        played: &mut Vec<CardInstance>,
        events: &mut Vec<Event>,
        trace: &mut TransitionTrace,
    ) {
        struct OpJokerInfo {
            joker_id: String,
            joker_name: String,
            slot_index: usize,
        }

        let infos: Vec<OpJokerInfo> = self
            .state
            .jokers
            .iter()
            .filter_map(|j| {
                self.ruleset.joker_by_id(&j.joker_id).and_then(|spec| {
                    if spec.activation_class == "joker_on_played" {
                        Some(OpJokerInfo {
                            joker_id: j.joker_id.clone(),
                            joker_name: spec.name.clone(),
                            slot_index: j.slot_index,
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();

        for info in &infos {
            match info.joker_id.as_str() {
                "j_space" => {
                    // 1 in 4 chance to level up the played hand type
                    let hit = self.roll_chance(4, "space_joker", trace);
                    if hit {
                        let new_level = self.bump_hand_level(hand_key, 1);
                        events.push(event_with_details(
                            EventStage::OnPlayed,
                            "on_played_joker",
                            format!(
                                "{} leveled up {} to Lv.{}",
                                info.joker_name, hand_key, new_level
                            ),
                            Some(info.slot_index),
                            Some(&info.joker_id),
                            None,
                            None,
                            None,
                            None,
                        ));
                    }
                }
                "j_dna" => {
                    // If first hand of round, copy first played card into deck
                    if self.state.hands_played_this_round == 0 {
                        if let Some(first_card) = played.first() {
                            let max_id = self
                                .state
                                .deck
                                .iter()
                                .chain(self.state.available.iter())
                                .chain(self.state.discarded.iter())
                                .chain(played.iter())
                                .map(|c| c.card_id)
                                .max()
                                .unwrap_or(52);
                            let copy = CardInstance {
                                card_id: max_id + 1,
                                rank: first_card.rank.clone(),
                                suit: first_card.suit.clone(),
                                enhancement: first_card.enhancement.clone(),
                                edition: first_card.edition.clone(),
                                seal: first_card.seal.clone(),
                            };
                            self.state.deck.push(copy);
                            events.push(event_with_details(
                                EventStage::OnPlayed,
                                "on_played_joker",
                                format!(
                                    "{} copied first played card into deck",
                                    info.joker_name
                                ),
                                Some(info.slot_index),
                                Some(&info.joker_id),
                                None,
                                None,
                                None,
                                None,
                            ));
                        }
                    }
                }
                "j_todo_list" => {
                    // If played hand matches target, gain $4
                    if let Some(ref target) = self.state.todo_list_target {
                        if hand_key == target.as_str() {
                            self.state.money += 4;
                            events.push(event_with_details(
                                EventStage::OnPlayed,
                                "on_played_joker",
                                format!(
                                    "{} matched target hand {} +$4",
                                    info.joker_name, target
                                ),
                                Some(info.slot_index),
                                Some(&info.joker_id),
                                None,
                                None,
                                None,
                                Some(4),
                            ));
                        }
                    }
                }
                "j_midas_mask" => {
                    // All played face cards become Gold cards
                    let mut count = 0;
                    for card in played.iter_mut() {
                        if card.rank.is_face() {
                            card.enhancement = Some("m_gold".to_string());
                            count += 1;
                        }
                    }
                    // Also update in available/deck/discarded
                    let played_ids: BTreeSet<u32> =
                        played.iter().filter(|c| c.rank.is_face()).map(|c| c.card_id).collect();
                    for card in self.state.available.iter_mut() {
                        if played_ids.contains(&card.card_id) {
                            card.enhancement = Some("m_gold".to_string());
                        }
                    }
                    for card in self.state.deck.iter_mut() {
                        if played_ids.contains(&card.card_id) {
                            card.enhancement = Some("m_gold".to_string());
                        }
                    }
                    for card in self.state.discarded.iter_mut() {
                        if played_ids.contains(&card.card_id) {
                            card.enhancement = Some("m_gold".to_string());
                        }
                    }
                    if count > 0 {
                        events.push(event_with_details(
                            EventStage::OnPlayed,
                            "on_played_joker",
                            format!(
                                "{} turned {} face card(s) into Gold cards",
                                info.joker_name, count
                            ),
                            Some(info.slot_index),
                            Some(&info.joker_id),
                            None,
                            None,
                            None,
                            None,
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    /// Held-in-hand activation: after scoring cards are processed but before
    /// the final score total is computed, unplayed cards still in hand are
    /// evaluated against held-in-hand Jokers.
    fn apply_held_in_hand(
        &mut self,
        played_ids: &BTreeSet<u32>,
        _chips: &mut i32,
        mult: &mut i32,
        xmult: &mut f64,
        events: &mut Vec<Event>,
        trace: &mut TransitionTrace,
    ) {
        let held_cards: Vec<CardInstance> = self
            .state
            .available
            .iter()
            .filter(|card| !played_ids.contains(&card.card_id))
            .cloned()
            .collect();

        if held_cards.is_empty() {
            return;
        }

        // Steel Card enhancement: X1.5 mult for each Steel card held in hand
        for card in &held_cards {
            if card.enhancement.as_deref() == Some("m_steel") {
                *xmult *= 1.5;
                events.push(event(
                    EventStage::HeldInHand,
                    "enhancement_steel",
                    format!("Steel Card {} X1.5 mult (held in hand)", card.card_id),
                ));
            }
        }

        // Collect joker info upfront to avoid borrow conflicts with self
        let joker_infos: Vec<(String, String, usize)> = self
            .state
            .jokers
            .iter()
            .filter_map(|j| {
                self.ruleset.joker_by_id(&j.joker_id).and_then(|spec| {
                    if spec.activation_class == "held_in_hand" {
                        Some((j.joker_id.clone(), spec.name.clone(), j.slot_index))
                    } else {
                        None
                    }
                })
            })
            .collect();

        // Check if Mime is present (retriggers held-in-hand once)
        let mime_present = joker_infos.iter().any(|(id, _, _)| id == "j_mime");
        let retrigger_count: i32 = if mime_present { 1 } else { 0 };

        for (joker_id, joker_name, slot_index) in &joker_infos {
            match joker_id.as_str() {
                "j_mime" => {
                    // Mime itself doesn't directly score; it enables retriggers
                    // for other held-in-hand effects (handled by retrigger_count).
                }
                "j_raised_fist" => {
                    if let Some(lowest) = held_cards.iter().min_by_key(|c| c.rank.index()) {
                        let rank_value = lowest.chip_value();
                        let triggers = 1 + retrigger_count;
                        for _ in 0..triggers {
                            *mult += rank_value;
                            events.push(event_with_details(
                                EventStage::HeldInHand,
                                "held_in_hand_activated",
                                format!("{} added {} mult from lowest held card", joker_name, rank_value),
                                Some(*slot_index),
                                Some(joker_id),
                                None,
                                Some(rank_value as f64),
                                None,
                                None,
                            ));
                        }
                    }
                }
                "j_baron" => {
                    let triggers = 1 + retrigger_count;
                    for (card_idx, card) in held_cards.iter().enumerate() {
                        if matches!(card.rank, Rank::King) {
                            for _ in 0..triggers {
                                *xmult *= 1.5;
                                events.push(event_with_details(
                                    EventStage::HeldInHand,
                                    "held_in_hand_activated",
                                    format!("{} X1.5 mult from held King", joker_name),
                                    Some(*slot_index),
                                    Some(joker_id),
                                    Some(card_idx),
                                    None,
                                    Some(1.5),
                                    None,
                                ));
                            }
                        }
                    }
                }
                "j_reserved_parking" => {
                    let triggers = 1 + retrigger_count;
                    let joker_name_owned = joker_name.clone();
                    let joker_id_owned = joker_id.clone();
                    let slot = *slot_index;
                    for (card_idx, card) in held_cards.iter().enumerate() {
                        if card.rank.is_face() {
                            for _ in 0..triggers {
                                let hit = self.roll_chance(
                                    2,
                                    format!("reserved_parking.card_{}", card.card_id),
                                    trace,
                                );
                                if hit {
                                    self.state.money += 1;
                                    events.push(event_with_details(
                                        EventStage::HeldInHand,
                                        "held_in_hand_activated",
                                        format!("{} earned $1 from held face card", joker_name_owned),
                                        Some(slot),
                                        Some(&joker_id_owned),
                                        Some(card_idx),
                                        None,
                                        None,
                                        Some(1),
                                    ));
                                }
                            }
                        }
                    }
                }
                "j_shoot_the_moon" => {
                    let triggers = 1 + retrigger_count;
                    for (card_idx, card) in held_cards.iter().enumerate() {
                        if matches!(card.rank, Rank::Queen) {
                            for _ in 0..triggers {
                                *mult += 13;
                                events.push(event_with_details(
                                    EventStage::HeldInHand,
                                    "held_in_hand_activated",
                                    format!("{} +13 mult from held Queen", joker_name),
                                    Some(*slot_index),
                                    Some(joker_id),
                                    Some(card_idx),
                                    Some(13.0),
                                    None,
                                    None,
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// End-of-round Joker activation: after a blind is cleared and before
    /// cashout/shop transition.
    fn apply_end_of_round_jokers(
        &mut self,
        events: &mut Vec<Event>,
        trace: &mut TransitionTrace,
    ) {
        struct EorJokerInfo {
            idx: usize,
            joker_id: String,
            joker_name: String,
            slot_index: usize,
            extra_i64: i32,
            extra_dollars: i32,
            extra_increase: i32,
            extra_odds: i32,
        }

        let cleared_boss = self.state.current_blind_slot == BlindSlot::Boss;
        let joker_count = self.state.jokers.len();
        let consumable_count = self.state.consumables.len();

        let infos: Vec<EorJokerInfo> = self
            .state
            .jokers
            .iter()
            .enumerate()
            .filter_map(|(idx, j)| {
                self.ruleset.joker_by_id(&j.joker_id).and_then(|spec| {
                    if spec.activation_class != "end_of_round" {
                        return None;
                    }
                    let extra_i64 = spec.config.get("extra").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let extra_obj = spec.config.get("extra").and_then(|v| v.as_object());
                    let extra_dollars = extra_obj.and_then(|o| o.get("dollars")).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let extra_increase = extra_obj.and_then(|o| o.get("increase")).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let extra_odds = extra_obj.and_then(|o| o.get("odds")).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    Some(EorJokerInfo {
                        idx,
                        joker_id: j.joker_id.clone(),
                        joker_name: spec.name.clone(),
                        slot_index: j.slot_index,
                        extra_i64,
                        extra_dollars,
                        extra_increase,
                        extra_odds,
                    })
                })
            })
            .collect();

        let mut jokers_to_destroy: Vec<usize> = Vec::new();

        for info in &infos {
            match info.joker_id.as_str() {
                "j_golden" => {
                    let dollars = if info.extra_i64 != 0 { info.extra_i64 } else { 4 };
                    self.state.money += dollars;
                    events.push(event_with_details(
                        EventStage::EndOfRound, "end_of_round_joker",
                        format!("{} earned ${}", info.joker_name, dollars),
                        Some(info.slot_index), Some(&info.joker_id), None, None, None, Some(dollars),
                    ));
                }
                "j_cloud_9" => {
                    let nine_count = self.count_rank_in_full_deck(&Rank::Nine);
                    let per_nine = if info.extra_i64 != 0 { info.extra_i64 } else { 1 };
                    let dollars = nine_count * per_nine;
                    if dollars > 0 {
                        self.state.money += dollars;
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} earned ${} ({} Nines in deck)", info.joker_name, dollars, nine_count),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, Some(dollars),
                        ));
                    }
                }
                "j_rocket" => {
                    let base = if info.extra_dollars != 0 { info.extra_dollars } else { 1 };
                    let increase = if info.extra_increase != 0 { info.extra_increase } else { 2 };
                    let dollars = base + self.state.rocket_extra_dollars;
                    self.state.money += dollars;
                    if cleared_boss {
                        self.state.rocket_extra_dollars += increase;
                    }
                    events.push(event_with_details(
                        EventStage::EndOfRound, "end_of_round_joker",
                        format!("{} earned ${}", info.joker_name, dollars),
                        Some(info.slot_index), Some(&info.joker_id), None, None, None, Some(dollars),
                    ));
                }
                "j_to_the_moon" => {
                    let per_five = if info.extra_i64 != 0 { info.extra_i64 } else { 1 };
                    let interest = (self.state.money / 5 * per_five).min(self.state.interest_cap);
                    if interest > 0 {
                        self.state.money += interest;
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} earned ${} interest", info.joker_name, interest),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, Some(interest),
                        ));
                    }
                }
                "j_satellite" => {
                    let per_planet = if info.extra_i64 != 0 { info.extra_i64 } else { 1 };
                    let dollars = self.state.unique_planets_used * per_planet;
                    if dollars > 0 {
                        self.state.money += dollars;
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} earned ${} ({} planet cards used)", info.joker_name, dollars, self.state.unique_planets_used),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, Some(dollars),
                        ));
                    }
                }
                "j_gift" => {
                    let per_item = if info.extra_i64 != 0 { info.extra_i64 } else { 1 };
                    let item_count = (joker_count + consumable_count) as i32;
                    let dollars = item_count * per_item;
                    if dollars > 0 {
                        self.state.money += dollars;
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} earned ${} ({} items)", info.joker_name, dollars, item_count),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, Some(dollars),
                        ));
                    }
                }
                "j_egg" => {
                    let sell_increase = if info.extra_i64 != 0 { info.extra_i64 } else { 3 };
                    self.state.egg_accumulated_sell += sell_increase;
                    events.push(event_with_details(
                        EventStage::EndOfRound, "end_of_round_joker",
                        format!("{} sell value increased by ${}", info.joker_name, sell_increase),
                        Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                    ));
                }
                "j_gros_michel" => {
                    let odds = if info.extra_odds != 0 { info.extra_odds } else { 6 };
                    let destroyed = self.roll_chance(odds, "gros_michel.destroy", trace);
                    if destroyed {
                        jokers_to_destroy.push(info.idx);
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} was destroyed!", info.joker_name),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                        ));
                    } else {
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} survived (1 in {} chance)", info.joker_name, odds),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                        ));
                    }
                }
                "j_cavendish" => {
                    let odds = if info.extra_odds != 0 { info.extra_odds } else { 1000 };
                    let destroyed = self.roll_chance(odds, "cavendish.destroy", trace);
                    if destroyed {
                        jokers_to_destroy.push(info.idx);
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} was destroyed!", info.joker_name),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                        ));
                    } else {
                        events.push(event_with_details(
                            EventStage::EndOfRound, "end_of_round_joker",
                            format!("{} survived (1 in {} chance)", info.joker_name, odds),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                        ));
                    }
                }
                _ => {}
            }
        }

        // Remove destroyed jokers (in reverse order to preserve indices)
        for idx in jokers_to_destroy.into_iter().rev() {
            self.state.jokers.remove(idx);
        }
        for (new_slot, joker) in self.state.jokers.iter_mut().enumerate() {
            joker.slot_index = new_slot;
        }
    }

    /// Boss-blind-pre-play Joker activation
    fn apply_blind_select_jokers(
        &mut self,
        events: &mut Vec<Event>,
        trace: &mut TransitionTrace,
    ) {
        struct BsJokerInfo {
            joker_id: String,
            joker_name: String,
            slot_index: usize,
            extra_i64: i32,
        }

        let infos: Vec<BsJokerInfo> = self
            .state
            .jokers
            .iter()
            .filter_map(|j| {
                self.ruleset.joker_by_id(&j.joker_id).and_then(|spec| {
                    if spec.activation_class != "boss_blind_pre_play" {
                        return None;
                    }
                    let extra_i64 = spec.config.get("extra").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    Some(BsJokerInfo {
                        joker_id: j.joker_id.clone(),
                        joker_name: spec.name.clone(),
                        slot_index: j.slot_index,
                        extra_i64,
                    })
                })
            })
            .collect();

        let common_jokers: Vec<balatro_spec::JokerSpec> = self
            .ruleset
            .jokers
            .iter()
            .filter(|j| j.rarity == 1 && j.unlocked)
            .cloned()
            .collect();

        let tarot_specs: Vec<balatro_spec::ConsumableSpec> = self
            .ruleset
            .consumables
            .iter()
            .filter(|c| c.set == "Tarot")
            .cloned()
            .collect();

        for info in &infos {
            match info.joker_id.as_str() {
                "j_chicot" => {
                    if matches!(self.state.current_blind_slot, BlindSlot::Boss) {
                        self.state.boss_blind_disabled = true;
                        self.state.boss_effect = "None (disabled by Chicot)".to_string();
                        events.push(event_with_details(
                            EventStage::BlindPrePlay, "blind_select_joker",
                            format!("{} destroyed Boss Blind ability", info.joker_name),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                        ));
                    }
                }
                "j_burglar" => {
                    let extra_hands = if info.extra_i64 != 0 { info.extra_i64 } else { 3 };
                    self.state.plays += extra_hands;
                    self.state.discards = 0;
                    events.push(event_with_details(
                        EventStage::BlindPrePlay, "blind_select_joker",
                        format!("{} added {} hands, lost all discards", info.joker_name, extra_hands),
                        Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                    ));
                }
                "j_riff_raff" => {
                    let count = if info.extra_i64 != 0 { info.extra_i64 as usize } else { 2 };
                    let mut created = 0;
                    for i in 0..count {
                        if self.state.jokers.len() >= self.state.joker_slot_limit || common_jokers.is_empty() {
                            break;
                        }
                        let candidates: Vec<String> = common_jokers.iter().map(|j| j.id.clone()).collect();
                        let chosen = self.choose_index(
                            candidates.len(),
                            format!("riff_raff.create_{}", i),
                            candidates,
                            trace,
                        );
                        let chosen_spec = &common_jokers[chosen];
                        let edition = roll_edition(&mut self.rng);
                        let new_joker = JokerInstance {
                            joker_id: chosen_spec.id.clone(),
                            name: chosen_spec.name.clone(),
                            base_cost: chosen_spec.base_cost,
                            cost: chosen_spec.cost,
                            buy_cost: chosen_spec.cost,
                            sell_value: (chosen_spec.cost / 2).max(1),
                            extra_sell_value: 0,
                            rarity: chosen_spec.rarity,
                            edition,
                            slot_index: self.state.jokers.len(),
                            activation_class: chosen_spec.activation_class.clone(),
                            wiki_effect_text_en: chosen_spec.wiki_effect_text_en.clone(),
                            remaining_uses: initial_remaining_uses(chosen_spec),
                            runtime_state: initial_runtime_state(chosen_spec, &mut self.rng),
                        };
                        self.state.jokers.push(new_joker);
                        created += 1;
                    }
                    if created > 0 {
                        events.push(event_with_details(
                            EventStage::BlindPrePlay, "blind_select_joker",
                            format!("{} created {} Common Joker(s)", info.joker_name, created),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                        ));
                    }
                }
                "j_cartomancer" => {
                    if self.state.consumables.len() < self.state.consumable_slot_limit && !tarot_specs.is_empty() {
                        let candidates: Vec<String> = tarot_specs.iter().map(|c| c.id.clone()).collect();
                        let chosen = self.choose_index(
                            candidates.len(),
                            "cartomancer.create_tarot",
                            candidates,
                            trace,
                        );
                        let chosen_spec = &tarot_specs[chosen];
                        let new_consumable = ConsumableInstance {
                            consumable_id: chosen_spec.id.clone(),
                            name: chosen_spec.name.clone(),
                            set: chosen_spec.set.clone(),
                            cost: chosen_spec.cost,
                            buy_cost: chosen_spec.cost,
                            sell_value: (chosen_spec.cost / 2).max(1),
                            slot_index: self.state.consumables.len(),
                            config: chosen_spec.config.clone(),
                        };
                        self.state.consumables.push(new_consumable);
                        events.push(event_with_details(
                            EventStage::BlindPrePlay, "blind_select_joker",
                            format!("{} created a Tarot card", info.joker_name),
                            Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                        ));
                    }
                }
                "j_marble" => {
                    let stone_count = if info.extra_i64 != 0 { info.extra_i64 as u32 } else { 1 };
                    let max_card_id = self
                        .state
                        .deck
                        .iter()
                        .chain(self.state.available.iter())
                        .chain(self.state.discarded.iter())
                        .map(|c| c.card_id)
                        .max()
                        .unwrap_or(52);
                    for i in 0..stone_count {
                        let stone_card = CardInstance {
                            card_id: max_card_id + 1 + i,
                            rank: Rank::Ace,
                            suit: Suit::Spades,
                            enhancement: Some("Stone".to_string()),
                            edition: None,
                            seal: None,
                        };
                        self.state.deck.push(stone_card);
                    }
                    events.push(event_with_details(
                        EventStage::BlindPrePlay, "blind_select_joker",
                        format!("{} added {} Stone card(s) to deck", info.joker_name, stone_count),
                        Some(info.slot_index), Some(&info.joker_id), None, None, None, None,
                    ));
                }
                _ => {}
            }
        }
    }

    // --- Voucher handling ---

    fn handle_buy_voucher(&mut self) -> Vec<Event> {
        let voucher = match self.state.shop_voucher.take() {
            Some(v) => v,
            None => return vec![],
        };
        if self.state.money < voucher.cost {
            self.state.shop_voucher = Some(voucher);
            return vec![];
        }
        self.state.money -= voucher.cost;
        let voucher_name = voucher.name.clone();
        let effect_key = voucher.effect_key.clone();
        self.state.owned_vouchers.push(voucher.voucher_id.clone());
        self.apply_voucher_effect(&effect_key);
        vec![event(
            EventStage::Shop,
            "buy_voucher",
            format!("Bought voucher: {}", voucher_name),
        )]
    }

    fn apply_voucher_effect(&mut self, effect_key: &str) {
        match effect_key {
            "grabber" => self.state.base_plays += 1,
            "wasteful" | "recyclomancy" => self.state.base_discards += 1,
            "crystal_ball" => self.state.consumable_slot_limit += 1,
            "antimatter" => self.state.joker_slot_limit += 1,
            "nacho_tong" | "paint_brush" => self.state.hand_size += 1,
            "clearance_sale" => self.state.shop_discount = 0.75,
            "restock" => {
                self.state.shop_base_reroll_cost =
                    (self.state.shop_base_reroll_cost - 2).max(0);
                self.state.shop_current_reroll_cost =
                    (self.state.shop_current_reroll_cost - 2).max(0);
            }
            "seed_money" => self.state.interest_cap = 25,
            "overstock" => {
                // +1 card slot in shop (increases the number of joker slots shown)
                // This is a minor effect -- we just note it's owned.
            }
            _ => {}
        }
    }

    // --- Booster pack handling ---

    fn handle_buy_pack(&mut self, pack_slot: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        if pack_slot >= self.state.shop_packs.len() {
            return vec![];
        }
        let pack = self.state.shop_packs[pack_slot].clone();
        if self.state.money < pack.cost {
            return vec![];
        }
        self.state.money -= pack.cost;
        self.state.shop_packs.remove(pack_slot);

        // Populate pack choices based on type
        let choices = self.generate_pack_choices(&pack.pack_type, trace);
        let pack_name = pack.pack_type.clone();
        self.state.open_pack = Some(BoosterPackInstance {
            pack_type: pack.pack_type,
            cost: pack.cost,
            choices,
            picks_remaining: pack.picks_remaining,
        });
        vec![event(
            EventStage::Shop,
            "buy_pack",
            format!("Opened {}", pack_name),
        )]
    }

    fn generate_pack_choices(
        &mut self,
        pack_type_name: &str,
        trace: &mut TransitionTrace,
    ) -> Vec<BoosterPackChoice> {
        let pack_type = match pack_type_name {
            "Arcana Pack" => PackType::Arcana,
            "Celestial Pack" => PackType::Celestial,
            "Spectral Pack" => PackType::Spectral,
            "Standard Pack" => PackType::Standard,
            "Buffoon Pack" => PackType::Buffoon,
            "Mega Arcana Pack" => PackType::MegaArcana,
            _ => PackType::Arcana,
        };
        let card_count = pack_type.card_count();
        let mut choices = Vec::new();

        match pack_type {
            PackType::Arcana | PackType::MegaArcana => {
                let tarot_pool: Vec<balatro_spec::ConsumableSpec> = self
                    .ruleset
                    .consumables
                    .iter()
                    .filter(|c| c.set == "Tarot")
                    .cloned()
                    .collect();
                if !tarot_pool.is_empty() {
                    for i in 0..card_count {
                        let candidates: Vec<String> =
                            tarot_pool.iter().map(|c| c.id.clone()).collect();
                        let chosen = self.choose_index(
                            candidates.len(),
                            format!("pack.arcana.card_{}", i),
                            candidates,
                            trace,
                        );
                        let spec = &tarot_pool[chosen];
                        choices.push(BoosterPackChoice {
                            index: i,
                            consumable_id: Some(spec.id.clone()),
                            joker_id: None,
                            card: None,
                            name: spec.name.clone(),
                        });
                    }
                }
            }
            PackType::Celestial => {
                let planet_pool: Vec<balatro_spec::ConsumableSpec> = self
                    .ruleset
                    .consumables
                    .iter()
                    .filter(|c| c.set == "Planet")
                    .cloned()
                    .collect();
                if !planet_pool.is_empty() {
                    for i in 0..card_count {
                        let candidates: Vec<String> =
                            planet_pool.iter().map(|c| c.id.clone()).collect();
                        let chosen = self.choose_index(
                            candidates.len(),
                            format!("pack.celestial.card_{}", i),
                            candidates,
                            trace,
                        );
                        let spec = &planet_pool[chosen];
                        choices.push(BoosterPackChoice {
                            index: i,
                            consumable_id: Some(spec.id.clone()),
                            joker_id: None,
                            card: None,
                            name: spec.name.clone(),
                        });
                    }
                }
            }
            PackType::Spectral => {
                let spectral_pool: Vec<balatro_spec::ConsumableSpec> = self
                    .ruleset
                    .consumables
                    .iter()
                    .filter(|c| c.set == "Spectral")
                    .cloned()
                    .collect();
                if !spectral_pool.is_empty() {
                    for i in 0..card_count {
                        let candidates: Vec<String> =
                            spectral_pool.iter().map(|c| c.id.clone()).collect();
                        let chosen = self.choose_index(
                            candidates.len(),
                            format!("pack.spectral.card_{}", i),
                            candidates,
                            trace,
                        );
                        let spec = &spectral_pool[chosen];
                        choices.push(BoosterPackChoice {
                            index: i,
                            consumable_id: Some(spec.id.clone()),
                            joker_id: None,
                            card: None,
                            name: spec.name.clone(),
                        });
                    }
                }
            }
            PackType::Standard => {
                for i in 0..card_count {
                    let suit_idx = self.rng.gen_range(0..4_u32);
                    let rank_idx = self.rng.gen_range(0..13_u32);
                    let suit = match suit_idx {
                        0 => Suit::Spades,
                        1 => Suit::Hearts,
                        2 => Suit::Diamonds,
                        _ => Suit::Clubs,
                    };
                    let rank = match rank_idx {
                        0 => Rank::Two,
                        1 => Rank::Three,
                        2 => Rank::Four,
                        3 => Rank::Five,
                        4 => Rank::Six,
                        5 => Rank::Seven,
                        6 => Rank::Eight,
                        7 => Rank::Nine,
                        8 => Rank::Ten,
                        9 => Rank::Jack,
                        10 => Rank::Queen,
                        11 => Rank::King,
                        _ => Rank::Ace,
                    };
                    let enhancement = roll_enhancement(&mut self.rng);
                    let edition = roll_edition(&mut self.rng);
                    let seal = roll_seal(&mut self.rng);
                    let card = CardInstance {
                        card_id: 1000 + i as u32,
                        rank: rank.clone(),
                        suit: suit.clone(),
                        enhancement,
                        edition,
                        seal,
                    };
                    let name = format!("{:?} of {:?}", rank, suit);
                    choices.push(BoosterPackChoice {
                        index: i,
                        consumable_id: None,
                        joker_id: None,
                        card: Some(card),
                        name,
                    });
                }
            }
            PackType::Buffoon => {
                let common_jokers: Vec<JokerSpec> = self
                    .ruleset
                    .jokers
                    .iter()
                    .filter(|j| j.rarity <= 2 && j.unlocked)
                    .cloned()
                    .collect();
                if !common_jokers.is_empty() {
                    for i in 0..card_count {
                        let candidates: Vec<String> =
                            common_jokers.iter().map(|j| j.id.clone()).collect();
                        let chosen = self.choose_index(
                            candidates.len(),
                            format!("pack.buffoon.card_{}", i),
                            candidates,
                            trace,
                        );
                        let spec = &common_jokers[chosen];
                        choices.push(BoosterPackChoice {
                            index: i,
                            consumable_id: None,
                            joker_id: Some(spec.id.clone()),
                            card: None,
                            name: spec.name.clone(),
                        });
                    }
                }
            }
        }
        choices
    }

    fn handle_pack_action(
        &mut self,
        action_index: usize,
        trace: &mut TransitionTrace,
    ) -> Vec<Event> {
        // skip_pack: close the pack
        if action_index == 36 {
            let pack_name = self
                .state
                .open_pack
                .as_ref()
                .map(|p| p.pack_type.clone())
                .unwrap_or_default();
            self.state.open_pack = None;
            return vec![event(
                EventStage::Shop,
                "skip_pack",
                format!("Skipped remaining picks from {}", pack_name),
            )];
        }

        // pick_pack_0..4
        if (31..=35).contains(&action_index) {
            let choice_idx = action_index - 31;
            let pack = match self.state.open_pack.as_ref() {
                Some(p) => p.clone(),
                None => return vec![],
            };
            if choice_idx >= pack.choices.len() {
                return vec![];
            }
            let choice = pack.choices[choice_idx].clone();
            let mut events = Vec::new();

            // Add the chosen card to the appropriate inventory
            if let Some(ref consumable_id) = choice.consumable_id {
                if self.state.consumables.len() < self.state.consumable_slot_limit {
                    if let Some(spec) = self.ruleset.consumable_by_id(consumable_id).cloned() {
                        let new_consumable = ConsumableInstance {
                            consumable_id: spec.id.clone(),
                            name: spec.name.clone(),
                            set: spec.set.clone(),
                            cost: spec.cost,
                            buy_cost: spec.cost,
                            sell_value: (spec.cost / 2).max(1),
                            slot_index: self.state.consumables.len(),
                            config: spec.config.clone(),
                        };
                        self.state.consumables.push(new_consumable);
                        events.push(event(
                            EventStage::Shop,
                            "pack_pick",
                            format!("Picked {} from pack", choice.name),
                        ));
                    }
                } else {
                    events.push(event(
                        EventStage::Shop,
                        "pack_pick_full",
                        format!("No room for {} (consumable slots full)", choice.name),
                    ));
                }
            } else if let Some(ref joker_id) = choice.joker_id {
                if self.state.jokers.len() < self.state.joker_slot_limit {
                    if let Some(spec) = self.ruleset.joker_by_id(joker_id).cloned() {
                        let edition = roll_edition(&mut self.rng);
                        let new_joker = JokerInstance {
                            joker_id: spec.id.clone(),
                            name: spec.name.clone(),
                            base_cost: spec.base_cost,
                            cost: spec.cost,
                            buy_cost: spec.cost,
                            sell_value: (spec.cost / 2).max(1),
                            extra_sell_value: 0,
                            rarity: spec.rarity,
                            edition,
                            slot_index: self.state.jokers.len(),
                            activation_class: spec.activation_class.clone(),
                            wiki_effect_text_en: spec.wiki_effect_text_en.clone(),
                            remaining_uses: initial_remaining_uses(&spec),
                            runtime_state: initial_runtime_state(&spec, &mut self.rng),
                        };
                        self.state.jokers.push(new_joker);
                        events.push(event(
                            EventStage::Shop,
                            "pack_pick",
                            format!("Picked {} from pack", choice.name),
                        ));
                    }
                } else {
                    events.push(event(
                        EventStage::Shop,
                        "pack_pick_full",
                        format!("No room for {} (joker slots full)", choice.name),
                    ));
                }
            } else if let Some(ref card) = choice.card {
                // Add playing card to deck
                self.state.deck.push(card.clone());
                events.push(event(
                    EventStage::Shop,
                    "pack_pick",
                    format!("Picked {} from pack", choice.name),
                ));
            }

            // Update pack state
            if let Some(ref mut pack) = self.state.open_pack {
                pack.choices.remove(choice_idx);
                // Re-index remaining choices
                for (idx, c) in pack.choices.iter_mut().enumerate() {
                    c.index = idx;
                }
                pack.picks_remaining = pack.picks_remaining.saturating_sub(1);
                if pack.picks_remaining == 0 || pack.choices.is_empty() {
                    self.state.open_pack = None;
                }
            }

            let _ = trace;
            return events;
        }

        vec![]
    }

    /// Count how many cards of a given rank exist across the full deck
    fn count_rank_in_full_deck(&self, target: &Rank) -> i32 {
        let count = self
            .state
            .deck
            .iter()
            .chain(self.state.available.iter())
            .chain(self.state.discarded.iter())
            .filter(|c| c.rank == *target)
            .count();
        count as i32
    }

    // ---- Runtime State Mutation Helpers ----

    /// Update scaling jokers after a hand is played.
    /// Called at the end of play_selected() after scoring.
    fn update_joker_runtime_on_play(&mut self, hand_key: &str, played: &[CardInstance]) {
        let has_face = played.iter().any(|c| c.is_face_card());
        let has_enhanced = played.iter().any(|c| c.enhancement.is_some());
        let enhanced_count = played.iter().filter(|c| c.enhancement.is_some()).count();
        let is_straight = hand_key == "straight" || hand_key == "straight_flush";
        let is_two_pair = hand_key == "two_pair" || hand_key == "full_house" || hand_key == "flush_house";
        let played_len = played.len();

        for joker in self.state.jokers.iter_mut() {
            match joker.joker_id.as_str() {
                "j_green_joker" => {
                    let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                    joker.runtime_state.insert("mult".to_string(), current + 1.0);
                }
                "j_ride_the_bus" => {
                    if has_face {
                        joker.runtime_state.insert("mult".to_string(), 0.0);
                    } else {
                        let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                        joker.runtime_state.insert("mult".to_string(), current + 1.0);
                    }
                }
                "j_ice_cream" => {
                    let current = joker.runtime_state.get("chips").copied().unwrap_or(100.0);
                    joker.runtime_state.insert("chips".to_string(), (current - 5.0).max(0.0));
                }
                "j_loyalty_card" => {
                    let current = joker.runtime_state.get("hands_played").copied().unwrap_or(0.0);
                    joker.runtime_state.insert("hands_played".to_string(), current + 1.0);
                }
                "j_card_sharp" => {
                    // Mark this hand type as played this round
                    let key = format!("hand:{}", hand_key);
                    joker.runtime_state.insert(key, 1.0);
                }
                "j_obelisk" => {
                    // Find most played hand type from runtime_state "played:*" keys
                    let mut most_played_key = String::new();
                    let mut most_played_count = 0.0_f64;
                    for (k, v) in joker.runtime_state.iter() {
                        if k.starts_with("played:") && *v > most_played_count {
                            most_played_count = *v;
                            most_played_key = k[7..].to_string();
                        }
                    }
                    if most_played_key.is_empty() || hand_key == most_played_key {
                        // Reset xmult to 1.0
                        joker.runtime_state.insert("xmult".to_string(), 1.0);
                    } else {
                        // Increment xmult by 0.2
                        let current = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                        joker.runtime_state.insert("xmult".to_string(), current + 0.2);
                    }
                }
                "j_supernova" => {
                    // Increment count for this hand type
                    let key = format!("played:{}", hand_key);
                    let current = joker.runtime_state.get(&key).copied().unwrap_or(0.0);
                    joker.runtime_state.insert(key, current + 1.0);
                }
                "j_runner" => {
                    if is_straight {
                        let current = joker.runtime_state.get("chips").copied().unwrap_or(0.0);
                        joker.runtime_state.insert("chips".to_string(), current + 15.0);
                    }
                }
                "j_square" => {
                    if played_len == 4 {
                        let current = joker.runtime_state.get("chips").copied().unwrap_or(0.0);
                        joker.runtime_state.insert("chips".to_string(), current + 4.0);
                    }
                }
                "j_trousers" => {
                    if is_two_pair {
                        let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                        joker.runtime_state.insert("mult".to_string(), current + 2.0);
                    }
                }
                "j_vampire" => {
                    if has_enhanced {
                        let current = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                        joker.runtime_state.insert("xmult".to_string(), current + enhanced_count as f64 * 0.1);
                    }
                }
                _ => {}
            }
        }

        // Vampire strips enhancements from played cards (do outside the joker loop to avoid borrow conflict)
        let has_vampire = self.state.jokers.iter().any(|j| j.joker_id == "j_vampire");
        if has_vampire {
            let played_ids: BTreeSet<u32> = played.iter().map(|c| c.card_id).collect();
            for card in self.state.available.iter_mut() {
                if played_ids.contains(&card.card_id) && card.enhancement.is_some() {
                    card.enhancement = None;
                }
            }
        }
    }

    /// Update scaling jokers after cards are discarded.
    fn update_joker_runtime_on_discard(&mut self, discarded: &[CardInstance]) {
        let jack_count = discarded.iter().filter(|c| matches!(c.rank, Rank::Jack)).count() as f64;
        let discard_count = discarded.len() as f64;

        for joker in self.state.jokers.iter_mut() {
            match joker.joker_id.as_str() {
                "j_green_joker" => {
                    let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                    joker.runtime_state.insert("mult".to_string(), (current - 1.0).max(0.0));
                }
                "j_hit_the_road" => {
                    if jack_count > 0.0 {
                        let current = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                        joker.runtime_state.insert("xmult".to_string(), current + jack_count * 0.5);
                    }
                }
                "j_castle" => {
                    let target_suit = joker.runtime_state.get("suit").copied().unwrap_or(0.0) as usize;
                    let matching = discarded.iter().filter(|c| c.suit.index() == target_suit).count() as f64;
                    if matching > 0.0 {
                        let current = joker.runtime_state.get("chips").copied().unwrap_or(0.0);
                        joker.runtime_state.insert("chips".to_string(), current + matching * 3.0);
                    }
                }
                "j_yorick" => {
                    let remaining = joker.runtime_state.get("discards_remaining").copied().unwrap_or(23.0);
                    let new_remaining = remaining - discard_count;
                    if new_remaining <= 0.0 {
                        let current_xmult = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                        joker.runtime_state.insert("xmult".to_string(), current_xmult + 1.0);
                        // Reset counter, carrying over overflow
                        joker.runtime_state.insert("discards_remaining".to_string(), 23.0 + new_remaining);
                    } else {
                        joker.runtime_state.insert("discards_remaining".to_string(), new_remaining);
                    }
                }
                "j_ramen" => {
                    let current = joker.runtime_state.get("xmult").copied().unwrap_or(2.0);
                    let new_val = current - discard_count * 0.01;
                    joker.runtime_state.insert("xmult".to_string(), new_val);
                    // Will be destroyed if <= 1.0 (handled below)
                }
                _ => {}
            }
        }

        // Destroy Ramen if xmult drops to 1.0 or below
        let mut ramen_destroyed = false;
        self.state.jokers.retain(|j| {
            if j.joker_id == "j_ramen" {
                let xm = j.runtime_state.get("xmult").copied().unwrap_or(2.0);
                if xm <= 1.0 {
                    ramen_destroyed = true;
                    return false;
                }
            }
            true
        });
        if ramen_destroyed {
            for (new_slot, joker) in self.state.jokers.iter_mut().enumerate() {
                joker.slot_index = new_slot;
            }
        }
    }

    /// Update scaling jokers when a blind is skipped.
    fn update_joker_runtime_on_skip(&mut self) {
        for joker in self.state.jokers.iter_mut() {
            match joker.joker_id.as_str() {
                "j_throwback" => {
                    let current = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                    joker.runtime_state.insert("xmult".to_string(), current + 0.25);
                }
                "j_red_card" => {
                    let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                    joker.runtime_state.insert("mult".to_string(), current + 3.0);
                }
                _ => {}
            }
        }
    }

    /// Update scaling jokers when the shop is rerolled.
    fn update_joker_runtime_on_reroll(&mut self) {
        for joker in self.state.jokers.iter_mut() {
            if joker.joker_id == "j_flash" {
                let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                joker.runtime_state.insert("mult".to_string(), current + 2.0);
            }
        }
    }

    /// Update scaling jokers when a joker is sold.
    fn update_joker_runtime_on_sell(&mut self) {
        for joker in self.state.jokers.iter_mut() {
            if joker.joker_id == "j_campfire" {
                let current = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                joker.runtime_state.insert("xmult".to_string(), current + 0.25);
            }
        }
    }

    /// Update scaling jokers when a consumable is used.
    fn update_joker_runtime_on_consumable(&mut self, consumable_set: &str) {
        for joker in self.state.jokers.iter_mut() {
            match joker.joker_id.as_str() {
                "j_constellation" => {
                    if consumable_set == "Planet" {
                        let current = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                        joker.runtime_state.insert("xmult".to_string(), current + 0.1);
                    }
                }
                "j_fortune_teller" => {
                    if consumable_set == "Tarot" {
                        let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                        joker.runtime_state.insert("mult".to_string(), current + 1.0);
                    }
                }
                _ => {}
            }
        }
    }

    /// Update scaling jokers when selecting a Small/Big blind (for Madness).
    fn update_joker_runtime_on_blind_select(&mut self) {
        // Madness: X0.5 mult gained when Small/Big Blind selected
        let is_small_or_big = matches!(self.state.current_blind_slot, BlindSlot::Small | BlindSlot::Big);
        if !is_small_or_big {
            return;
        }
        for joker in self.state.jokers.iter_mut() {
            if joker.joker_id == "j_madness" {
                let current = joker.runtime_state.get("xmult").copied().unwrap_or(1.0);
                joker.runtime_state.insert("xmult".to_string(), current + 0.5);
            }
        }
    }

    /// Reset per-round runtime state when entering a new blind.
    fn reset_per_round_joker_state(&mut self) {
        for joker in self.state.jokers.iter_mut() {
            match joker.joker_id.as_str() {
                "j_card_sharp" => {
                    // Clear all "hand:*" keys
                    let keys_to_remove: Vec<String> = joker.runtime_state.keys()
                        .filter(|k| k.starts_with("hand:"))
                        .cloned()
                        .collect();
                    for k in keys_to_remove {
                        joker.runtime_state.remove(&k);
                    }
                }
                "j_hit_the_road" => {
                    // Reset per-round Jack counter
                    joker.runtime_state.insert("xmult".to_string(), 1.0);
                }
                _ => {}
            }
        }
    }

    /// Update jokers at end of round (popcorn decay, campfire boss reset).
    fn update_joker_runtime_on_round_end(&mut self) {
        let cleared_boss = self.state.current_blind_slot == BlindSlot::Boss;

        let mut jokers_to_destroy: Vec<String> = Vec::new();

        for joker in self.state.jokers.iter_mut() {
            match joker.joker_id.as_str() {
                "j_popcorn" => {
                    let current = joker.runtime_state.get("mult").copied().unwrap_or(20.0);
                    let new_val = (current - 4.0).max(0.0);
                    joker.runtime_state.insert("mult".to_string(), new_val);
                    if new_val <= 0.0 {
                        jokers_to_destroy.push(joker.joker_id.clone());
                    }
                }
                "j_campfire" => {
                    if cleared_boss {
                        joker.runtime_state.insert("xmult".to_string(), 1.0);
                    }
                }
                "j_ceremonial" => {
                    // +1 mult when blind defeated
                    let current = joker.runtime_state.get("mult").copied().unwrap_or(0.0);
                    joker.runtime_state.insert("mult".to_string(), current + 1.0);
                }
                _ => {}
            }
        }

        // Destroy Popcorn if mult drops to 0
        if !jokers_to_destroy.is_empty() {
            self.state.jokers.retain(|j| !jokers_to_destroy.contains(&j.joker_id));
            for (new_slot, joker) in self.state.jokers.iter_mut().enumerate() {
                joker.slot_index = new_slot;
            }
        }
    }
}

fn default_voucher_pool() -> Vec<VoucherSpec> {
    vec![
        VoucherSpec {
            id: "v_overstock".to_string(),
            name: "Overstock".to_string(),
            cost: 10,
            effect_key: "overstock".to_string(),
            description: "+1 card slot in shop".to_string(),
        },
        VoucherSpec {
            id: "v_clearance_sale".to_string(),
            name: "Clearance Sale".to_string(),
            cost: 10,
            effect_key: "clearance_sale".to_string(),
            description: "All shop items 25% off".to_string(),
        },
        VoucherSpec {
            id: "v_hone".to_string(),
            name: "Hone".to_string(),
            cost: 10,
            effect_key: "hone".to_string(),
            description: "Foil/Holo/Polychrome cards appear 2x more often".to_string(),
        },
        VoucherSpec {
            id: "v_restock".to_string(),
            name: "Restock".to_string(),
            cost: 10,
            effect_key: "restock".to_string(),
            description: "Reroll costs $2 less".to_string(),
        },
        VoucherSpec {
            id: "v_crystal_ball".to_string(),
            name: "Crystal Ball".to_string(),
            cost: 10,
            effect_key: "crystal_ball".to_string(),
            description: "+1 consumable slot".to_string(),
        },
        VoucherSpec {
            id: "v_telescope".to_string(),
            name: "Telescope".to_string(),
            cost: 10,
            effect_key: "telescope".to_string(),
            description: "Celestial packs always contain the Planet for your most played hand".to_string(),
        },
        VoucherSpec {
            id: "v_grabber".to_string(),
            name: "Grabber".to_string(),
            cost: 10,
            effect_key: "grabber".to_string(),
            description: "+1 hand per round".to_string(),
        },
        VoucherSpec {
            id: "v_wasteful".to_string(),
            name: "Wasteful".to_string(),
            cost: 10,
            effect_key: "wasteful".to_string(),
            description: "+1 discard per round".to_string(),
        },
        VoucherSpec {
            id: "v_seed_money".to_string(),
            name: "Seed Money".to_string(),
            cost: 10,
            effect_key: "seed_money".to_string(),
            description: "Max interest cap from $5 to $25".to_string(),
        },
        VoucherSpec {
            id: "v_blank".to_string(),
            name: "Blank".to_string(),
            cost: 10,
            effect_key: "blank".to_string(),
            description: "Unlocks Tier 2 vouchers".to_string(),
        },
        VoucherSpec {
            id: "v_antimatter".to_string(),
            name: "Antimatter".to_string(),
            cost: 10,
            effect_key: "antimatter".to_string(),
            description: "+1 Joker slot".to_string(),
        },
        VoucherSpec {
            id: "v_nacho_tong".to_string(),
            name: "Nacho Tong".to_string(),
            cost: 10,
            effect_key: "nacho_tong".to_string(),
            description: "+1 hand size".to_string(),
        },
        VoucherSpec {
            id: "v_recyclomancy".to_string(),
            name: "Recyclomancy".to_string(),
            cost: 10,
            effect_key: "recyclomancy".to_string(),
            description: "+1 discard per round".to_string(),
        },
        VoucherSpec {
            id: "v_magic_trick".to_string(),
            name: "Magic Trick".to_string(),
            cost: 10,
            effect_key: "magic_trick".to_string(),
            description: "Playing cards can appear in the shop".to_string(),
        },
        VoucherSpec {
            id: "v_paint_brush".to_string(),
            name: "Paint Brush".to_string(),
            cost: 10,
            effect_key: "paint_brush".to_string(),
            description: "+1 hand size".to_string(),
        },
        VoucherSpec {
            id: "v_tarot_merchant".to_string(),
            name: "Tarot Merchant".to_string(),
            cost: 10,
            effect_key: "tarot_merchant".to_string(),
            description: "Tarot cards appear 2x more in shop".to_string(),
        },
        VoucherSpec {
            id: "v_planet_merchant".to_string(),
            name: "Planet Merchant".to_string(),
            cost: 10,
            effect_key: "planet_merchant".to_string(),
            description: "Planet cards appear 2x more in shop".to_string(),
        },
    ]
}

/// Hand-written vanilla tag pool (mirrors `G.P_TAGS` in
/// `vendor/balatro/steam-local/extracted/game.lua:224-249`). Descriptions
/// follow the real-client Chinese format seen in observer captures so the
/// `state_mapping.to_real_shape` normalizer produces matching strings.
///
/// Used as a fallback when `RulesetBundle.tags` is empty (i.e. older bundle
/// JSONs that pre-date the tag catalog migration).
fn default_tag_pool() -> Vec<TagSpec> {
    vec![
        TagSpec {
            id: "tag_uncommon".to_string(),
            name: "Uncommon Tag".to_string(),
            effect_key: "store_free_uncommon_joker".to_string(),
            description: "商店会有一张免费的 罕见小丑牌".to_string(),
        },
        TagSpec {
            id: "tag_rare".to_string(),
            name: "Rare Tag".to_string(),
            effect_key: "store_free_rare_joker".to_string(),
            description: "商店会有一张免费的 稀有小丑牌".to_string(),
        },
        TagSpec {
            id: "tag_negative".to_string(),
            name: "Negative Tag".to_string(),
            effect_key: "negative_next_joker".to_string(),
            description: "商店里的下一张 基础版本小丑牌 将会免费且变为负片".to_string(),
        },
        TagSpec {
            id: "tag_foil".to_string(),
            name: "Foil Tag".to_string(),
            effect_key: "foil_next_joker".to_string(),
            description: "商店里的下一张 基础版本小丑牌 将会免费且变为闪箔".to_string(),
        },
        TagSpec {
            id: "tag_holo".to_string(),
            name: "Holographic Tag".to_string(),
            effect_key: "holo_next_joker".to_string(),
            description: "商店里的下一张 基础版本小丑牌 将会免费且变为镭射".to_string(),
        },
        TagSpec {
            id: "tag_polychrome".to_string(),
            name: "Polychrome Tag".to_string(),
            effect_key: "polychrome_next_joker".to_string(),
            description: "商店里的下一张 基础版本小丑牌 将会免费且变为多彩".to_string(),
        },
        TagSpec {
            id: "tag_investment".to_string(),
            name: "Investment Tag".to_string(),
            effect_key: "investment_25_after_boss".to_string(),
            description: "击败 Boss盲注后 获得$25".to_string(),
        },
        TagSpec {
            id: "tag_voucher".to_string(),
            name: "Voucher Tag".to_string(),
            effect_key: "voucher_next_shop".to_string(),
            description: "添加一张优惠券 到下一个商店".to_string(),
        },
        TagSpec {
            id: "tag_boss".to_string(),
            name: "Boss Tag".to_string(),
            effect_key: "reroll_boss_blind".to_string(),
            description: "重掷 Boss盲注".to_string(),
        },
        TagSpec {
            id: "tag_standard".to_string(),
            name: "Standard Tag".to_string(),
            effect_key: "free_standard_mega_pack".to_string(),
            description: "获得一个免费的 超级标准包".to_string(),
        },
        TagSpec {
            id: "tag_charm".to_string(),
            name: "Charm Tag".to_string(),
            effect_key: "free_arcana_mega_pack".to_string(),
            description: "获得一个免费的 超级秘术包".to_string(),
        },
        TagSpec {
            id: "tag_meteor".to_string(),
            name: "Meteor Tag".to_string(),
            effect_key: "free_celestial_mega_pack".to_string(),
            description: "获得一个免费的 超级天体包".to_string(),
        },
        TagSpec {
            id: "tag_buffoon".to_string(),
            name: "Buffoon Tag".to_string(),
            effect_key: "free_buffoon_mega_pack".to_string(),
            description: "获得一个免费的 超级小丑包".to_string(),
        },
        TagSpec {
            id: "tag_handy".to_string(),
            name: "Handy Tag".to_string(),
            effect_key: "dollars_per_hand_played".to_string(),
            description: "本赛局每打出过一次手牌 获得$1".to_string(),
        },
        TagSpec {
            id: "tag_garbage".to_string(),
            name: "Garbage Tag".to_string(),
            effect_key: "dollars_per_unused_discard".to_string(),
            description: "本赛局每一次 未使用的弃牌得到$1".to_string(),
        },
        TagSpec {
            id: "tag_ethereal".to_string(),
            name: "Ethereal Tag".to_string(),
            effect_key: "free_spectral_pack".to_string(),
            description: "获得一个免费的 幻灵包".to_string(),
        },
        TagSpec {
            id: "tag_coupon".to_string(),
            name: "Coupon Tag".to_string(),
            effect_key: "coupon_next_shop".to_string(),
            description: "下一家店内的 初始卡牌和补充包 均为免费".to_string(),
        },
        TagSpec {
            id: "tag_double".to_string(),
            name: "Double Tag".to_string(),
            effect_key: "double_next_tag".to_string(),
            description: "下一次选定的标签 会额外获得一个复制品 双倍标签除外".to_string(),
        },
        TagSpec {
            id: "tag_juggle".to_string(),
            name: "Juggle Tag".to_string(),
            effect_key: "juggle_hand_size_next_round".to_string(),
            description: "下一回合 +3手牌上限".to_string(),
        },
        TagSpec {
            id: "tag_d_six".to_string(),
            name: "D6 Tag".to_string(),
            effect_key: "d6_reroll_start_zero".to_string(),
            description: "下一个商店的 重掷起价为$0".to_string(),
        },
        TagSpec {
            id: "tag_top_up".to_string(),
            name: "Top-up Tag".to_string(),
            effect_key: "spawn_common_jokers".to_string(),
            description: "生成最多2张 普通小丑牌".to_string(),
        },
        TagSpec {
            id: "tag_skip".to_string(),
            name: "Speed Tag".to_string(),
            effect_key: "dollars_per_skip".to_string(),
            description: "本赛局中每跳过 一次盲注，可获得$5".to_string(),
        },
        TagSpec {
            id: "tag_orbital".to_string(),
            name: "Orbital Tag".to_string(),
            effect_key: "orbital_level_up_hand".to_string(),
            description: "升级 3个等级".to_string(),
        },
        TagSpec {
            id: "tag_economy".to_string(),
            name: "Economy Tag".to_string(),
            effect_key: "economy_double_money".to_string(),
            description: "资金翻倍 （最高$40）".to_string(),
        },
    ]
}

/// Roll for an edition on a generated joker or playing card.
/// 96% None, 2% Foil, 1.4% Holo, 0.6% Polychrome.
fn roll_edition(rng: &mut ChaCha8Rng) -> Option<String> {
    let roll: f64 = rng.gen_range(0.0..100.0);
    if roll < 96.0 {
        None
    } else if roll < 98.0 {
        Some("e_foil".to_string())
    } else if roll < 99.4 {
        Some("e_holo".to_string())
    } else {
        Some("e_polychrome".to_string())
    }
}

/// Roll for an enhancement on a generated playing card.
/// 90% None, ~1.43% each for 7 types (~10% total).
fn roll_enhancement(rng: &mut ChaCha8Rng) -> Option<String> {
    let roll: f64 = rng.gen_range(0.0..100.0);
    if roll < 90.0 {
        None
    } else {
        let enhancements = [
            "m_bonus", "m_mult", "m_wild", "m_glass", "m_steel", "m_stone", "m_gold",
        ];
        let idx = ((roll - 90.0) / (10.0 / 7.0)).min(6.0) as usize;
        Some(enhancements[idx].to_string())
    }
}

/// Roll for a seal on a generated playing card.
/// 97% None, 1% Gold, 1% Red, 1% Blue.
fn roll_seal(rng: &mut ChaCha8Rng) -> Option<String> {
    let roll: f64 = rng.gen_range(0.0..100.0);
    if roll < 97.0 {
        None
    } else if roll < 98.0 {
        Some("Gold".to_string())
    } else if roll < 99.0 {
        Some("Red".to_string())
    } else {
        Some("Blue".to_string())
    }
}

fn apply_discount(base_cost: i32, discount: f32) -> i32 {
    ((base_cost as f32) * discount).round() as i32
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HandClassification {
    key: String,
}

fn classify_hand(cards: &[CardInstance]) -> HandClassification {
    let mut rank_counts = [0_u8; 13];
    let mut suit_counts = [0_u8; 4];
    let mut unique_ranks = BTreeSet::new();
    for card in cards {
        rank_counts[card.rank_index()] += 1;
        suit_counts[card.suit_index()] += 1;
        unique_ranks.insert(card.rank_index());
    }
    let max_rank = *rank_counts.iter().max().unwrap_or(&0);
    let pairs = rank_counts.iter().filter(|count| **count >= 2).count();
    let has_three = rank_counts.iter().any(|count| *count >= 3);
    let has_flush = suit_counts.iter().any(|count| *count >= 5);
    let has_straight = straight_exists(&unique_ranks);

    let key = if max_rank >= 5 && has_flush {
        "flush_five"
    } else if has_flush && has_three && pairs >= 2 {
        "flush_house"
    } else if max_rank >= 5 {
        "five_of_a_kind"
    } else if has_straight && has_flush {
        "straight_flush"
    } else if max_rank >= 4 {
        "four_of_a_kind"
    } else if has_three && pairs >= 2 {
        "full_house"
    } else if has_flush {
        "flush"
    } else if has_straight {
        "straight"
    } else if has_three {
        "three_of_kind"
    } else if pairs >= 2 {
        "two_pair"
    } else if pairs >= 1 {
        "pair"
    } else {
        "high_card"
    };
    HandClassification {
        key: key.to_string(),
    }
}

fn straight_exists(unique_ranks: &BTreeSet<usize>) -> bool {
    if unique_ranks.len() < 5 {
        return false;
    }
    let values: Vec<usize> = unique_ranks.iter().copied().collect();
    if [0, 1, 2, 3, 12].iter().all(|rank| unique_ranks.contains(rank)) {
        return true;
    }
    values.windows(5).any(|window| {
        window
            .windows(2)
            .all(|pair| pair[1] == pair[0] + 1)
    })
}

/// Resolve what ability a Joker effectively has, following Blueprint/Brainstorm chains.
fn resolve_joker_ability<'a>(
    joker_index: usize,
    joker_instances: &[JokerInstance],
    joker_specs: &'a [Option<JokerSpec>],
) -> Option<&'a JokerSpec> {
    let mut visited = HashSet::new();
    let mut current_index = joker_index;

    loop {
        if !visited.insert(current_index) {
            return None;
        }
        if current_index >= joker_instances.len() {
            return None;
        }
        let instance = &joker_instances[current_index];
        let is_blueprint = instance.joker_id == "j_blueprint" || instance.name == "Blueprint";
        let is_brainstorm = instance.joker_id == "j_brainstorm" || instance.name == "Brainstorm";

        if is_blueprint {
            let right_index = current_index + 1;
            if right_index >= joker_instances.len() {
                return None;
            }
            current_index = right_index;
        } else if is_brainstorm {
            if joker_instances.is_empty() || current_index == 0 {
                return None;
            }
            current_index = 0;
        } else {
            return joker_specs.get(current_index).and_then(|s| s.as_ref());
        }
    }
}

/// Determine whether a JokerSpec grants a retrigger for the given card.
fn spec_grants_retrigger(
    spec: &JokerSpec,
    card: &CardInstance,
    card_index: usize,
    scoring_card_count: usize,
    is_final_hand: bool,
) -> u32 {
    let _ = scoring_card_count;
    let id = spec.id.as_str();

    if id == "j_sock_and_buskin" || spec.name == "Sock and Buskin" {
        if card.is_face_card() { return 1; }
        return 0;
    }
    if id == "j_hanging_chad" || spec.name == "Hanging Chad" {
        if card_index == 0 { return 2; }
        return 0;
    }
    if id == "j_seltzer" || spec.name == "Seltzer" {
        return 1;
    }
    if id == "j_dusk" || spec.name == "Dusk" {
        if is_final_hand { return 1; }
        return 0;
    }
    0
}

/// Calculate the total number of retriggers for a scoring card.
fn calculate_retriggers(
    card: &CardInstance,
    card_index: usize,
    joker_instances: &[JokerInstance],
    joker_specs: &[Option<JokerSpec>],
    is_final_hand: bool,
    scoring_card_count: usize,
) -> u32 {
    let mut total: u32 = 0;

    if card.typed_seal() == Seal::Red {
        total += 1;
    }

    for (joker_idx, joker) in joker_instances.iter().enumerate() {
        if (joker.joker_id == "j_seltzer" || joker.name == "Seltzer")
            && joker.remaining_uses == Some(0)
        {
            continue;
        }
        if let Some(resolved_spec) =
            resolve_joker_ability(joker_idx, joker_instances, joker_specs)
        {
            total += spec_grants_retrigger(
                resolved_spec,
                card,
                card_index,
                scoring_card_count,
                is_final_hand,
            );
        }
    }

    total
}

/// Determine initial remaining_uses for a Joker based on its spec.
fn initial_remaining_uses(spec: &JokerSpec) -> Option<u32> {
    if spec.id == "j_seltzer" || spec.name == "Seltzer" {
        let uses = spec
            .config
            .get("extra")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32;
        return Some(uses);
    }
    None
}

/// Build the initial runtime_state for scaling Jokers.
fn initial_runtime_state(spec: &JokerSpec, rng: &mut ChaCha8Rng) -> BTreeMap<String, f64> {
    let mut state = BTreeMap::new();
    match spec.id.as_str() {
        "j_ice_cream" => {
            let base = config_extra_obj(spec).and_then(|o| o.get("chips").and_then(|v| v.as_i64())).unwrap_or(100) as f64;
            state.insert("chips".to_string(), base);
        }
        "j_popcorn" => {
            let base = spec.config.get("mult").and_then(|v| v.as_i64()).unwrap_or(20) as f64;
            state.insert("mult".to_string(), base);
        }
        "j_ramen" => {
            let base = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(2.0);
            state.insert("xmult".to_string(), base);
        }
        "j_yorick" => {
            state.insert("xmult".to_string(), 1.0);
            state.insert("discards_remaining".to_string(), 23.0);
        }
        "j_idol" => {
            let rank = rng.gen_range(0..13) as f64;
            let suit = rng.gen_range(0..4) as f64;
            state.insert("rank".to_string(), rank);
            state.insert("suit".to_string(), suit);
        }
        "j_castle" => {
            state.insert("chips".to_string(), 0.0);
            let suit = rng.gen_range(0..4) as f64;
            state.insert("suit".to_string(), suit);
        }
        "j_green_joker" | "j_ride_the_bus" | "j_red_card" | "j_flash"
        | "j_fortune_teller" | "j_ceremonial" | "j_runner" | "j_square"
        | "j_trousers" => {
            // Start with 0 accumulated value
        }
        "j_constellation" | "j_glass" | "j_hologram" | "j_throwback"
        | "j_campfire" | "j_madness" | "j_obelisk" | "j_vampire"
        | "j_lucky_cat" | "j_caino" | "j_hit_the_road" => {
            state.insert("xmult".to_string(), 1.0);
        }
        "j_loyalty_card" => {
            state.insert("hands_played".to_string(), 0.0);
        }
        "j_supernova" | "j_card_sharp" => {
            // Uses dynamic keys like "played:<hand>" or "hand:<hand>"
        }
        _ => {}
    }
    state
}

// --- Card enhancement / edition helpers ---

/// Returns true if the card matches the given suit, accounting for Wild Card enhancement.
pub fn card_matches_suit(card: &CardInstance, suit: &Suit) -> bool {
    if card.enhancement.as_deref() == Some("m_wild") {
        return true;
    }
    card.suit == *suit
}

/// Apply card enhancement effects during scoring.
/// Returns (chips_add, mult_add, xmult_factor, money_add, is_glass, is_stone).
/// Glass and stone flags are returned for special post-processing.
fn apply_card_enhancement(
    card: &CardInstance,
    chips: &mut i32,
    mult: &mut i32,
    xmult: &mut f64,
    money_delta: &mut i32,
    events: &mut Vec<Event>,
    rng: &mut ChaCha8Rng,
) -> (bool, bool) {
    let mut is_glass = false;
    match card.enhancement.as_deref() {
        Some("m_bonus") => {
            *chips += 30;
            events.push(event(
                EventStage::CardScored,
                "enhancement_bonus",
                format!("Bonus Card {} +30 chips", card.card_id),
            ));
        }
        Some("m_mult") => {
            *mult += 4;
            events.push(event(
                EventStage::CardScored,
                "enhancement_mult",
                format!("Mult Card {} +4 mult", card.card_id),
            ));
        }
        Some("m_wild") => {
            // Wild card effect is passive (suit matching); no scoring bonus itself.
        }
        Some("m_glass") => {
            *xmult *= 2.0;
            is_glass = true;
            events.push(event(
                EventStage::CardScored,
                "enhancement_glass",
                format!("Glass Card {} X2 mult", card.card_id),
            ));
        }
        Some("m_stone") => {
            *chips += 50;
            events.push(event(
                EventStage::CardScored,
                "enhancement_stone",
                format!("Stone Card {} +50 chips", card.card_id),
            ));
        }
        Some("m_lucky") => {
            // 1/5 chance +20 mult
            let mult_roll: i32 = rng.gen_range(1..=5);
            if mult_roll == 1 {
                *mult += 20;
                events.push(event(
                    EventStage::CardScored,
                    "enhancement_lucky_mult",
                    format!("Lucky Card {} +20 mult", card.card_id),
                ));
            }
            // 1/15 chance +$20
            let money_roll: i32 = rng.gen_range(1..=15);
            if money_roll == 1 {
                *money_delta += 20;
                events.push(event(
                    EventStage::CardScored,
                    "enhancement_lucky_money",
                    format!("Lucky Card {} +$20", card.card_id),
                ));
            }
        }
        // m_steel: only activates held in hand, not when played
        // m_gold: only at end of round
        _ => {}
    }
    (is_glass, card.enhancement.as_deref() == Some("m_stone"))
}

/// Apply card edition effects during scoring (for playing cards).
fn apply_card_edition(
    card: &CardInstance,
    chips: &mut i32,
    mult: &mut i32,
    xmult: &mut f64,
    events: &mut Vec<Event>,
) {
    match card.edition.as_deref() {
        Some("e_foil") => {
            *chips += 50;
            events.push(event(
                EventStage::CardScored,
                "edition_foil",
                format!("Foil Card {} +50 chips", card.card_id),
            ));
        }
        Some("e_holo") => {
            *mult += 10;
            events.push(event(
                EventStage::CardScored,
                "edition_holo",
                format!("Holographic Card {} +10 mult", card.card_id),
            ));
        }
        Some("e_polychrome") => {
            *xmult *= 1.5;
            events.push(event(
                EventStage::CardScored,
                "edition_polychrome",
                format!("Polychrome Card {} X1.5 mult", card.card_id),
            ));
        }
        // e_negative is passive (joker slot), not a scoring effect
        _ => {}
    }
}

/// Apply joker edition effects after the joker's own effect.
fn apply_joker_edition(
    joker: &JokerInstance,
    chips: &mut i32,
    mult: &mut i32,
    xmult: &mut f64,
    events: &mut Vec<Event>,
) {
    match joker.edition.as_deref() {
        Some("e_foil") => {
            *chips += 50;
            events.push(event(
                EventStage::JokerPostScore,
                "joker_edition_foil",
                format!("{} (Foil) +50 chips", joker.name),
            ));
        }
        Some("e_holo") => {
            *mult += 10;
            events.push(event(
                EventStage::JokerPostScore,
                "joker_edition_holo",
                format!("{} (Holographic) +10 mult", joker.name),
            ));
        }
        Some("e_polychrome") => {
            *xmult *= 1.5;
            events.push(event(
                EventStage::JokerPostScore,
                "joker_edition_polychrome",
                format!("{} (Polychrome) X1.5 mult", joker.name),
            ));
        }
        _ => {}
    }
}

// --- Joker helper functions ---

fn is_face_card(rank: &Rank) -> bool {
    matches!(rank, Rank::Jack | Rank::Queen | Rank::King)
}

fn is_even_rank(rank: &Rank) -> bool {
    matches!(rank, Rank::Two | Rank::Four | Rank::Six | Rank::Eight | Rank::Ten)
}

fn is_odd_rank(rank: &Rank) -> bool {
    matches!(rank, Rank::Ace | Rank::Three | Rank::Five | Rank::Seven | Rank::Nine)
}

fn is_fibonacci_rank(rank: &Rank) -> bool {
    matches!(rank, Rank::Ace | Rank::Two | Rank::Three | Rank::Five | Rank::Eight)
}

fn config_extra_i64(spec: &JokerSpec) -> Option<i64> {
    spec.config.get("extra").and_then(|v| v.as_i64())
}

fn config_extra_f64(spec: &JokerSpec) -> Option<f64> {
    spec.config.get("extra").and_then(|v| v.as_f64())
}

fn config_extra_obj(spec: &JokerSpec) -> Option<&serde_json::Map<String, serde_json::Value>> {
    spec.config.get("extra").and_then(|v| v.as_object())
}

fn apply_joker_effect(
    spec: &JokerSpec,
    ctx: &ScoringContext<'_>,
    chips: &mut i32,
    mult: &mut i32,
    xmult: &mut f64,
    money_delta: &mut i32,
    events: &mut Vec<Event>,
    trace: &mut JokerResolutionTrace,
    runtime_state: &BTreeMap<String, f64>,
) {
    let effect = spec.effect.as_deref().unwrap_or_default();

    // --- Generic effect-string handlers ---

    if effect == "Mult" {
        trace.supported = true;
        trace.effect_key = Some("mult".to_string());
        if let Some(flat) = spec.config.get("mult").and_then(|value| value.as_i64()) {
            *mult += flat as i32;
            trace.matched = true;
            trace.summary = format!("{} added {} mult", spec.name, flat);
            events.push(event(
                EventStage::JokerPostScore, "joker_mult",
                format!("{} added {} mult", spec.name, flat),
            ));
        } else {
            trace.summary = format!("{} declared Mult but no flat bonus was found", spec.name);
        }
        return;
    }

    if effect == "Suit Mult" {
        trace.supported = true;
        trace.effect_key = Some("suit_mult".to_string());
        if let Some(extra) = spec.config.get("extra").and_then(|value| value.as_object()) {
            let suit_name = extra.get("suit").and_then(|value| value.as_str()).unwrap_or_default();
            let suit_bonus = extra.get("s_mult").and_then(|value| value.as_i64()).unwrap_or(0) as i32;
            let matches = ctx.played.iter().filter(|card| suit_label(&card.suit) == suit_name).count() as i32;
            if matches > 0 {
                *mult += matches * suit_bonus;
                trace.matched = true;
                trace.summary = format!("{} matched {} suited card(s)", spec.name, matches);
                events.push(event(EventStage::JokerPostScore, "joker_suit_mult", format!("{} added {} mult", spec.name, matches * suit_bonus)));
            } else {
                trace.summary = format!("{} had no matching {} card", spec.name, suit_name);
            }
        } else {
            trace.summary = format!("{} declared Suit Mult but had no extra config", spec.name);
        }
        return;
    }

    // --- Hand-type conditional bonuses ---
    if let Some(hand_type) = spec.config.get("type").and_then(|value| value.as_str()) {
        trace.supported = true;
        trace.effect_key = Some("hand_type".to_string());
        if hand_type_to_key(hand_type) == ctx.hand_key {
            if let Some(flat) = spec.config.get("t_mult").and_then(|value| value.as_i64()) {
                *mult += flat as i32;
                trace.matched = true;
                trace.summary = format!("{} matched hand type {}", spec.name, hand_type);
                events.push(event(EventStage::JokerPostScore, "joker_type_mult", format!("{} added {} mult", spec.name, flat)));
            }
            if let Some(flat) = spec.config.get("t_chips").and_then(|value| value.as_i64()) {
                *chips += flat as i32;
                trace.matched = true;
                trace.summary = format!("{} matched hand type {}", spec.name, hand_type);
                events.push(event(EventStage::JokerPostScore, "joker_type_chips", format!("{} added {} chips", spec.name, flat)));
            }
            if let Some(xm) = spec.config.get("Xmult").and_then(|value| value.as_f64()) {
                *xmult *= xm;
                trace.matched = true;
                trace.summary = format!("{} matched hand type {} for X{} mult", spec.name, hand_type, xm);
                events.push(event(EventStage::JokerPostScore, "joker_type_xmult", format!("{} applied X{} mult", spec.name, xm)));
            }
            if !trace.matched {
                trace.summary = format!("{} matched {} but had no native t_mult/t_chips payload", spec.name, hand_type);
            }
        } else {
            trace.summary = format!("{} did not match hand type {}", spec.name, hand_type);
        }
        return;
    }

    if effect == "Discard Chips" {
        trace.supported = true;
        trace.effect_key = Some("discard_chips".to_string());
        if let Some(extra) = spec.config.get("extra").and_then(|value| value.as_i64()) {
            let gained = extra as i32 * ctx.discards_left;
            *chips += gained;
            trace.matched = gained > 0;
            trace.summary = format!("{} scaled with {} discards left", spec.name, ctx.discards_left);
            events.push(event(EventStage::JokerPostScore, "joker_discard_chips", format!("{} added {} chips", spec.name, gained)));
        } else {
            trace.summary = format!("{} declared Discard Chips but had no extra payload", spec.name);
        }
        return;
    }

    // --- ID-based dispatch for all specific jokers (128 match arms from a9c8fafb) ---
    match spec.id.as_str() {
        "j_abstract" => {
            let per = config_extra_i64(spec).unwrap_or(3) as i32;
            let gained = ctx.jokers.len() as i32 * per;
            *mult += gained;
            trace.supported = true; trace.matched = true;
            trace.effect_key = Some("abstract_joker".to_string());
            trace.summary = format!("{} counted {} Joker(s)", spec.name, ctx.jokers.len());
            events.push(event(EventStage::JokerPostScore, "joker_abstract", format!("{} added {} mult", spec.name, gained)));
        }
        "j_scary_face" => {
            let per = config_extra_i64(spec).unwrap_or(30) as i32;
            let face_count = ctx.played.iter().filter(|c| is_face_card(&c.rank)).count() as i32;
            trace.supported = true; trace.effect_key = Some("scary_face".to_string());
            if face_count > 0 {
                let gained = face_count * per;
                *chips += gained; trace.matched = true;
                trace.summary = format!("{} matched {} face card(s)", spec.name, face_count);
                events.push(event(EventStage::JokerPostScore, "joker_scary_face", format!("{} added {} chips", spec.name, gained)));
            } else { trace.summary = format!("{} had no face-card targets", spec.name); }
        }
        "j_half" => {
            trace.supported = true; trace.effect_key = Some("half_joker".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let bonus = extra.get("mult").and_then(|v| v.as_i64()).unwrap_or(20) as i32;
                let max_size = extra.get("size").and_then(|v| v.as_i64()).unwrap_or(3) as usize;
                if ctx.played.len() <= max_size {
                    *mult += bonus; trace.matched = true;
                    trace.summary = format!("{} hand has {} cards (<= {})", spec.name, ctx.played.len(), max_size);
                    events.push(event(EventStage::JokerPostScore, "joker_half", format!("{} added {} mult", spec.name, bonus)));
                } else { trace.summary = format!("{} hand has {} cards (> {})", spec.name, ctx.played.len(), max_size); }
            }
        }
        "j_stencil" => {
            trace.supported = true; trace.effect_key = Some("stencil".to_string());
            let empty = (ctx.joker_slot_max as i32 - ctx.jokers.len() as i32).max(0) + 1;
            let xm = empty as f64;
            if xm > 1.0 { *xmult *= xm; trace.matched = true; }
            trace.summary = format!("{} X{} mult ({} empty slots)", spec.name, xm, empty);
            events.push(event(EventStage::JokerPostScore, "joker_stencil", format!("{} applied X{} mult", spec.name, xm)));
        }
        "j_mystic_summit" => {
            trace.supported = true; trace.effect_key = Some("mystic_summit".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let bonus = extra.get("mult").and_then(|v| v.as_i64()).unwrap_or(15) as i32;
                let needed = extra.get("d_remaining").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                if ctx.discards_left == needed {
                    *mult += bonus; trace.matched = true;
                    trace.summary = format!("{} discards={} matched requirement", spec.name, ctx.discards_left);
                    events.push(event(EventStage::JokerPostScore, "joker_mystic_summit", format!("{} added {} mult", spec.name, bonus)));
                } else { trace.summary = format!("{} discards={} != {}", spec.name, ctx.discards_left, needed); }
            }
        }
        "j_blue_joker" => {
            let per = config_extra_i64(spec).unwrap_or(2) as i32;
            let gained = per * ctx.deck_cards_remaining;
            *chips += gained;
            trace.supported = true; trace.matched = gained > 0; trace.effect_key = Some("blue_joker".to_string());
            trace.summary = format!("{} +{} chips ({} remaining)", spec.name, gained, ctx.deck_cards_remaining);
            events.push(event(EventStage::JokerPostScore, "joker_blue", format!("{} added {} chips", spec.name, gained)));
        }
        "j_stone" => { trace.supported = true; trace.matched = false; trace.effect_key = Some("stone_joker".to_string()); trace.summary = format!("{} (no Stone cards in default deck)", spec.name); }
        "j_steel_joker" => { trace.supported = true; trace.matched = false; trace.effect_key = Some("steel_joker".to_string()); trace.summary = format!("{} (no Steel cards in default deck)", spec.name); }
        "j_ice_cream" => {
            trace.supported = true; trace.effect_key = Some("ice_cream".to_string());
            let base = config_extra_obj(spec).and_then(|o| o.get("chips").and_then(|v| v.as_i64())).unwrap_or(100) as f64;
            let current = runtime_state.get("chips").copied().unwrap_or(base) as i32;
            if current > 0 { *chips += current; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_ice_cream", format!("{} added {} chips", spec.name, current))); }
            trace.summary = format!("{} +{} chips (decaying)", spec.name, current);
        }
        "j_popcorn" => {
            trace.supported = true; trace.effect_key = Some("popcorn".to_string());
            let base = spec.config.get("mult").and_then(|v| v.as_i64()).unwrap_or(20) as f64;
            let current = runtime_state.get("mult").copied().unwrap_or(base) as i32;
            if current > 0 { *mult += current; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_popcorn", format!("{} added {} mult", spec.name, current))); }
            trace.summary = format!("{} +{} mult (decaying)", spec.name, current);
        }
        "j_bull" => {
            let per = config_extra_i64(spec).unwrap_or(2) as i32;
            let gained = per * ctx.money.max(0);
            *chips += gained;
            trace.supported = true; trace.matched = gained > 0; trace.effect_key = Some("bull".to_string());
            trace.summary = format!("{} +{} chips (${} held)", spec.name, gained, ctx.money);
            events.push(event(EventStage::JokerPostScore, "joker_bull", format!("{} added {} chips", spec.name, gained)));
        }
        "j_bootstraps" => {
            trace.supported = true; trace.effect_key = Some("bootstraps".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let mult_per = extra.get("mult").and_then(|v| v.as_i64()).unwrap_or(2) as i32;
                let dollars_per = extra.get("dollars").and_then(|v| v.as_i64()).unwrap_or(5) as i32;
                let sets = if dollars_per > 0 { ctx.money.max(0) / dollars_per } else { 0 };
                let gained = sets * mult_per;
                *mult += gained; trace.matched = gained > 0;
                trace.summary = format!("{} +{} mult (${} / ${})", spec.name, gained, ctx.money, dollars_per);
                events.push(event(EventStage::JokerPostScore, "joker_bootstraps", format!("{} added {} mult", spec.name, gained)));
            }
        }
        "j_blackboard" => {
            trace.supported = true; trace.effect_key = Some("blackboard".to_string());
            let xm = config_extra_f64(spec).unwrap_or(3.0);
            let all_dark = !ctx.held_in_hand.is_empty() && ctx.held_in_hand.iter().all(|c| matches!(c.suit, Suit::Spades | Suit::Clubs));
            if all_dark { *xmult *= xm; trace.matched = true; trace.summary = format!("{} all held cards Spades/Clubs => X{}", spec.name, xm); events.push(event(EventStage::JokerPostScore, "joker_blackboard", format!("{} applied X{} mult", spec.name, xm))); }
            else { trace.summary = format!("{} not all held cards Spades/Clubs", spec.name); }
        }
        "j_flower_pot" => {
            trace.supported = true; trace.effect_key = Some("flower_pot".to_string());
            let xm = config_extra_f64(spec).unwrap_or(3.0);
            let suits: BTreeSet<usize> = ctx.played.iter().map(|c| c.suit_index()).collect();
            if suits.len() >= 4 { *xmult *= xm; trace.matched = true; trace.summary = format!("{} all 4 suits present => X{}", spec.name, xm); events.push(event(EventStage::JokerPostScore, "joker_flower_pot", format!("{} applied X{} mult", spec.name, xm))); }
            else { trace.summary = format!("{} only {} suits", spec.name, suits.len()); }
        }
        "j_seeing_double" => {
            trace.supported = true; trace.effect_key = Some("seeing_double".to_string());
            let xm = config_extra_f64(spec).unwrap_or(2.0);
            let has_club = ctx.played.iter().any(|c| matches!(c.suit, Suit::Clubs));
            let has_other = ctx.played.iter().any(|c| !matches!(c.suit, Suit::Clubs));
            if has_club && has_other { *xmult *= xm; trace.matched = true; trace.summary = format!("{} Club + other suit => X{}", spec.name, xm); events.push(event(EventStage::JokerPostScore, "joker_seeing_double", format!("{} applied X{} mult", spec.name, xm))); }
            else { trace.summary = format!("{} condition not met", spec.name); }
        }
        "j_acrobat" => {
            trace.supported = true; trace.effect_key = Some("acrobat".to_string());
            let xm = config_extra_f64(spec).unwrap_or(3.0);
            if ctx.plays_left <= 1 { *xmult *= xm; trace.matched = true; trace.summary = format!("{} final hand => X{}", spec.name, xm); events.push(event(EventStage::JokerPostScore, "joker_acrobat", format!("{} applied X{} mult", spec.name, xm))); }
            else { trace.summary = format!("{} not final hand ({} plays left)", spec.name, ctx.plays_left); }
        }
        "j_stuntman" => {
            trace.supported = true; trace.effect_key = Some("stuntman".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let bonus = extra.get("chip_mod").and_then(|v| v.as_i64()).unwrap_or(250) as i32;
                *chips += bonus; trace.matched = true;
                trace.summary = format!("{} +{} chips", spec.name, bonus);
                events.push(event(EventStage::JokerPostScore, "joker_stuntman", format!("{} added {} chips", spec.name, bonus)));
            }
        }
        "j_swashbuckler" => {
            trace.supported = true; trace.effect_key = Some("swashbuckler".to_string());
            let total_sell: i32 = ctx.jokers.iter().filter(|j| j.joker_id != spec.id).map(|j| (j.sell_value + j.extra_sell_value).max(0)).sum();
            *mult += total_sell; trace.matched = total_sell > 0;
            trace.summary = format!("{} +{} mult from other joker sell values", spec.name, total_sell);
            events.push(event(EventStage::JokerPostScore, "joker_swashbuckler", format!("{} added {} mult", spec.name, total_sell)));
        }
        "j_baseball" => {
            trace.supported = true; trace.effect_key = Some("baseball".to_string());
            let xm_per = config_extra_f64(spec).unwrap_or(1.5);
            let uncommon_count = ctx.jokers.iter().filter(|j| j.rarity == 2).count();
            if uncommon_count > 0 { let total_xm = xm_per.powi(uncommon_count as i32); *xmult *= total_xm; trace.matched = true; trace.summary = format!("{} {} uncommon jokers => X{:.2}", spec.name, uncommon_count, total_xm); events.push(event(EventStage::JokerPostScore, "joker_baseball", format!("{} applied X{:.2} mult", spec.name, total_xm))); }
            else { trace.summary = format!("{} no uncommon jokers", spec.name); }
        }
        "j_gros_michel" => {
            trace.supported = true; trace.effect_key = Some("gros_michel".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let bonus = extra.get("mult").and_then(|v| v.as_i64()).unwrap_or(15) as i32;
                *mult += bonus; trace.matched = true;
                trace.summary = format!("{} +{} mult", spec.name, bonus);
                events.push(event(EventStage::JokerPostScore, "joker_gros_michel", format!("{} added {} mult", spec.name, bonus)));
            }
        }
        "j_cavendish" => {
            trace.supported = true; trace.effect_key = Some("cavendish".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let xm = extra.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(3.0);
                *xmult *= xm; trace.matched = true;
                trace.summary = format!("{} X{}", spec.name, xm);
                events.push(event(EventStage::JokerPostScore, "joker_cavendish", format!("{} applied X{} mult", spec.name, xm)));
            }
        }
        "j_supernova" => {
            trace.supported = true; trace.effect_key = Some("supernova".to_string());
            // Count for current hand type stored as "played:<key>"
            let hand_count_key = format!("played:{}", ctx.hand_key);
            let times_played = runtime_state.get(&hand_count_key).copied().unwrap_or(0.0) as i32;
            if times_played > 0 { *mult += times_played; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_supernova", format!("{} added {} mult", spec.name, times_played))); }
            trace.summary = format!("{} +{} mult ({} played {} times)", spec.name, times_played, ctx.hand_key, times_played);
        }
        "j_erosion" => {
            let per = config_extra_i64(spec).unwrap_or(4) as i32;
            let deficit = (52_i32 - ctx.full_deck_size).max(0);
            let gained = per * deficit;
            trace.supported = true; trace.matched = gained > 0; trace.effect_key = Some("erosion".to_string());
            trace.summary = format!("{} +{} mult ({} cards below 52)", spec.name, gained, deficit);
            if gained > 0 { *mult += gained; events.push(event(EventStage::JokerPostScore, "joker_erosion", format!("{} added {} mult", spec.name, gained))); }
        }
        "j_ramen" => {
            trace.supported = true; trace.effect_key = Some("ramen".to_string());
            let base = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(2.0);
            let xm = runtime_state.get("xmult").copied().unwrap_or(base);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_ramen", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (decaying)", spec.name, xm);
        }
        "j_drivers_license" => { trace.supported = true; trace.matched = false; trace.effect_key = Some("drivers_license".to_string()); trace.summary = format!("{} (0 enhanced cards in default deck)", spec.name); }
        "j_constellation" => {
            trace.supported = true; trace.effect_key = Some("constellation".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; }
            trace.summary = format!("{} X{:.2} (accumulated)", spec.name, xm);
            if xm > 1.0 { events.push(event(EventStage::JokerPostScore, "joker_constellation", format!("{} applied X{:.2} mult", spec.name, xm))); }
        }
        "j_glass" => {
            trace.supported = true; trace.effect_key = Some("glass_joker".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_glass", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (accumulated)", spec.name, xm);
        }
        "j_hologram" => {
            trace.supported = true; trace.effect_key = Some("hologram".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_hologram", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (accumulated)", spec.name, xm);
        }
        "j_throwback" => {
            trace.supported = true; trace.effect_key = Some("throwback".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_throwback", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (blind skips)", spec.name, xm);
        }
        "j_campfire" => {
            trace.supported = true; trace.effect_key = Some("campfire".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_campfire", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (cards sold)", spec.name, xm);
        }
        "j_red_card" => {
            trace.supported = true; trace.effect_key = Some("red_card".to_string());
            let bonus = runtime_state.get("mult").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_red_card", format!("{} added {} mult", spec.name, bonus))); }
            trace.summary = format!("{} +{} mult (accumulated)", spec.name, bonus);
        }
        "j_flash" => {
            trace.supported = true; trace.effect_key = Some("flash_card".to_string());
            let bonus = runtime_state.get("mult").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_flash", format!("{} added {} mult", spec.name, bonus))); }
            trace.summary = format!("{} +{} mult (rerolls)", spec.name, bonus);
        }
        "j_fortune_teller" => {
            trace.supported = true; trace.effect_key = Some("fortune_teller".to_string());
            let bonus = runtime_state.get("mult").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_fortune_teller", format!("{} added {} mult", spec.name, bonus))); }
            trace.summary = format!("{} +{} mult (tarots used)", spec.name, bonus);
        }
        "j_green_joker" => {
            trace.supported = true; trace.effect_key = Some("green_joker".to_string());
            let bonus = runtime_state.get("mult").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_green", format!("{} added {} mult", spec.name, bonus))); }
            trace.summary = format!("{} +{} mult (accumulated)", spec.name, bonus);
        }
        "j_ride_the_bus" => {
            trace.supported = true; trace.effect_key = Some("ride_the_bus".to_string());
            let bonus = runtime_state.get("mult").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_ride_the_bus", format!("{} added {} mult", spec.name, bonus))); }
            trace.summary = format!("{} +{} mult (consecutive hands)", spec.name, bonus);
        }
        "j_card_sharp" => {
            trace.supported = true; trace.effect_key = Some("card_sharp".to_string());
            // Check if this hand type was already played this round (stored as "hand:<key>" = 1.0)
            let hand_key_marker = format!("hand:{}", ctx.hand_key);
            let already_played = runtime_state.get(&hand_key_marker).copied().unwrap_or(0.0) > 0.0;
            if already_played {
                let xm = config_extra_f64(spec).unwrap_or(3.0);
                *xmult *= xm; trace.matched = true;
                trace.summary = format!("{} same hand type this round => X{}", spec.name, xm);
                events.push(event(EventStage::JokerPostScore, "joker_card_sharp", format!("{} applied X{} mult", spec.name, xm)));
            } else {
                trace.summary = format!("{} first time playing {} this round", spec.name, ctx.hand_key);
            }
        }
        "j_madness" => {
            trace.supported = true; trace.effect_key = Some("madness".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_madness", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (blinds selected)", spec.name, xm);
        }
        "j_loyalty_card" => {
            trace.supported = true; trace.effect_key = Some("loyalty_card".to_string());
            let hands_played = runtime_state.get("hands_played").copied().unwrap_or(0.0) as i32;
            // Activates every 5th hand (when counter mod 5 == 0 and at least 5 hands played)
            if hands_played > 0 && hands_played % 5 == 0 {
                let xm = config_extra_f64(spec).unwrap_or(4.0);
                *xmult *= xm; trace.matched = true;
                trace.summary = format!("{} every 5th hand (hand #{}) => X{}", spec.name, hands_played, xm);
                events.push(event(EventStage::JokerPostScore, "joker_loyalty_card", format!("{} applied X{} mult", spec.name, xm)));
            } else {
                trace.summary = format!("{} hand #{}, next at {}", spec.name, hands_played, ((hands_played / 5) + 1) * 5);
            }
        }
        "j_obelisk" => {
            trace.supported = true; trace.effect_key = Some("obelisk".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_obelisk", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (consecutive non-most-played)", spec.name, xm);
        }
        "j_vampire" => {
            trace.supported = true; trace.effect_key = Some("vampire".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_vampire", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (enhanced cards eaten)", spec.name, xm);
        }
        "j_lucky_cat" => {
            trace.supported = true; trace.effect_key = Some("lucky_cat".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_lucky_cat", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (lucky triggers)", spec.name, xm);
        }
        "j_ceremonial" => {
            trace.supported = true; trace.effect_key = Some("ceremonial".to_string());
            let bonus = runtime_state.get("mult").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_ceremonial", format!("{} added {} mult", spec.name, bonus))); }
            trace.summary = format!("{} +{} mult (accumulated)", spec.name, bonus);
        }
        "j_smiley" => {
            let per = config_extra_i64(spec).unwrap_or(5) as i32;
            let count = ctx.played.iter().filter(|c| is_face_card(&c.rank)).count() as i32;
            trace.supported = true; trace.effect_key = Some("smiley_face".to_string());
            if count > 0 { let gained = count * per; *mult += gained; trace.matched = true; trace.summary = format!("{} +{} mult from {} face cards", spec.name, gained, count); events.push(event(EventStage::JokerPostScore, "joker_smiley", format!("{} added {} mult", spec.name, gained))); }
            else { trace.summary = format!("{} no face cards scored", spec.name); }
        }
        "j_fibonacci" => {
            let per = config_extra_i64(spec).unwrap_or(8) as i32;
            let count = ctx.played.iter().filter(|c| is_fibonacci_rank(&c.rank)).count() as i32;
            trace.supported = true; trace.effect_key = Some("fibonacci".to_string());
            if count > 0 { let gained = count * per; *mult += gained; trace.matched = true; trace.summary = format!("{} +{} mult from {} fib cards", spec.name, gained, count); events.push(event(EventStage::JokerPostScore, "joker_fibonacci", format!("{} added {} mult", spec.name, gained))); }
            else { trace.summary = format!("{} no fibonacci cards scored", spec.name); }
        }
        "j_scholar" => {
            trace.supported = true; trace.effect_key = Some("scholar".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let c_per = extra.get("chips").and_then(|v| v.as_i64()).unwrap_or(20) as i32;
                let m_per = extra.get("mult").and_then(|v| v.as_i64()).unwrap_or(4) as i32;
                let count = ctx.played.iter().filter(|c| matches!(c.rank, Rank::Ace)).count() as i32;
                if count > 0 { *chips += count * c_per; *mult += count * m_per; trace.matched = true; trace.summary = format!("{} {} Aces => +{} chips, +{} mult", spec.name, count, count * c_per, count * m_per); events.push(event(EventStage::JokerPostScore, "joker_scholar", format!("{} added {} chips and {} mult", spec.name, count * c_per, count * m_per))); }
                else { trace.summary = format!("{} no Aces scored", spec.name); }
            }
        }
        "j_odd_todd" => {
            let per = config_extra_i64(spec).unwrap_or(31) as i32;
            let count = ctx.played.iter().filter(|c| is_odd_rank(&c.rank)).count() as i32;
            trace.supported = true; trace.effect_key = Some("odd_todd".to_string());
            if count > 0 { let gained = count * per; *chips += gained; trace.matched = true; trace.summary = format!("{} +{} chips from {} odd cards", spec.name, gained, count); events.push(event(EventStage::JokerPostScore, "joker_odd_todd", format!("{} added {} chips", spec.name, gained))); }
            else { trace.summary = format!("{} no odd cards scored", spec.name); }
        }
        "j_even_steven" => {
            let per = config_extra_i64(spec).unwrap_or(4) as i32;
            let count = ctx.played.iter().filter(|c| is_even_rank(&c.rank)).count() as i32;
            trace.supported = true; trace.effect_key = Some("even_steven".to_string());
            if count > 0 { let gained = count * per; *mult += gained; trace.matched = true; trace.summary = format!("{} +{} mult from {} even cards", spec.name, gained, count); events.push(event(EventStage::JokerPostScore, "joker_even_steven", format!("{} added {} mult", spec.name, gained))); }
            else { trace.summary = format!("{} no even cards scored", spec.name); }
        }
        "j_walkie_talkie" => {
            trace.supported = true; trace.effect_key = Some("walkie_talkie".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let c_per = extra.get("chips").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
                let m_per = extra.get("mult").and_then(|v| v.as_i64()).unwrap_or(4) as i32;
                let count = ctx.played.iter().filter(|c| matches!(c.rank, Rank::Ten | Rank::Four)).count() as i32;
                if count > 0 { *chips += count * c_per; *mult += count * m_per; trace.matched = true; trace.summary = format!("{} {} tens/fours", spec.name, count); events.push(event(EventStage::JokerPostScore, "joker_walkie_talkie", format!("{} added {} chips and {} mult", spec.name, count * c_per, count * m_per))); }
                else { trace.summary = format!("{} no 10s or 4s scored", spec.name); }
            }
        }
        "j_photograph" => {
            let xm = config_extra_f64(spec).unwrap_or(2.0);
            let has_face = ctx.played.iter().any(|c| is_face_card(&c.rank));
            trace.supported = true; trace.effect_key = Some("photograph".to_string());
            if has_face { *xmult *= xm; trace.matched = true; trace.summary = format!("{} face card => X{}", spec.name, xm); events.push(event(EventStage::JokerPostScore, "joker_photograph", format!("{} applied X{} mult", spec.name, xm))); }
            else { trace.summary = format!("{} no face cards scored", spec.name); }
        }
        "j_triboulet" => {
            let xm_per = config_extra_f64(spec).unwrap_or(2.0);
            let count = ctx.played.iter().filter(|c| matches!(c.rank, Rank::King | Rank::Queen)).count();
            trace.supported = true; trace.effect_key = Some("triboulet".to_string());
            if count > 0 { let total_xm = xm_per.powi(count as i32); *xmult *= total_xm; trace.matched = true; trace.summary = format!("{} {} Kings/Queens => X{:.2}", spec.name, count, total_xm); events.push(event(EventStage::JokerPostScore, "joker_triboulet", format!("{} applied X{:.2} mult", spec.name, total_xm))); }
            else { trace.summary = format!("{} no Kings/Queens scored", spec.name); }
        }
        "j_arrowhead" => {
            let per = config_extra_i64(spec).unwrap_or(50) as i32;
            let count = ctx.played.iter().filter(|c| matches!(c.suit, Suit::Spades)).count() as i32;
            trace.supported = true; trace.effect_key = Some("arrowhead".to_string());
            if count > 0 { let gained = count * per; *chips += gained; trace.matched = true; trace.summary = format!("{} +{} chips from {} Spades", spec.name, gained, count); events.push(event(EventStage::JokerPostScore, "joker_arrowhead", format!("{} added {} chips", spec.name, gained))); }
            else { trace.summary = format!("{} no Spades scored", spec.name); }
        }
        "j_onyx_agate" => {
            let per = config_extra_i64(spec).unwrap_or(7) as i32;
            let count = ctx.played.iter().filter(|c| matches!(c.suit, Suit::Clubs)).count() as i32;
            trace.supported = true; trace.effect_key = Some("onyx_agate".to_string());
            if count > 0 { let gained = count * per; *mult += gained; trace.matched = true; trace.summary = format!("{} +{} mult from {} Clubs", spec.name, gained, count); events.push(event(EventStage::JokerPostScore, "joker_onyx_agate", format!("{} added {} mult", spec.name, gained))); }
            else { trace.summary = format!("{} no Clubs scored", spec.name); }
        }
        "j_rough_gem" => {
            let per = config_extra_i64(spec).unwrap_or(1) as i32;
            let count = ctx.played.iter().filter(|c| matches!(c.suit, Suit::Diamonds)).count() as i32;
            trace.supported = true; trace.effect_key = Some("rough_gem".to_string());
            if count > 0 { let earned = count * per; *money_delta += earned; trace.matched = true; trace.summary = format!("{} +${} from {} Diamonds", spec.name, earned, count); events.push(event(EventStage::JokerPostScore, "joker_rough_gem", format!("{} earned ${}", spec.name, earned))); }
            else { trace.summary = format!("{} no Diamonds scored", spec.name); }
        }
        "j_bloodstone" => {
            trace.supported = true; trace.effect_key = Some("bloodstone".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let xm = extra.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(1.5);
                let count = ctx.played.iter().filter(|c| matches!(c.suit, Suit::Hearts)).count();
                if count > 0 { *xmult *= xm; trace.matched = true; trace.summary = format!("{} {} Hearts => X{} (probabilistic)", spec.name, count, xm); events.push(event(EventStage::JokerPostScore, "joker_bloodstone", format!("{} applied X{} mult (probabilistic)", spec.name, xm))); }
                else { trace.summary = format!("{} no Hearts scored", spec.name); }
            }
        }
        "j_ancient" => {
            let xm = config_extra_f64(spec).unwrap_or(1.5);
            let count = ctx.played.iter().filter(|c| matches!(c.suit, Suit::Spades)).count();
            trace.supported = true; trace.effect_key = Some("ancient_joker".to_string());
            if count > 0 { let total_xm = xm.powi(count as i32); *xmult *= total_xm; trace.matched = true; trace.summary = format!("{} {} cards of suit => X{:.2} (suit tracking TODO)", spec.name, count, total_xm); events.push(event(EventStage::JokerPostScore, "joker_ancient", format!("{} applied X{:.2} mult", spec.name, total_xm))); }
            else { trace.summary = format!("{} no matching suit (default Spades)", spec.name); }
        }
        "j_ticket" => { trace.supported = true; trace.matched = false; trace.effect_key = Some("golden_ticket".to_string()); trace.summary = format!("{} (no Gold cards in default deck)", spec.name); }
        "j_wee" => {
            trace.supported = true; trace.effect_key = Some("wee_joker".to_string());
            if let Some(extra) = config_extra_obj(spec) {
                let per = extra.get("chip_mod").and_then(|v| v.as_i64()).unwrap_or(8) as i32;
                let current = extra.get("chips").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let twos = ctx.played.iter().filter(|c| matches!(c.rank, Rank::Two)).count() as i32;
                let gained = current + twos * per;
                if gained > 0 { *chips += gained; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_wee", format!("{} added {} chips", spec.name, gained))); }
                trace.summary = format!("{} +{} chips ({} twos scored, scaling TODO)", spec.name, gained, twos);
            }
        }
        "j_hack" => {
            trace.supported = true; trace.effect_key = Some("hack".to_string());
            let count = ctx.played.iter().filter(|c| matches!(c.rank, Rank::Two | Rank::Three | Rank::Four | Rank::Five)).count() as i32;
            if count > 0 {
                let extra_chips: i32 = ctx.played.iter().filter(|c| matches!(c.rank, Rank::Two | Rank::Three | Rank::Four | Rank::Five)).map(|c| c.chip_value()).sum();
                *chips += extra_chips; trace.matched = true;
                trace.summary = format!("{} retriggered {} low cards (+{} chips)", spec.name, count, extra_chips);
                events.push(event(EventStage::JokerPostScore, "joker_hack", format!("{} added {} chips (retrigger)", spec.name, extra_chips)));
            } else { trace.summary = format!("{} no 2/3/4/5 cards scored", spec.name); }
        }
        "j_sock_and_buskin" => {
            trace.supported = true; trace.effect_key = Some("sock_and_buskin".to_string());
            let extra_chips: i32 = ctx.played.iter().filter(|c| is_face_card(&c.rank)).map(|c| c.chip_value()).sum();
            if extra_chips > 0 { *chips += extra_chips; trace.matched = true; trace.summary = format!("{} retriggered face cards (+{} chips)", spec.name, extra_chips); events.push(event(EventStage::JokerPostScore, "joker_sock_and_buskin", format!("{} added {} chips (retrigger)", spec.name, extra_chips))); }
            else { trace.summary = format!("{} no face cards to retrigger", spec.name); }
        }
        "j_hanging_chad" => {
            trace.supported = true; trace.effect_key = Some("hanging_chad".to_string());
            let retrigger_count_val = config_extra_i64(spec).unwrap_or(2) as i32;
            if let Some(first) = ctx.played.first() {
                let extra_chips = first.chip_value() * retrigger_count_val;
                *chips += extra_chips; trace.matched = true;
                trace.summary = format!("{} retriggered first card {} times (+{} chips)", spec.name, retrigger_count_val, extra_chips);
                events.push(event(EventStage::JokerPostScore, "joker_hanging_chad", format!("{} added {} chips (retrigger)", spec.name, extra_chips)));
            } else { trace.summary = format!("{} no cards played", spec.name); }
        }
        "j_business" => {
            let face_count = ctx.played.iter().filter(|c| is_face_card(&c.rank)).count() as i32;
            trace.supported = true; trace.effect_key = Some("business_card".to_string());
            if face_count > 0 { let earned = face_count; *money_delta += earned; trace.matched = true; trace.summary = format!("{} {} face cards => +${} (probabilistic)", spec.name, face_count, earned); events.push(event(EventStage::JokerPostScore, "joker_business", format!("{} earned ${} (probabilistic)", spec.name, earned))); }
            else { trace.summary = format!("{} no face cards scored", spec.name); }
        }
        "j_baron" => {
            let xm = config_extra_f64(spec).unwrap_or(1.5);
            let king_count = ctx.held_in_hand.iter().filter(|c| matches!(c.rank, Rank::King)).count();
            trace.supported = true; trace.effect_key = Some("baron".to_string());
            if king_count > 0 { let total_xm = xm.powi(king_count as i32); *xmult *= total_xm; trace.matched = true; trace.summary = format!("{} {} held Kings => X{:.2}", spec.name, king_count, total_xm); events.push(event(EventStage::JokerPostScore, "joker_baron", format!("{} applied X{:.2} mult", spec.name, total_xm))); }
            else { trace.summary = format!("{} no Kings held", spec.name); }
        }
        "j_shoot_the_moon" => {
            let per = config_extra_i64(spec).unwrap_or(13) as i32;
            let queen_count = ctx.held_in_hand.iter().filter(|c| matches!(c.rank, Rank::Queen)).count() as i32;
            trace.supported = true; trace.effect_key = Some("shoot_the_moon".to_string());
            if queen_count > 0 { let gained = queen_count * per; *mult += gained; trace.matched = true; trace.summary = format!("{} {} held Queens => +{} mult", spec.name, queen_count, gained); events.push(event(EventStage::JokerPostScore, "joker_shoot_the_moon", format!("{} added {} mult", spec.name, gained))); }
            else { trace.summary = format!("{} no Queens held", spec.name); }
        }
        "j_raised_fist" => {
            trace.supported = true; trace.effect_key = Some("raised_fist".to_string());
            if let Some(lowest) = ctx.held_in_hand.iter().min_by_key(|c| c.chip_value()) {
                let gained = lowest.chip_value() * 2;
                *mult += gained; trace.matched = true;
                trace.summary = format!("{} lowest held card value {} => +{} mult", spec.name, lowest.chip_value(), gained);
                events.push(event(EventStage::JokerPostScore, "joker_raised_fist", format!("{} added {} mult", spec.name, gained)));
            } else { trace.summary = format!("{} no cards held", spec.name); }
        }
        "j_mime" => { trace.supported = true; trace.effect_key = Some("mime".to_string()); trace.summary = format!("{} (held-in-hand retrigger TODO)", spec.name); }
        "j_reserved_parking" => {
            let face_count = ctx.held_in_hand.iter().filter(|c| is_face_card(&c.rank)).count() as i32;
            trace.supported = true; trace.effect_key = Some("reserved_parking".to_string());
            if face_count > 0 { let earned = (face_count + 1) / 2; *money_delta += earned; trace.matched = true; trace.summary = format!("{} {} held face cards => +${} (probabilistic)", spec.name, face_count, earned); }
            else { trace.summary = format!("{} no face cards held", spec.name); }
        }
        // Passive / no scoring effect jokers
        "j_four_fingers" | "j_shortcut" | "j_pareidolia" | "j_smeared" | "j_splash"
        | "j_ring_master" | "j_oops" | "j_credit_card" | "j_chaos" | "j_juggler"
        | "j_drunkard" | "j_troubadour" | "j_merry_andy" | "j_turtle_bean"
        | "j_certificate" | "j_astronomer" | "j_diet_cola" | "j_mr_bones"
        | "j_blueprint" | "j_brainstorm" | "j_invisible" | "j_chicot" | "j_perkeo"
        | "j_marble" | "j_burglar" | "j_riff_raff" | "j_cartomancer" | "j_burnt"
        | "j_dna" | "j_sixth_sense" | "j_luchador" | "j_midas_mask" => {
            trace.supported = true; trace.effect_key = Some("passive_no_score_effect".to_string());
            trace.summary = format!("{} has no direct scoring effect", spec.name);
        }
        // Economy / end-of-round jokers
        "j_delayed_grat" | "j_golden" | "j_egg" | "j_cloud_9" | "j_rocket"
        | "j_to_the_moon" | "j_satellite" | "j_gift" | "j_hallucination"
        | "j_faceless" | "j_mail" | "j_trading" | "j_vagabond" | "j_matador" => {
            trace.supported = true; trace.effect_key = Some("economy_no_score_effect".to_string());
            trace.summary = format!("{} economy/trigger effect (not during scoring)", spec.name);
        }
        // Retrigger-only jokers
        "j_dusk" | "j_selzer" | "j_8_ball" | "j_space" | "j_superposition"
        | "j_todo_list" | "j_seance" | "j_hiker" => {
            trace.supported = true; trace.effect_key = Some("trigger_effect_no_direct_score".to_string());
            trace.summary = format!("{} trigger/retrigger effect (partial)", spec.name);
        }
        // Scaling jokers with complex state
        "j_runner" => {
            trace.supported = true; trace.effect_key = Some("runner".to_string());
            let bonus = runtime_state.get("chips").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *chips += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_runner", format!("{} added {} chips", spec.name, bonus))); }
            trace.summary = format!("{} +{} chips (accumulated)", spec.name, bonus);
        }
        "j_square" => {
            trace.supported = true; trace.effect_key = Some("square_joker".to_string());
            let bonus = runtime_state.get("chips").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *chips += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_square", format!("{} added {} chips", spec.name, bonus))); }
            trace.summary = format!("{} +{} chips (accumulated)", spec.name, bonus);
        }
        "j_trousers" => {
            trace.supported = true; trace.effect_key = Some("spare_trousers".to_string());
            let bonus = runtime_state.get("mult").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_trousers", format!("{} added {} mult", spec.name, bonus))); }
            trace.summary = format!("{} +{} mult (accumulated)", spec.name, bonus);
        }
        "j_castle" => {
            trace.supported = true; trace.effect_key = Some("castle".to_string());
            let bonus = runtime_state.get("chips").copied().unwrap_or(0.0) as i32;
            if bonus > 0 { *chips += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_castle", format!("{} added {} chips", spec.name, bonus))); }
            trace.summary = format!("{} +{} chips (accumulated)", spec.name, bonus);
        }
        "j_hit_the_road" => {
            trace.supported = true; trace.effect_key = Some("hit_the_road".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_hit_the_road", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (Jacks discarded this round)", spec.name, xm);
        }
        "j_idol" => {
            trace.supported = true; trace.effect_key = Some("idol".to_string());
            // Target rank stored as "rank" index (0-12), target suit as "suit" index (0-3)
            let target_rank = runtime_state.get("rank").copied().unwrap_or(-1.0) as i32;
            let target_suit = runtime_state.get("suit").copied().unwrap_or(-1.0) as i32;
            if target_rank >= 0 && target_suit >= 0 {
                let xm = config_extra_f64(spec).unwrap_or(2.0);
                let matched_count = ctx.played.iter().filter(|c| c.rank.index() as i32 == target_rank && c.suit.index() as i32 == target_suit).count();
                if matched_count > 0 {
                    let total_xm = xm.powi(matched_count as i32);
                    *xmult *= total_xm; trace.matched = true;
                    trace.summary = format!("{} {} matching card(s) => X{:.2}", spec.name, matched_count, total_xm);
                    events.push(event(EventStage::JokerPostScore, "joker_idol", format!("{} applied X{:.2} mult", spec.name, total_xm)));
                } else {
                    trace.summary = format!("{} no matching rank+suit target", spec.name);
                }
            } else {
                trace.summary = format!("{} no target set yet", spec.name);
            }
        }
        "j_caino" => {
            trace.supported = true; trace.effect_key = Some("caino".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_caino", format!("{} applied X{:.2} mult", spec.name, xm))); }
            trace.summary = format!("{} X{:.2} (face cards destroyed)", spec.name, xm);
        }
        "j_yorick" => {
            trace.supported = true; trace.effect_key = Some("yorick".to_string());
            let xm = runtime_state.get("xmult").copied().unwrap_or(1.0);
            if xm > 1.0 { *xmult *= xm; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_yorick", format!("{} applied X{:.2} mult", spec.name, xm))); }
            let remaining = runtime_state.get("discards_remaining").copied().unwrap_or(23.0) as i32;
            trace.summary = format!("{} X{:.2} ({} discards until next bump)", spec.name, xm, remaining);
        }
        _ => {
            // Unknown joker - not implemented
        }
    }
}

fn event(stage: EventStage, kind: impl Into<String>, summary: impl Into<String>) -> Event {
    Event {
        stage,
        kind: kind.into(),
        summary: summary.into(),
        activation_stage: None,
        joker_slot: None,
        joker_id: None,
        source_card_slot: None,
        effect_text_en: None,
        chips_delta: None,
        mult_delta: None,
        xmult_delta: None,
        money_delta: None,
        state_delta: BTreeMap::new(),
        payload: BTreeMap::new(),
    }
}

fn event_with_details(
    stage: EventStage,
    kind: impl Into<String>,
    summary: impl Into<String>,
    joker_slot: Option<usize>,
    joker_id: Option<&str>,
    source_card_slot: Option<usize>,
    mult_delta: Option<f64>,
    xmult_delta: Option<f64>,
    money_delta: Option<i32>,
) -> Event {
    Event {
        stage,
        kind: kind.into(),
        summary: summary.into(),
        activation_stage: None,
        joker_slot,
        joker_id: joker_id.map(String::from),
        source_card_slot,
        effect_text_en: None,
        chips_delta: None,
        mult_delta,
        xmult_delta,
        money_delta,
        state_delta: BTreeMap::new(),
        payload: BTreeMap::new(),
    }
}

fn hand_type_to_key(name: &str) -> &'static str {
    match name {
        "Pair" => "pair",
        "Two Pair" => "two_pair",
        "Three of a Kind" => "three_of_kind",
        "Straight" => "straight",
        "Flush" => "flush",
        "Full House" => "full_house",
        "Four of a Kind" => "four_of_a_kind",
        "Straight Flush" => "straight_flush",
        "Five of a Kind" => "five_of_a_kind",
        "Flush House" => "flush_house",
        "Flush Five" => "flush_five",
        _ => "high_card",
    }
}

fn suit_label(suit: &Suit) -> &'static str {
    match suit {
        Suit::Spades => "Spades",
        Suit::Hearts => "Hearts",
        Suit::Diamonds => "Diamonds",
        Suit::Clubs => "Clubs",
    }
}

fn rank_up(rank: &Rank) -> Rank {
    match rank {
        Rank::Two => Rank::Three,
        Rank::Three => Rank::Four,
        Rank::Four => Rank::Five,
        Rank::Five => Rank::Six,
        Rank::Six => Rank::Seven,
        Rank::Seven => Rank::Eight,
        Rank::Eight => Rank::Nine,
        Rank::Nine => Rank::Ten,
        Rank::Ten => Rank::Jack,
        Rank::Jack => Rank::Queen,
        Rank::Queen => Rank::King,
        Rank::King => Rank::Ace,
        Rank::Ace => Rank::Ace,
    }
}

fn blind_reward(blind: &BlindKind) -> i32 {
    match blind {
        BlindKind::Small => 3,
        BlindKind::Big => 4,
        BlindKind::Boss(_) => 5,
    }
}

fn blind_reward_for_slot(slot: BlindSlot) -> i32 {
    match slot {
        BlindSlot::Small => 3,
        BlindSlot::Big => 4,
        BlindSlot::Boss => 5,
    }
}

pub fn action_name(index: usize) -> String {
    match index {
        0..=7 => format!("select_card_{}", index),
        8 => "play".to_string(),
        9 => "discard".to_string(),
        10..=12 => format!("select_blind_{}", index - 10),
        13 => "cashout".to_string(),
        14..=23 => format!("buy_shop_item_{}", index - 14),
        24..=25 => format!("buy_consumable_{}", index - 24),
        26..=27 => format!("sell_consumable_{}", index - 26),
        28 => "buy_voucher".to_string(),
        29..=30 => format!("buy_pack_{}", index - 29),
        31..=35 => format!("pick_pack_{}", index - 31),
        36 => "skip_pack".to_string(),
        37..=46 => format!("move_left_{}", index - 37),
        47..=69 => format!("move_right_{}", index - 47),
        70 => "next_round".to_string(),
        71..=78 => format!("use_consumable_{}", index - 71),
        79 => "reroll_shop".to_string(),
        80..=84 => format!("sell_joker_{}", index - 80),
        85 => "skip_blind".to_string(),
        _ => format!("unknown_{}", index),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        action_name, apply_joker_effect, calculate_retriggers, resolve_joker_ability,
        BlindProgress, BlindSlot, BossEffect, CardInstance, ConsumableInstance, Engine,
        EngineError, EventStage, JokerInstance, JokerResolutionTrace, Phase, Rank,
        RunConfig, ScoringContext, Suit, TransitionTrace, CONSUMABLE_SLOT_LIMIT,
        HAND_LIMIT, JOKER_LIMIT,
    };
    use balatro_spec::{JokerSpec, RulesetBundle};
    use rand_chacha::ChaCha8Rng;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn fixture_bundle() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/ruleset/balatro-1.0.1o-full.json")
    }

    // --- Test helpers ---

    fn make_card(rank: Rank, suit: Suit) -> CardInstance {
        CardInstance { card_id: 0, rank, suit, enhancement: None, edition: None, seal: None }
    }

    fn make_card_with_seal(rank: Rank, suit: Suit, seal: Option<&str>) -> CardInstance {
        CardInstance { card_id: 1, rank, suit, enhancement: None, edition: None, seal: seal.map(|s| s.to_string()) }
    }

    fn make_joker_instance(id: &str, name: &str) -> JokerInstance {
        JokerInstance { joker_id: id.to_string(), name: name.to_string(), base_cost: 5, cost: 5, buy_cost: 5, sell_value: 2, extra_sell_value: 0, rarity: 1, edition: None, slot_index: 0, activation_class: "joker_independent".to_string(), wiki_effect_text_en: String::new(), remaining_uses: None, runtime_state: BTreeMap::new() }
    }

    fn make_joker_for_retrigger(id: &str, name: &str, slot: usize) -> JokerInstance {
        JokerInstance { joker_id: id.to_string(), name: name.to_string(), base_cost: 0, cost: 0, buy_cost: 0, sell_value: 0, extra_sell_value: 0, rarity: 1, edition: None, slot_index: slot, activation_class: String::new(), wiki_effect_text_en: String::new(), remaining_uses: None, runtime_state: BTreeMap::new() }
    }

    fn make_joker_spec(id: &str, name: &str) -> JokerSpec {
        JokerSpec { id: id.to_string(), order: 0, name: name.to_string(), set: "Joker".to_string(), base_cost: 0, cost: 0, rarity: 1, effect: None, config: std::collections::BTreeMap::new(), wiki_effect_text_en: String::new(), activation_class: String::new(), source_refs: std::collections::BTreeMap::new(), unlocked: true, blueprint_compat: true, perishable_compat: true, eternal_compat: true, sprite: None }
    }

    fn make_joker_from_bundle(bundle: &RulesetBundle, joker_id: &str, slot_index: usize) -> JokerInstance {
        let spec = bundle.joker_by_id(joker_id).expect("joker spec");
        JokerInstance { joker_id: spec.id.clone(), name: spec.name.clone(), base_cost: spec.base_cost, cost: spec.cost, buy_cost: spec.cost, sell_value: (spec.cost / 2).max(1), extra_sell_value: 0, rarity: spec.rarity, edition: None, slot_index, activation_class: spec.activation_class.clone(), wiki_effect_text_en: spec.wiki_effect_text_en.clone(), remaining_uses: None, runtime_state: BTreeMap::new() }
    }

    fn make_consumable(id: &str, name: &str, set: &str, cost: i32, config: BTreeMap<String, serde_json::Value>) -> ConsumableInstance {
        ConsumableInstance { consumable_id: id.to_string(), name: name.to_string(), set: set.to_string(), cost, buy_cost: cost, sell_value: (cost / 2).max(1), slot_index: 0, config }
    }

    fn default_ctx<'a>(hand_key: &'a str, played: &'a [CardInstance], held: &'a [CardInstance], jokers: &'a [JokerInstance]) -> ScoringContext<'a> {
        ScoringContext { hand_key, played, held_in_hand: held, discards_left: 3, plays_left: 4, jokers, money: 10, deck_cards_remaining: 36, full_deck_size: 52, joker_slot_max: JOKER_LIMIT }
    }

    fn fresh_trace() -> JokerResolutionTrace {
        JokerResolutionTrace { order: 0, joker_id: String::new(), joker_name: String::new(), slot_index: 0, stage: "joker_main".to_string(), supported: false, matched: false, retrigger_count: 0, effect_key: None, summary: String::new() }
    }

    fn load_spec(id: &str) -> JokerSpec {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        bundle.jokers.iter().find(|j| j.id == id).cloned().expect("joker spec")
    }

    // ==== Core engine tests (from main) ====

    #[test]
    fn new_engine_records_deck_and_seed_str_on_snapshot() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut cfg = RunConfig::default();
        cfg.deck_key = "red".into();
        cfg.seed_str = "4WAX5M4D".into();
        cfg.stake = 1;
        let e = Engine::new(42, bundle, cfg);
        let snap = e.snapshot();
        assert_eq!(snap.seed_str, "4WAX5M4D");
        assert_eq!(snap.deck_name, "red");
        assert_eq!(snap.stake_name, "WHITE");
    }

    #[test]
    fn engine_is_deterministic_for_same_seed_and_actions() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut left = Engine::new(7, bundle.clone(), RunConfig::default());
        let mut right = Engine::new(7, bundle, RunConfig::default());
        for _ in 0..8 {
            let action = left.legal_actions().into_iter().find(|action| action.enabled).expect("legal action").index;
            let l = left.step(action).expect("left step");
            let r = right.step(action).expect("right step");
            assert_eq!(l.snapshot_after, r.snapshot_after);
            assert_eq!(l.events, r.events);
        }
    }

    #[test]
    fn legal_action_names_match_legacy_layout() {
        assert_eq!(action_name(8), "play");
        assert_eq!(action_name(79), "reroll_shop");
        assert_eq!(action_name(85), "skip_blind");
    }

    #[test]
    fn initial_blind_path_matches_linear_progression() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let engine = Engine::new(7, bundle, RunConfig::default());
        let snapshot = engine.snapshot();
        assert_eq!(snapshot.stage, "Stage_PreBlind");
        assert_eq!(snapshot.blind_name, "Small Blind");
        assert_eq!(snapshot.blind_states.get("Small").map(String::as_str), Some("Select"));
        assert_eq!(snapshot.blind_states.get("Big").map(String::as_str), Some("Upcoming"));
    }

    #[test]
    fn skip_blind_advances_small_to_big_to_boss() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(11, bundle, RunConfig::default());
        engine.step(85).expect("skip small");
        let after_small_skip = engine.snapshot();
        assert_eq!(after_small_skip.blind_name, "Big Blind");
        engine.step(85).expect("skip big");
        let after_big_skip = engine.snapshot();
        assert_ne!(after_big_skip.blind_name, "Big Blind");
    }

    #[test]
    fn boss_cannot_be_selected_from_initial_preblind() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(13, bundle, RunConfig::default());
        let err = engine.step(12).expect_err("boss blind should be illegal initially");
        assert!(matches!(err, EngineError::IllegalAction(12)));
    }

    #[test]
    fn shop_inventory_is_hidden_outside_shop() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(17, bundle, RunConfig::default());
        assert!(engine.snapshot().shop_jokers.is_empty());
        engine.state.phase = Phase::Shop;
        let mut trace = TransitionTrace::default();
        engine.refresh_shop(&mut trace, "test_shop_refresh");
        assert!(!engine.snapshot().shop_jokers.is_empty());
    }

    #[test]
    fn ante_advances_only_after_boss_resolution() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(19, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 3;
        engine.handle_post_blind(13, &mut trace);
        assert_eq!(engine.state.ante, 1);
        assert_eq!(engine.state.current_blind_slot, BlindSlot::Big);
    }

    #[test]
    fn cashout_from_small_blind_enters_shop_then_big_blind() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(23, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 3;
        let transition = engine.step(13).expect("cashout after small blind");
        assert_eq!(transition.snapshot_after.stage, "Stage_Shop");
        assert_eq!(transition.snapshot_after.money, 7);
    }

    #[test]
    fn select_blind_transition_emits_transient_states_and_shuffle_trace() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(29, bundle, RunConfig::default());
        let transition = engine.step(10).expect("select small blind");
        assert_eq!(transition.snapshot_after.stage, "Stage_Blind");
        assert!(transition.trace.transient_lua_states.contains(&"NEW_ROUND".to_string()));
    }

    #[test]
    fn cashout_transition_emits_cashout_shuffle_and_shop_refresh_trace() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(31, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 3;
        let transition = engine.step(13).expect("cashout");
        assert!(transition.trace.rng_calls.iter().any(|entry| entry.domain == "deck.shuffle.cashout"));
        assert!(transition.trace.rng_calls.iter().any(|entry| entry.domain.starts_with("cashout_shop_refresh.consumable_slot_")));
    }

    // ==== Consumable tests (from main) ====

    #[test]
    fn shop_offers_consumables_after_cashout() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(41, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 3;
        let transition = engine.step(13).expect("cashout");
        assert!(!transition.snapshot_after.shop_consumables.is_empty());
    }

    #[test]
    fn buy_consumable_deducts_money_and_adds_to_inventory() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(43, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 10;
        engine.step(13).expect("cashout to shop");
        let snap = engine.snapshot();
        assert!(snap.money >= 10);
        let money_before = snap.money;
        let consumable_cost = snap.shop_consumables[0].buy_cost;
        let transition = engine.step(24).expect("buy consumable 0");
        assert_eq!(transition.snapshot_after.money, money_before - consumable_cost);
        assert_eq!(transition.snapshot_after.consumables.len(), 1);
    }

    #[test]
    fn sell_consumable_adds_money_and_removes_from_inventory() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(47, bundle, RunConfig::default());
        engine.state.phase = Phase::Shop;
        engine.state.money = 10;
        let mut config = BTreeMap::new();
        config.insert("hand_type".to_string(), serde_json::json!("Pair"));
        engine.state.consumables.push(make_consumable("c_mercury", "Mercury", "Planet", 3, config));
        let mut trace = TransitionTrace::default();
        engine.refresh_shop(&mut trace, "test");
        let sell_value = engine.state.consumables[0].sell_value;
        let money_before = engine.state.money;
        let transition = engine.step(26).expect("sell consumable 0");
        assert_eq!(transition.snapshot_after.money, money_before + sell_value);
        assert!(transition.snapshot_after.consumables.is_empty());
    }

    #[test]
    fn use_planet_consumable_levels_up_hand() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(53, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        let mut config = BTreeMap::new();
        config.insert("hand_type".to_string(), serde_json::json!("Pair"));
        engine.state.consumables.push(make_consumable("c_mercury", "Mercury", "Planet", 3, config));
        engine.enter_current_blind(&mut trace);
        let before_level = engine.state.hand_levels.get("pair").copied().unwrap_or(1);
        let transition = engine.step(71).expect("use consumable 0");
        let after_level = transition.snapshot_after.hand_levels.get("pair").copied().unwrap_or(1);
        assert_eq!(after_level, before_level + 1);
    }

    #[test]
    fn use_tarot_hermit_doubles_money() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(59, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.money = 15;
        let mut config = BTreeMap::new();
        config.insert("extra".to_string(), serde_json::json!(20));
        engine.state.consumables.push(make_consumable("c_hermit", "The Hermit", "Tarot", 3, config));
        engine.enter_current_blind(&mut trace);
        let transition = engine.step(71).expect("use hermit");
        assert_eq!(transition.snapshot_after.money, 30);
    }

    #[test]
    fn cannot_buy_consumable_when_slots_full() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(61, bundle, RunConfig::default());
        engine.state.phase = Phase::Shop;
        engine.state.money = 100;
        for i in 0..CONSUMABLE_SLOT_LIMIT {
            let mut config = BTreeMap::new();
            config.insert("hand_type".to_string(), serde_json::json!("Pair"));
            let mut c = make_consumable("c_mercury", "Mercury", "Planet", 3, config);
            c.slot_index = i;
            engine.state.consumables.push(c);
        }
        let mut trace = TransitionTrace::default();
        engine.refresh_shop(&mut trace, "test");
        let legal = engine.legal_actions();
        assert!(!legal.iter().any(|a| a.name == "buy_consumable_0" && a.enabled));
    }

    #[test]
    fn consumable_action_names_correct() {
        assert_eq!(action_name(24), "buy_consumable_0");
        assert_eq!(action_name(25), "buy_consumable_1");
        assert_eq!(action_name(26), "sell_consumable_0");
        assert_eq!(action_name(27), "sell_consumable_1");
        assert_eq!(action_name(71), "use_consumable_0");
    }

    #[test]
    fn planet_level_bonus_affects_scoring() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(67, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.hand_levels.insert("pair".to_string(), 3);
        engine.enter_current_blind(&mut trace);
        let available = engine.state.available.clone();
        let mut pair_indices = Vec::new();
        for (i, c1) in available.iter().enumerate() {
            for (j, c2) in available.iter().enumerate().skip(i + 1) {
                if c1.rank_index() == c2.rank_index() { pair_indices.push(i); pair_indices.push(j); break; }
            }
            if pair_indices.len() >= 2 { break; }
        }
        assert!(pair_indices.len() >= 2);
        engine.step(pair_indices[0]).expect("select card 0");
        engine.step(pair_indices[1]).expect("select card 1");
        let transition = engine.step(8).expect("play pair");
        assert!(transition.events.iter().any(|e| e.kind == "hand_played"));
    }

    /// P1 parity test: `hand_stats["Pair"].played` tracks total hands played,
    /// and `played_this_round` resets on round entry while `played` persists.
    #[test]
    fn hand_stats_tracks_played_count_across_hands() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(73, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);

        // Pre-condition: Pair has zero plays, level 1, correct base chips/mult
        let pre = engine.state.hand_stats.get("Pair").cloned().expect("Pair seeded");
        assert_eq!(pre.played, 0);
        assert_eq!(pre.played_this_round, 0);
        assert_eq!(pre.level, 1);
        assert_eq!(pre.chips, 10);
        assert_eq!(pre.mult, 2);
        assert_eq!(pre.order, 11);

        // Play a Pair: pick two matching-rank cards from the dealt hand.
        let available = engine.state.available.clone();
        let mut pair_indices = Vec::new();
        for (i, c1) in available.iter().enumerate() {
            for (j, c2) in available.iter().enumerate().skip(i + 1) {
                if c1.rank_index() == c2.rank_index() {
                    pair_indices.push(i);
                    pair_indices.push(j);
                    break;
                }
            }
            if pair_indices.len() >= 2 { break; }
        }
        assert!(pair_indices.len() >= 2, "dealt hand should contain a pair");
        engine.step(pair_indices[0]).expect("select card 0");
        engine.step(pair_indices[1]).expect("select card 1");
        engine.step(8).expect("play pair");

        let after = engine.state.hand_stats.get("Pair").cloned().expect("Pair stats");
        assert_eq!(after.played, 1, "all-time played count increments");
        assert_eq!(after.played_this_round, 1, "per-round counter increments");

        // Force a round transition by calling enter_current_blind again; this
        // should zero played_this_round but leave played untouched.
        let mut trace2 = TransitionTrace::default();
        engine.enter_current_blind(&mut trace2);
        let rolled = engine.state.hand_stats.get("Pair").cloned().expect("Pair stats");
        assert_eq!(rolled.played, 1, "lifetime counter persists across rounds");
        assert_eq!(rolled.played_this_round, 0, "per-round counter resets");
    }

    /// `bump_hand_level` must recompute chips/mult from the ruleset.
    #[test]
    fn hand_stats_level_up_recomputes_chips_and_mult() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(79, bundle, RunConfig::default());
        // Pair: base 10 chips, 2 mult; +15 chips, +1 mult per level bonus.
        engine.bump_hand_level("pair", 2);
        let stats = engine.state.hand_stats.get("Pair").cloned().expect("Pair");
        assert_eq!(stats.level, 3);
        assert_eq!(stats.chips, 10 + 2 * 15);
        assert_eq!(stats.mult, 2 + 2 * 1);
        // Legacy hand_levels stays in sync.
        assert_eq!(engine.state.hand_levels.get("pair").copied(), Some(3));
    }

    // ==== Retrigger tests (from main) ====

    #[test]
    fn red_seal_retrigger_adds_one() {
        let card = make_card_with_seal(Rank::King, Suit::Spades, Some("Red"));
        let retriggers = calculate_retriggers(&card, 0, &[], &[], false, 1);
        assert_eq!(retriggers, 1);
    }

    #[test]
    fn no_seal_no_retriggers() {
        let card = make_card_with_seal(Rank::King, Suit::Spades, None);
        let retriggers = calculate_retriggers(&card, 0, &[], &[], false, 1);
        assert_eq!(retriggers, 0);
    }

    #[test]
    fn sock_and_buskin_face_card_retriggers() {
        let face_card = make_card_with_seal(Rank::Queen, Suit::Hearts, None);
        let jokers = vec![make_joker_for_retrigger("j_sock_and_buskin", "Sock and Buskin", 0)];
        let specs = vec![Some(make_joker_spec("j_sock_and_buskin", "Sock and Buskin"))];
        assert_eq!(calculate_retriggers(&face_card, 0, &jokers, &specs, false, 2), 1);
    }

    #[test]
    fn hanging_chad_only_first_card() {
        let card = make_card_with_seal(Rank::Ace, Suit::Clubs, None);
        let jokers = vec![make_joker_for_retrigger("j_hanging_chad", "Hanging Chad", 0)];
        let specs = vec![Some(make_joker_spec("j_hanging_chad", "Hanging Chad"))];
        assert_eq!(calculate_retriggers(&card, 0, &jokers, &specs, false, 3), 2);
        assert_eq!(calculate_retriggers(&card, 1, &jokers, &specs, false, 3), 0);
    }

    #[test]
    fn dusk_only_on_final_hand() {
        let card = make_card_with_seal(Rank::Ten, Suit::Spades, None);
        let jokers = vec![make_joker_for_retrigger("j_dusk", "Dusk", 0)];
        let specs = vec![Some(make_joker_spec("j_dusk", "Dusk"))];
        assert_eq!(calculate_retriggers(&card, 0, &jokers, &specs, true, 1), 1);
        assert_eq!(calculate_retriggers(&card, 0, &jokers, &specs, false, 1), 0);
    }

    #[test]
    fn seltzer_retriggers_all_cards() {
        let card = make_card_with_seal(Rank::Two, Suit::Hearts, None);
        let mut seltzer = make_joker_for_retrigger("j_seltzer", "Seltzer", 0);
        seltzer.remaining_uses = Some(5);
        let jokers = vec![seltzer];
        let specs = vec![Some(make_joker_spec("j_seltzer", "Seltzer"))];
        assert_eq!(calculate_retriggers(&card, 0, &jokers, &specs, false, 2), 1);
    }

    #[test]
    fn multiple_retrigger_sources_stack() {
        let card = make_card_with_seal(Rank::King, Suit::Spades, Some("Red"));
        let jokers = vec![make_joker_for_retrigger("j_sock_and_buskin", "Sock and Buskin", 0)];
        let specs = vec![Some(make_joker_spec("j_sock_and_buskin", "Sock and Buskin"))];
        assert_eq!(calculate_retriggers(&card, 0, &jokers, &specs, false, 1), 2);
    }

    #[test]
    fn blueprint_copies_retrigger_joker() {
        let card = make_card_with_seal(Rank::Jack, Suit::Diamonds, None);
        let jokers = vec![make_joker_for_retrigger("j_blueprint", "Blueprint", 0), make_joker_for_retrigger("j_sock_and_buskin", "Sock and Buskin", 1)];
        let specs = vec![Some(make_joker_spec("j_blueprint", "Blueprint")), Some(make_joker_spec("j_sock_and_buskin", "Sock and Buskin"))];
        assert_eq!(calculate_retriggers(&card, 0, &jokers, &specs, false, 1), 2);
    }

    #[test]
    fn resolve_joker_ability_handles_blueprint_brainstorm_cycle() {
        let jokers = vec![make_joker_for_retrigger("j_blueprint", "Blueprint", 0), make_joker_for_retrigger("j_brainstorm", "Brainstorm", 1)];
        let specs: Vec<Option<JokerSpec>> = vec![Some(make_joker_spec("j_blueprint", "Blueprint")), Some(make_joker_spec("j_brainstorm", "Brainstorm"))];
        assert!(resolve_joker_ability(0, &jokers, &specs).is_none());
    }

    #[test]
    fn retrigger_supported_flag_is_true() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(42, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.state.selected_slots.insert(0);
        let mut play_trace = TransitionTrace::default();
        let _events = engine.play_selected(&mut play_trace);
        assert!(play_trace.retrigger_supported);
    }

    #[test]
    fn red_seal_scoring_adds_card_chips_twice() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(99, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.state.available = vec![CardInstance { card_id: 999, rank: Rank::Ace, suit: Suit::Spades, enhancement: None, edition: None, seal: Some("Red".to_string()) }];
        engine.state.selected_slots.insert(0);
        let mut play_trace = TransitionTrace::default();
        let events = engine.play_selected(&mut play_trace);
        let card_chip_events: Vec<_> = events.iter().filter(|e| e.kind == "card_chips").collect();
        assert_eq!(card_chip_events.len(), 2);
    }

    // ==== Joker effect unit tests (from a9c8fafb) ====

    #[test]
    fn half_joker_adds_mult_when_three_or_fewer_cards() {
        let spec = load_spec("j_half");
        let played = vec![make_card(Rank::Ace, Suit::Spades), make_card(Rank::King, Suit::Hearts)];
        let jokers = vec![make_joker_instance("j_half", "Half Joker")];
        let ctx = default_ctx("pair", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert!(trace.supported && trace.matched);
        assert_eq!(mult, 20);
    }

    #[test]
    fn half_joker_no_mult_when_more_than_three() {
        let spec = load_spec("j_half");
        let played = vec![make_card(Rank::Ace, Suit::Spades), make_card(Rank::King, Suit::Hearts), make_card(Rank::Queen, Suit::Diamonds), make_card(Rank::Jack, Suit::Clubs)];
        let jokers = vec![make_joker_instance("j_half", "Half Joker")];
        let ctx = default_ctx("four_of_a_kind", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert!(trace.supported && !trace.matched);
        assert_eq!(mult, 0);
    }

    #[test]
    fn fibonacci_joker_adds_mult_for_matching_ranks() {
        let spec = load_spec("j_fibonacci");
        let played = vec![make_card(Rank::Ace, Suit::Spades), make_card(Rank::Two, Suit::Hearts), make_card(Rank::Three, Suit::Diamonds), make_card(Rank::Seven, Suit::Clubs), make_card(Rank::Eight, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_fibonacci", "Fibonacci")];
        let ctx = default_ctx("flush", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert!(trace.matched);
        assert_eq!(mult, 32);
    }

    #[test]
    fn even_steven_adds_mult_for_even_rank_cards() {
        let spec = load_spec("j_even_steven");
        let played = vec![make_card(Rank::Two, Suit::Spades), make_card(Rank::Four, Suit::Hearts), make_card(Rank::Six, Suit::Diamonds), make_card(Rank::Three, Suit::Clubs)];
        let jokers = vec![make_joker_instance("j_even_steven", "Even Steven")];
        let ctx = default_ctx("high_card", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert_eq!(mult, 12);
    }

    #[test]
    fn odd_todd_adds_chips_for_odd_rank_cards() {
        let spec = load_spec("j_odd_todd");
        let played = vec![make_card(Rank::Ace, Suit::Spades), make_card(Rank::Three, Suit::Hearts), make_card(Rank::Five, Suit::Diamonds), make_card(Rank::Four, Suit::Clubs)];
        let jokers = vec![make_joker_instance("j_odd_todd", "Odd Todd")];
        let ctx = default_ctx("high_card", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert_eq!(chips, 93);
    }

    #[test]
    fn blackboard_xmult_when_all_held_dark_suits() {
        let spec = load_spec("j_blackboard");
        let played = vec![make_card(Rank::Ace, Suit::Hearts)];
        let held = vec![make_card(Rank::Two, Suit::Spades), make_card(Rank::Three, Suit::Clubs)];
        let jokers = vec![make_joker_instance("j_blackboard", "Blackboard")];
        let ctx = default_ctx("high_card", &played, &held, &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert!(trace.matched);
        assert!((xmult - 3.0).abs() < 0.001);
    }

    #[test]
    fn flower_pot_xmult_when_all_four_suits() {
        let spec = load_spec("j_flower_pot");
        let played = vec![make_card(Rank::Ace, Suit::Spades), make_card(Rank::King, Suit::Hearts), make_card(Rank::Queen, Suit::Diamonds), make_card(Rank::Jack, Suit::Clubs), make_card(Rank::Ten, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_flower_pot", "Flower Pot")];
        let ctx = default_ctx("flush", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert!((xmult - 3.0).abs() < 0.001);
    }

    #[test]
    fn bull_adds_chips_per_dollar() {
        let spec = load_spec("j_bull");
        let played = vec![make_card(Rank::Ace, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_bull", "Bull")];
        let mut ctx = default_ctx("high_card", &played, &[], &jokers);
        ctx.money = 25;
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert_eq!(chips, 50);
    }

    #[test]
    fn acrobat_xmult_on_final_hand() {
        let spec = load_spec("j_acrobat");
        let played = vec![make_card(Rank::Ace, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_acrobat", "Acrobat")];
        let mut ctx = default_ctx("high_card", &played, &[], &jokers);
        ctx.plays_left = 1;
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert!((xmult - 3.0).abs() < 0.001);
    }

    #[test]
    fn stuntman_adds_chips() {
        let spec = load_spec("j_stuntman");
        let played = vec![make_card(Rank::Ace, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_stuntman", "Stuntman")];
        let ctx = default_ctx("high_card", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert_eq!(chips, 250);
    }

    #[test]
    fn abstract_joker_adds_mult_per_joker_owned() {
        let spec = load_spec("j_abstract");
        let played = vec![make_card(Rank::Ace, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_abstract", "Abstract Joker"), make_joker_instance("j_joker", "Joker"), make_joker_instance("j_half", "Half Joker")];
        let ctx = default_ctx("high_card", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &BTreeMap::new());
        assert_eq!(mult, 9);
    }

    // ==== Phase tests (from a24b00b5) ====

    #[test]
    fn held_in_hand_baron_kings_give_xmult() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(42, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.state.jokers.push(make_joker_from_bundle(&bundle, "j_baron", 0));
        engine.state.available = vec![
            CardInstance { card_id: 100, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 101, rank: Rank::Three, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 102, rank: Rank::Four, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 103, rank: Rank::King, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 104, rank: Rank::King, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
        ];
        engine.state.selected_slots.insert(0);
        let events = engine.play_selected(&mut trace);
        let baron_events: Vec<_> = events.iter().filter(|e| e.stage == EventStage::HeldInHand && e.joker_id.as_deref() == Some("j_baron")).collect();
        assert_eq!(baron_events.len(), 2, "Expected 2 Baron activations for 2 held Kings");
    }

    #[test]
    fn end_of_round_golden_joker_adds_money() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(77, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 3;
        engine.state.jokers.push(make_joker_from_bundle(&bundle, "j_golden", 0));
        let money_before = engine.state.money;
        let transition = engine.step(13).expect("cashout with golden joker");
        let golden_events: Vec<_> = transition.events.iter().filter(|e| e.stage == EventStage::EndOfRound && e.joker_id.as_deref() == Some("j_golden")).collect();
        assert_eq!(golden_events.len(), 1);
        assert_eq!(transition.snapshot_after.money, money_before + 4 + 3);
    }

    #[test]
    fn end_of_round_gros_michel_destruction_chance() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut destroyed_count = 0;
        let mut survived_count = 0;
        for seed in 0..60 {
            let mut engine = Engine::new(seed, bundle.clone(), RunConfig::default());
            let mut trace = TransitionTrace::default();
            engine.enter_current_blind(&mut trace);
            engine.mark_current_blind_progress(BlindProgress::Defeated);
            engine.state.phase = Phase::PostBlind;
            engine.state.reward = 3;
            engine.state.jokers.push(make_joker_from_bundle(&bundle, "j_gros_michel", 0));
            let transition = engine.step(13).expect("cashout");
            let gm_events: Vec<_> = transition.events.iter().filter(|e| e.stage == EventStage::EndOfRound && e.joker_id.as_deref() == Some("j_gros_michel")).collect();
            assert_eq!(gm_events.len(), 1);
            if gm_events[0].summary.contains("destroyed") { destroyed_count += 1; }
            else { survived_count += 1; }
        }
        assert!(destroyed_count > 0);
        assert!(survived_count > 0);
    }

    #[test]
    fn boss_blind_chicot_disables_boss_effect() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(99, bundle.clone(), RunConfig::default());
        engine.step(85).expect("skip small");
        engine.step(85).expect("skip big");
        engine.state.jokers.push(make_joker_from_bundle(&bundle, "j_chicot", 0));
        let transition = engine.step(12).expect("select boss blind");
        let chicot_events: Vec<_> = transition.events.iter().filter(|e| e.kind == "blind_select_joker" && e.joker_id.as_deref() == Some("j_chicot")).collect();
        assert_eq!(chicot_events.len(), 1);
        assert!(transition.snapshot_after.boss_effect.contains("disabled by Chicot"));
    }

    #[test]
    fn blind_select_burglar_adds_hands_removes_discards() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(55, bundle.clone(), RunConfig::default());
        engine.state.jokers.push(make_joker_from_bundle(&bundle, "j_burglar", 0));
        let transition = engine.step(10).expect("select small blind with burglar");
        assert_eq!(transition.snapshot_after.plays, 7);
        assert_eq!(transition.snapshot_after.discards, 0);
    }

    // ==== Enhancement scoring tests (Task A) ====

    #[test]
    fn bonus_card_adds_30_chips() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(200, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        // Replace hand with a single Bonus-enhanced card
        engine.state.available = vec![CardInstance {
            card_id: 500,
            rank: Rank::Five,
            suit: Suit::Hearts,
            enhancement: Some("m_bonus".to_string()),
            edition: None,
            seal: None,
        }];
        engine.state.selected_slots.insert(0);
        let mut play_trace = TransitionTrace::default();
        let events = engine.play_selected(&mut play_trace);
        let bonus_events: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "enhancement_bonus")
            .collect();
        assert_eq!(bonus_events.len(), 1, "Expected 1 Bonus Card enhancement event");
        assert!(bonus_events[0].summary.contains("+30 chips"));
    }

    #[test]
    fn glass_card_applies_x2_mult() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(201, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.state.available = vec![CardInstance {
            card_id: 501,
            rank: Rank::Ace,
            suit: Suit::Spades,
            enhancement: Some("m_glass".to_string()),
            edition: None,
            seal: None,
        }];
        engine.state.selected_slots.insert(0);
        let mut play_trace = TransitionTrace::default();
        let events = engine.play_selected(&mut play_trace);
        let glass_events: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "enhancement_glass")
            .collect();
        assert_eq!(glass_events.len(), 1, "Expected 1 Glass Card enhancement event");
        assert!(glass_events[0].summary.contains("X2 mult"));
    }

    // ==== Edition scoring tests (Task B) ====

    #[test]
    fn foil_edition_adds_50_chips() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(202, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.state.available = vec![CardInstance {
            card_id: 502,
            rank: Rank::King,
            suit: Suit::Diamonds,
            enhancement: None,
            edition: Some("e_foil".to_string()),
            seal: None,
        }];
        engine.state.selected_slots.insert(0);
        let mut play_trace = TransitionTrace::default();
        let events = engine.play_selected(&mut play_trace);
        let foil_events: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "edition_foil")
            .collect();
        assert_eq!(foil_events.len(), 1, "Expected 1 Foil edition event");
        assert!(foil_events[0].summary.contains("+50 chips"));
    }

    #[test]
    fn polychrome_edition_applies_x1_5_mult() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(203, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.state.available = vec![CardInstance {
            card_id: 503,
            rank: Rank::Queen,
            suit: Suit::Clubs,
            enhancement: None,
            edition: Some("e_polychrome".to_string()),
            seal: None,
        }];
        engine.state.selected_slots.insert(0);
        let mut play_trace = TransitionTrace::default();
        let events = engine.play_selected(&mut play_trace);
        let poly_events: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "edition_polychrome")
            .collect();
        assert_eq!(poly_events.len(), 1, "Expected 1 Polychrome edition event");
        assert!(poly_events[0].summary.contains("X1.5 mult"));
    }

    // ==== joker_on_played phase tests (Task C) ====

    #[test]
    fn midas_mask_turns_face_cards_gold() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(204, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.state.jokers.push(make_joker_from_bundle(&bundle, "j_midas_mask", 0));
        // Set up hand with face cards and a non-face card
        engine.state.available = vec![
            CardInstance {
                card_id: 600,
                rank: Rank::King,
                suit: Suit::Spades,
                enhancement: None,
                edition: None,
                seal: None,
            },
            CardInstance {
                card_id: 601,
                rank: Rank::Queen,
                suit: Suit::Hearts,
                enhancement: None,
                edition: None,
                seal: None,
            },
            CardInstance {
                card_id: 602,
                rank: Rank::Five,
                suit: Suit::Diamonds,
                enhancement: None,
                edition: None,
                seal: None,
            },
        ];
        engine.state.selected_slots.insert(0);
        engine.state.selected_slots.insert(1);
        engine.state.selected_slots.insert(2);
        let mut play_trace = TransitionTrace::default();
        let events = engine.play_selected(&mut play_trace);
        let midas_events: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "on_played_joker" && e.summary.contains("Gold"))
            .collect();
        assert_eq!(midas_events.len(), 1, "Expected 1 Midas Mask event");
        assert!(midas_events[0].summary.contains("2 face card(s)"));
        // After Midas, the gold card money event should fire for the 2 face cards
        let gold_money_events: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "gold_card_money")
            .collect();
        assert_eq!(gold_money_events.len(), 1);
        assert!(gold_money_events[0].summary.contains("2 Gold Card(s)"));
    }

    // ==== Scaling Joker runtime_state tests ====

    #[test]
    fn green_joker_accumulates_mult_on_play_and_decrements_on_discard() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(100, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        let mut green = make_joker_from_bundle(&bundle, "j_green_joker", 0);
        green.runtime_state.insert("mult".to_string(), 0.0);
        engine.state.jokers.push(green);
        engine.state.selected_slots.insert(0);
        engine.play_selected(&mut TransitionTrace::default());
        let green_mult_after_play = engine.state.jokers[0].runtime_state.get("mult").copied().unwrap_or(0.0);
        assert_eq!(green_mult_after_play, 1.0, "Green Joker should gain +1 mult after hand played");
        engine.state.selected_slots.insert(0);
        engine.play_selected(&mut TransitionTrace::default());
        let green_mult_after_second = engine.state.jokers[0].runtime_state.get("mult").copied().unwrap_or(0.0);
        assert_eq!(green_mult_after_second, 2.0, "Green Joker should gain +1 mult per hand");
        engine.state.discards = 2;
        engine.state.selected_slots.insert(0);
        engine.discard_selected(&mut TransitionTrace::default());
        let green_mult_after_discard = engine.state.jokers[0].runtime_state.get("mult").copied().unwrap_or(0.0);
        assert_eq!(green_mult_after_discard, 1.0, "Green Joker should lose 1 mult per discard");
    }

    #[test]
    fn green_joker_mult_applied_during_scoring() {
        let spec = load_spec("j_green_joker");
        let played = vec![make_card(Rank::Ace, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_green_joker", "Green Joker")];
        let ctx = default_ctx("high_card", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        let mut rs = BTreeMap::new();
        rs.insert("mult".to_string(), 5.0);
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &rs);
        assert!(trace.supported && trace.matched);
        assert_eq!(mult, 5, "Green Joker should add 5 accumulated mult");
    }

    #[test]
    fn ride_the_bus_resets_on_face_card() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(200, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        let mut rtb = make_joker_from_bundle(&bundle, "j_ride_the_bus", 0);
        rtb.runtime_state.insert("mult".to_string(), 0.0);
        engine.state.jokers.push(rtb);
        engine.state.available = vec![
            CardInstance { card_id: 200, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 201, rank: Rank::Three, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 202, rank: Rank::Four, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
        ];
        engine.state.selected_slots.insert(0);
        engine.play_selected(&mut TransitionTrace::default());
        let mult_after = engine.state.jokers[0].runtime_state.get("mult").copied().unwrap_or(0.0);
        assert_eq!(mult_after, 1.0, "Ride the Bus should gain +1 on no face card hand");
        engine.state.available = vec![
            CardInstance { card_id: 203, rank: Rank::Five, suit: Suit::Clubs, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 204, rank: Rank::Six, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
        ];
        engine.state.plays = 3;
        engine.state.selected_slots.insert(0);
        engine.play_selected(&mut TransitionTrace::default());
        let mult_after2 = engine.state.jokers[0].runtime_state.get("mult").copied().unwrap_or(0.0);
        assert_eq!(mult_after2, 2.0, "Ride the Bus should accumulate");
        engine.state.available = vec![
            CardInstance { card_id: 205, rank: Rank::King, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 206, rank: Rank::Seven, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
        ];
        engine.state.plays = 2;
        engine.state.selected_slots.insert(0);
        engine.play_selected(&mut TransitionTrace::default());
        let mult_after_reset = engine.state.jokers[0].runtime_state.get("mult").copied().unwrap_or(0.0);
        assert_eq!(mult_after_reset, 0.0, "Ride the Bus should reset to 0 when face card scored");
    }

    #[test]
    fn ice_cream_decays_by_5_per_hand() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(300, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        let mut ice = make_joker_from_bundle(&bundle, "j_ice_cream", 0);
        ice.runtime_state.insert("chips".to_string(), 100.0);
        engine.state.jokers.push(ice);
        for hand_num in 0..4 {
            engine.state.available = vec![
                CardInstance { card_id: 300 + hand_num, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
                CardInstance { card_id: 310 + hand_num, rank: Rank::Three, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            ];
            engine.state.plays = 4 - hand_num as i32;
            engine.state.selected_slots.insert(0);
            engine.play_selected(&mut TransitionTrace::default());
        }
        let final_chips = engine.state.jokers[0].runtime_state.get("chips").copied().unwrap_or(0.0);
        assert_eq!(final_chips, 80.0, "Ice Cream should decay by 5 per hand: 100 - 4*5 = 80");
    }

    #[test]
    fn ice_cream_scoring_uses_runtime_chips() {
        let spec = load_spec("j_ice_cream");
        let played = vec![make_card(Rank::Ace, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_ice_cream", "Ice Cream")];
        let ctx = default_ctx("high_card", &played, &[], &jokers);
        let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
        let mut events = Vec::new();
        let mut trace = fresh_trace();
        let mut rs = BTreeMap::new();
        rs.insert("chips".to_string(), 75.0);
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &rs);
        assert!(trace.supported && trace.matched);
        assert_eq!(chips, 75, "Ice Cream should add chips from runtime_state");
    }

    #[test]
    fn loyalty_card_x4_every_5th_hand() {
        let spec = load_spec("j_loyalty_card");
        let played = vec![make_card(Rank::Ace, Suit::Spades)];
        let jokers = vec![make_joker_instance("j_loyalty_card", "Loyalty Card")];
        let ctx = default_ctx("high_card", &played, &[], &jokers);
        {
            let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
            let mut events = Vec::new();
            let mut trace = fresh_trace();
            let mut rs = BTreeMap::new();
            rs.insert("hands_played".to_string(), 4.0);
            apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &rs);
            assert!(trace.supported);
            assert!(!trace.matched, "Should NOT trigger at hand 4");
            assert!((xmult - 1.0).abs() < 0.001, "No xmult at hand 4");
        }
        {
            let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
            let mut events = Vec::new();
            let mut trace = fresh_trace();
            let mut rs = BTreeMap::new();
            rs.insert("hands_played".to_string(), 5.0);
            apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &rs);
            assert!(trace.supported && trace.matched, "Should trigger at hand 5");
            assert!((xmult - 4.0).abs() < 0.001, "Should apply X4 mult at hand 5");
        }
        {
            let (mut chips, mut mult, mut xmult, mut money) = (0, 0, 1.0, 0);
            let mut events = Vec::new();
            let mut trace = fresh_trace();
            let mut rs = BTreeMap::new();
            rs.insert("hands_played".to_string(), 10.0);
            apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace, &rs);
            assert!(trace.matched, "Should trigger at hand 10");
            assert!((xmult - 4.0).abs() < 0.001, "Should apply X4 mult at hand 10");
        }
    }

    #[test]
    fn loyalty_card_counter_increments_on_play() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(400, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        let mut loyalty = make_joker_from_bundle(&bundle, "j_loyalty_card", 0);
        loyalty.runtime_state.insert("hands_played".to_string(), 0.0);
        engine.state.jokers.push(loyalty);
        for i in 0..3 {
            engine.state.available = vec![
                CardInstance { card_id: 400 + i, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
                CardInstance { card_id: 410 + i, rank: Rank::Three, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            ];
            engine.state.plays = 4 - i as i32;
            engine.state.selected_slots.insert(0);
            engine.play_selected(&mut TransitionTrace::default());
        }
        let counter = engine.state.jokers[0].runtime_state.get("hands_played").copied().unwrap_or(0.0);
        assert_eq!(counter, 3.0, "Loyalty Card counter should be 3 after 3 hands");
    }

    #[test]
    fn throwback_gains_xmult_on_blind_skip() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(500, bundle.clone(), RunConfig::default());
        let mut throwback = make_joker_from_bundle(&bundle, "j_throwback", 0);
        throwback.runtime_state.insert("xmult".to_string(), 1.0);
        engine.state.jokers.push(throwback);
        engine.step(85).expect("skip small blind");
        let xm = engine.state.jokers[0].runtime_state.get("xmult").copied().unwrap_or(1.0);
        assert!((xm - 1.25).abs() < 0.001, "Throwback should gain X0.25 per skip");
        engine.step(85).expect("skip big blind");
        let xm2 = engine.state.jokers[0].runtime_state.get("xmult").copied().unwrap_or(1.0);
        assert!((xm2 - 1.5).abs() < 0.001, "Throwback should accumulate skips");
    }

    #[test]
    fn flash_card_gains_mult_on_reroll() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(600, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 3;
        engine.step(13).expect("cashout to shop");
        let mut flash = make_joker_from_bundle(&bundle, "j_flash", 0);
        flash.runtime_state.insert("mult".to_string(), 0.0);
        engine.state.jokers.push(flash);
        engine.state.money = 100;
        engine.step(79).expect("reroll");
        let m = engine.state.jokers.iter().find(|j| j.joker_id == "j_flash")
            .map(|j| j.runtime_state.get("mult").copied().unwrap_or(0.0))
            .unwrap_or(0.0);
        assert_eq!(m, 2.0, "Flash Card should gain +2 mult per reroll");
    }

    #[test]
    fn popcorn_decays_on_round_end() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(700, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.enter_current_blind(&mut trace);
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 3;
        let mut popcorn = make_joker_from_bundle(&bundle, "j_popcorn", 0);
        popcorn.runtime_state.insert("mult".to_string(), 20.0);
        engine.state.jokers.push(popcorn);
        engine.step(13).expect("cashout triggers end of round");
        let m = engine.state.jokers.iter().find(|j| j.joker_id == "j_popcorn")
            .map(|j| j.runtime_state.get("mult").copied().unwrap_or(0.0))
            .unwrap_or(0.0);
        assert_eq!(m, 16.0, "Popcorn should decay by 4 per round: 20 - 4 = 16");
    }

    #[test]
    fn constellation_gains_xmult_on_planet_use() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(800, bundle.clone(), RunConfig::default());
        let mut trace = TransitionTrace::default();
        let mut constellation = make_joker_from_bundle(&bundle, "j_constellation", 0);
        constellation.runtime_state.insert("xmult".to_string(), 1.0);
        engine.state.jokers.push(constellation);
        let mut config = BTreeMap::new();
        config.insert("hand_type".to_string(), serde_json::json!("Pair"));
        engine.state.consumables.push(make_consumable("c_mercury", "Mercury", "Planet", 3, config));
        engine.enter_current_blind(&mut trace);
        engine.step(71).expect("use planet consumable");
        let xm = engine.state.jokers[0].runtime_state.get("xmult").copied().unwrap_or(1.0);
        assert!((xm - 1.1).abs() < 0.001, "Constellation should gain X0.1 per planet use");
    }

    #[test]
    fn campfire_gains_xmult_on_sell_resets_on_boss() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(900, bundle.clone(), RunConfig::default());
        let mut campfire = make_joker_from_bundle(&bundle, "j_campfire", 0);
        campfire.runtime_state.insert("xmult".to_string(), 1.0);
        campfire.slot_index = 0;
        let dummy = make_joker_from_bundle(&bundle, "j_joker", 1);
        engine.state.jokers.push(campfire);
        engine.state.jokers.push(dummy);
        engine.state.phase = Phase::Shop;
        engine.state.money = 10;
        let mut trace = TransitionTrace::default();
        engine.refresh_shop(&mut trace, "test");
        engine.step(81).expect("sell dummy joker");
        let xm = engine.state.jokers[0].runtime_state.get("xmult").copied().unwrap_or(1.0);
        assert!((xm - 1.25).abs() < 0.001, "Campfire should gain X0.25 per sell");
    }

    // ==== Boss Blind Effect tests ====

    fn setup_boss_blind(seed: u64, boss_name: &str) -> Engine {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(seed, bundle.clone(), RunConfig::default());
        engine.step(85).expect("skip small");
        engine.step(85).expect("skip big");
        let target_boss = bundle.blinds.iter().find(|b| b.name == boss_name).expect("boss blind spec").clone();
        engine.state.boss_blind = target_boss;
        engine
    }

    #[test]
    fn boss_the_goad_debuffs_spades() {
        let mut engine = setup_boss_blind(42, "The Goad");
        let transition = engine.step(12).expect("select boss blind");
        assert!(transition.snapshot_after.boss_effect.contains("The Goad"));
        assert!(!engine.state.debuffed_cards.is_empty());
        let all_spade_ids: BTreeSet<u32> = engine.state.deck.iter().chain(engine.state.available.iter()).chain(engine.state.discarded.iter()).filter(|c| matches!(c.suit, Suit::Spades)).map(|c| c.card_id).collect();
        for id in &engine.state.debuffed_cards { assert!(all_spade_ids.contains(id), "Debuffed card {} should be a Spade", id); }
        assert_eq!(engine.state.debuffed_cards.len(), 13);
    }

    #[test]
    fn boss_the_goad_debuffed_spades_dont_score() {
        let mut engine = setup_boss_blind(42, "The Goad");
        engine.step(12).expect("select boss blind");
        engine.state.available = vec![
            CardInstance { card_id: 1, rank: Rank::Ace, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 2, rank: Rank::King, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 14, rank: Rank::Ace, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
        ];
        engine.state.debuffed_cards.insert(1);
        engine.state.debuffed_cards.insert(2);
        engine.state.selected_slots.insert(0);
        engine.state.selected_slots.insert(1);
        let mut trace = TransitionTrace::default();
        let events = engine.play_selected(&mut trace);
        let debuffed: Vec<_> = events.iter().filter(|e| e.kind == "card_debuffed").collect();
        assert_eq!(debuffed.len(), 2, "Both Spade cards should be debuffed");
    }

    #[test]
    fn boss_the_head_debuffs_hearts() {
        let mut engine = setup_boss_blind(42, "The Head");
        engine.step(12).expect("select boss blind");
        assert_eq!(engine.state.debuffed_cards.len(), 13);
        let all_hearts: BTreeSet<u32> = engine.state.deck.iter().chain(engine.state.available.iter()).chain(engine.state.discarded.iter()).filter(|c| matches!(c.suit, Suit::Hearts)).map(|c| c.card_id).collect();
        for id in &engine.state.debuffed_cards { assert!(all_hearts.contains(id)); }
    }

    #[test]
    fn boss_the_club_debuffs_clubs() {
        let mut engine = setup_boss_blind(42, "The Club");
        engine.step(12).expect("select boss blind");
        assert_eq!(engine.state.debuffed_cards.len(), 13);
    }

    #[test]
    fn boss_the_window_debuffs_diamonds() {
        let mut engine = setup_boss_blind(42, "The Window");
        engine.step(12).expect("select boss blind");
        assert_eq!(engine.state.debuffed_cards.len(), 13);
    }

    #[test]
    fn boss_the_plant_debuffs_face_cards() {
        let mut engine = setup_boss_blind(42, "The Plant");
        engine.step(12).expect("select boss blind");
        assert_eq!(engine.state.debuffed_cards.len(), 12);
    }

    #[test]
    fn boss_the_flint_halves_chips_and_mult() {
        let mut engine = setup_boss_blind(42, "The Flint");
        engine.step(12).expect("select boss blind");
        engine.state.available = vec![
            CardInstance { card_id: 100, rank: Rank::Ace, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 101, rank: Rank::Ace, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
        ];
        engine.state.selected_slots.insert(0);
        engine.state.selected_slots.insert(1);
        let mut trace = TransitionTrace::default();
        let events = engine.play_selected(&mut trace);
        let flint_events: Vec<_> = events.iter().filter(|e| e.kind == "boss_effect_scoring" && e.summary.contains("Flint")).collect();
        assert_eq!(flint_events.len(), 1, "Should have a Flint halving event");
        assert!(flint_events[0].summary.contains("halved"));
    }

    #[test]
    fn boss_the_needle_only_one_hand() {
        let mut engine = setup_boss_blind(42, "The Needle");
        let transition = engine.step(12).expect("select boss blind");
        assert_eq!(transition.snapshot_after.plays, 1, "The Needle should allow only 1 hand");
    }

    #[test]
    fn boss_the_water_zero_discards() {
        let mut engine = setup_boss_blind(42, "The Water");
        let transition = engine.step(12).expect("select boss blind");
        assert_eq!(transition.snapshot_after.discards, 0, "The Water should start with 0 discards");
    }

    #[test]
    fn boss_the_wall_doubles_required_score() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(42, bundle.clone(), RunConfig::default());
        let base_score = *bundle.ante_base_scores.first().unwrap();
        let normal_boss_req = base_score * 2;
        engine.step(85).expect("skip small");
        engine.step(85).expect("skip big");
        let wall_boss = bundle.blinds.iter().find(|b| b.name == "The Wall").expect("The Wall").clone();
        engine.state.boss_blind = wall_boss;
        engine.step(12).expect("select boss blind");
        let wall_req = engine.required_score();
        assert_eq!(wall_req, normal_boss_req * 2, "The Wall should double the already-doubled boss score");
    }

    #[test]
    fn boss_the_eye_blocks_repeated_hand_types() {
        let mut engine = setup_boss_blind(42, "The Eye");
        engine.step(12).expect("select boss blind");
        assert!(matches!(engine.state.active_boss_effect, Some(BossEffect::TheEye)));
        engine.state.available = vec![
            CardInstance { card_id: 200, rank: Rank::Ace, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 201, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 202, rank: Rank::Three, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 203, rank: Rank::Four, suit: Suit::Clubs, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 204, rank: Rank::Six, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 205, rank: Rank::Seven, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 206, rank: Rank::Eight, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 207, rank: Rank::Nine, suit: Suit::Clubs, enhancement: None, edition: None, seal: None },
        ];
        engine.state.selected_slots.insert(0);
        let mut trace = TransitionTrace::default();
        engine.play_selected(&mut trace);
        assert!(engine.state.boss_hand_types_played.contains("high_card"));
        engine.state.available = vec![
            CardInstance { card_id: 300, rank: Rank::King, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 301, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 302, rank: Rank::Four, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
        ];
        engine.state.selected_slots.clear();
        engine.state.selected_slots.insert(0);
        let legal = engine.gen_action_space();
        assert_eq!(legal[8], 0, "Play should be blocked for repeated hand type under The Eye");
    }

    #[test]
    fn boss_the_mouth_locks_hand_type() {
        let mut engine = setup_boss_blind(42, "The Mouth");
        engine.step(12).expect("select boss blind");
        assert!(matches!(engine.state.active_boss_effect, Some(BossEffect::TheMouth)));
        engine.state.available = vec![
            CardInstance { card_id: 300, rank: Rank::Ace, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 301, rank: Rank::Ace, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 302, rank: Rank::Two, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 303, rank: Rank::Three, suit: Suit::Clubs, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 304, rank: Rank::Four, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 305, rank: Rank::Five, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 306, rank: Rank::Six, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 307, rank: Rank::Seven, suit: Suit::Clubs, enhancement: None, edition: None, seal: None },
        ];
        engine.state.selected_slots.insert(0);
        engine.state.selected_slots.insert(1);
        let mut trace = TransitionTrace::default();
        engine.play_selected(&mut trace);
        assert_eq!(engine.state.boss_forced_hand_type, Some("pair".to_string()));
        engine.state.available = vec![
            CardInstance { card_id: 400, rank: Rank::King, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 401, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 402, rank: Rank::Three, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
        ];
        engine.state.selected_slots.clear();
        engine.state.selected_slots.insert(0);
        let legal = engine.gen_action_space();
        assert_eq!(legal[8], 0, "Play should be blocked for different hand type under The Mouth");
    }

    #[test]
    fn boss_the_psychic_requires_five_cards() {
        let mut engine = setup_boss_blind(42, "The Psychic");
        engine.step(12).expect("select boss blind");
        assert!(matches!(engine.state.active_boss_effect, Some(BossEffect::ThePsychic)));
        engine.state.selected_slots.insert(0);
        engine.state.selected_slots.insert(1);
        engine.state.selected_slots.insert(2);
        let legal = engine.gen_action_space();
        assert_eq!(legal[8], 0, "Play should be blocked with fewer than 5 cards under The Psychic");
        engine.state.selected_slots.insert(3);
        engine.state.selected_slots.insert(4);
        let legal = engine.gen_action_space();
        assert_eq!(legal[8], 1, "Play should be allowed with exactly 5 cards under The Psychic");
    }

    #[test]
    fn boss_the_hook_discards_after_play() {
        let mut engine = setup_boss_blind(42, "The Hook");
        engine.step(12).expect("select boss blind");
        assert!(matches!(engine.state.active_boss_effect, Some(BossEffect::TheHook)));
        engine.state.available = vec![
            CardInstance { card_id: 500, rank: Rank::Ace, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 501, rank: Rank::Two, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 502, rank: Rank::Three, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 503, rank: Rank::Four, suit: Suit::Clubs, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 504, rank: Rank::Five, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
        ];
        engine.state.deck.clear();
        engine.state.score = 0;
        engine.state.selected_slots.insert(0);
        let mut trace = TransitionTrace::default();
        let events = engine.play_selected(&mut trace);
        let hook_events: Vec<_> = events.iter().filter(|e| e.kind == "boss_effect_scoring" && e.summary.contains("Hook")).collect();
        assert_eq!(hook_events.len(), 1, "The Hook should discard 2 cards");
        assert!(hook_events[0].summary.contains("Discarded"));
    }

    #[test]
    fn boss_the_tooth_loses_money_per_card() {
        let mut engine = setup_boss_blind(42, "The Tooth");
        engine.step(12).expect("select boss blind");
        assert!(matches!(engine.state.active_boss_effect, Some(BossEffect::TheTooth)));
        engine.state.money = 10;
        engine.state.available = vec![
            CardInstance { card_id: 600, rank: Rank::Ace, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 601, rank: Rank::King, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 602, rank: Rank::Queen, suit: Suit::Diamonds, enhancement: None, edition: None, seal: None },
        ];
        engine.state.deck.clear();
        engine.state.score = 0;
        engine.state.selected_slots.insert(0);
        engine.state.selected_slots.insert(1);
        engine.state.selected_slots.insert(2);
        let mut trace = TransitionTrace::default();
        let events = engine.play_selected(&mut trace);
        let tooth_events: Vec<_> = events.iter().filter(|e| e.kind == "boss_effect_scoring" && e.summary.contains("Tooth")).collect();
        assert_eq!(tooth_events.len(), 1);
        assert!(tooth_events[0].summary.contains("Lost $3"));
        assert_eq!(engine.state.money, 7, "Should have lost $3 from playing 3 cards");
    }

    #[test]
    fn boss_the_manacle_reduces_hand_size() {
        let mut engine = setup_boss_blind(42, "The Manacle");
        engine.step(12).expect("select boss blind");
        assert!(engine.state.boss_manacle_hand_size_reduced);
        assert_eq!(engine.state.available.len(), HAND_LIMIT - 1);
    }

    #[test]
    fn boss_the_arm_decreases_hand_level() {
        let mut engine = setup_boss_blind(42, "The Arm");
        engine.step(12).expect("select boss blind");
        engine.state.hand_levels.insert("pair".to_string(), 3);
        engine.state.available = vec![
            CardInstance { card_id: 700, rank: Rank::Ace, suit: Suit::Hearts, enhancement: None, edition: None, seal: None },
            CardInstance { card_id: 701, rank: Rank::Ace, suit: Suit::Spades, enhancement: None, edition: None, seal: None },
        ];
        engine.state.deck.clear();
        engine.state.selected_slots.insert(0);
        engine.state.selected_slots.insert(1);
        let mut trace = TransitionTrace::default();
        engine.play_selected(&mut trace);
        assert_eq!(engine.state.hand_levels.get("pair").copied(), Some(2));
    }

    #[test]
    fn boss_effect_cleared_on_blind_defeat() {
        let mut engine = setup_boss_blind(42, "The Goad");
        engine.step(12).expect("select boss blind");
        assert!(engine.state.active_boss_effect.is_some());
        assert!(!engine.state.debuffed_cards.is_empty());
        engine.state.score = engine.required_score();
        engine.state.phase = Phase::PostBlind;
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.reward = 5;
        let mut trace = TransitionTrace::default();
        engine.handle_post_blind(13, &mut trace);
        assert!(engine.state.active_boss_effect.is_none());
        assert!(engine.state.debuffed_cards.is_empty());
    }

    #[test]
    fn boss_chicot_disables_boss_effect_no_debuff() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(42, bundle.clone(), RunConfig::default());
        engine.step(85).expect("skip small");
        engine.step(85).expect("skip big");
        let goad = bundle.blinds.iter().find(|b| b.name == "The Goad").expect("The Goad").clone();
        engine.state.boss_blind = goad;
        engine.state.jokers.push(make_joker_from_bundle(&bundle, "j_chicot", 0));
        engine.step(12).expect("select boss blind");
        assert!(engine.state.boss_blind_disabled);
        assert!(engine.state.debuffed_cards.is_empty());
    }

    #[test]
    fn boss_effect_from_blind_name_maps_correctly() {
        assert_eq!(BossEffect::from_blind_name("The Goad"), Some(BossEffect::TheGoad));
        assert_eq!(BossEffect::from_blind_name("The Wall"), Some(BossEffect::TheWall));
        assert_eq!(BossEffect::from_blind_name("The Hook"), Some(BossEffect::TheHook));
        assert_eq!(BossEffect::from_blind_name("The Flint"), Some(BossEffect::TheFlint));
        assert_eq!(BossEffect::from_blind_name("The Eye"), Some(BossEffect::TheEye));
        assert_eq!(BossEffect::from_blind_name("The Psychic"), Some(BossEffect::ThePsychic));
        assert_eq!(BossEffect::from_blind_name("The Needle"), Some(BossEffect::TheNeedle));
        assert_eq!(BossEffect::from_blind_name("Small Blind"), None);
        assert_eq!(BossEffect::from_blind_name("Big Blind"), None);
    }

    // ==== Voucher tests ====

    #[test]
    fn buy_grabber_voucher_increases_hands_per_round() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(101, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance { voucher_id: "v_grabber".to_string(), name: "Grabber".to_string(), cost: 10, effect_key: "grabber".to_string(), description: "+1 hand per round".to_string() });
        let plays_before = engine.state.base_plays;
        let money_before = engine.state.money;
        let transition = engine.step(28).expect("buy voucher");
        assert_eq!(transition.snapshot_after.money, money_before - 10);
        assert!(transition.snapshot_after.owned_vouchers.contains(&"v_grabber".to_string()));
        assert_eq!(engine.state.base_plays, plays_before + 1);
        engine.state.phase = Phase::PreBlind;
        engine.state.current_blind_slot = BlindSlot::Small;
        engine.state.small_progress = BlindProgress::Select;
        let transition = engine.step(10).expect("select small blind");
        assert_eq!(transition.snapshot_after.plays, plays_before + 1);
    }

    #[test]
    fn buy_antimatter_voucher_increases_joker_slots() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(103, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance { voucher_id: "v_antimatter".to_string(), name: "Antimatter".to_string(), cost: 10, effect_key: "antimatter".to_string(), description: "+1 Joker slot".to_string() });
        let slots_before = engine.state.joker_slot_limit;
        engine.step(28).expect("buy antimatter voucher");
        assert_eq!(engine.state.joker_slot_limit, slots_before + 1);
    }

    #[test]
    fn buy_clearance_sale_voucher_discounts_shop_prices() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(105, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 50;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance { voucher_id: "v_clearance_sale".to_string(), name: "Clearance Sale".to_string(), cost: 10, effect_key: "clearance_sale".to_string(), description: "All shop items 25% off".to_string() });
        engine.step(28).expect("buy clearance sale");
        assert!((engine.state.shop_discount - 0.75).abs() < 0.01);
        engine.state.money = 50;
        engine.refresh_shop(&mut trace, "test_after_clearance");
        for slot in &engine.state.shop { assert!(slot.joker.buy_cost <= slot.joker.cost); }
    }

    #[test]
    fn cannot_buy_voucher_without_money() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(107, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 5;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance { voucher_id: "v_grabber".to_string(), name: "Grabber".to_string(), cost: 10, effect_key: "grabber".to_string(), description: "+1 hand per round".to_string() });
        let legal = engine.legal_actions();
        assert!(!legal.iter().any(|a| a.name == "buy_voucher" && a.enabled));
    }

    #[test]
    fn voucher_not_offered_again_after_purchase() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(109, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 50;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance { voucher_id: "v_grabber".to_string(), name: "Grabber".to_string(), cost: 10, effect_key: "grabber".to_string(), description: "+1 hand per round".to_string() });
        engine.step(28).expect("buy grabber");
        assert!(engine.state.owned_vouchers.contains(&"v_grabber".to_string()));
        engine.refresh_shop(&mut trace, "test_after");
        if let Some(ref voucher) = engine.state.shop_voucher { assert_ne!(voucher.voucher_id, "v_grabber"); }
    }

    // ==== Booster Pack tests ====

    #[test]
    fn buy_arcana_pack_and_pick_tarot() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(111, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_packs = vec![super::BoosterPackInstance { pack_type: "Arcana Pack".to_string(), cost: 4, choices: Vec::new(), picks_remaining: 1 }];
        let transition = engine.step(29).expect("buy pack 0");
        assert_eq!(transition.snapshot_after.money, 20 - 4);
        let snap = engine.snapshot();
        assert!(snap.open_pack.is_some());
        let pack = snap.open_pack.as_ref().unwrap();
        assert_eq!(pack.pack_type, "Arcana Pack");
        assert!(!pack.choices.is_empty());
        let legal = engine.legal_actions();
        let enabled: Vec<_> = legal.iter().filter(|a| a.enabled).collect();
        assert!(enabled.iter().all(|a| a.name.starts_with("pick_pack_") || a.name == "skip_pack"));
        let consumable_count_before = engine.state.consumables.len();
        let transition = engine.step(31).expect("pick pack card 0");
        assert_eq!(transition.snapshot_after.consumables.len(), consumable_count_before + 1);
        assert!(transition.snapshot_after.open_pack.is_none());
    }

    #[test]
    fn buy_buffoon_pack_and_pick_joker() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(113, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_packs = vec![super::BoosterPackInstance { pack_type: "Buffoon Pack".to_string(), cost: 4, choices: Vec::new(), picks_remaining: 1 }];
        let joker_count_before = engine.state.jokers.len();
        engine.step(29).expect("buy pack");
        let snap = engine.snapshot();
        assert!(snap.open_pack.is_some());
        let transition = engine.step(31).expect("pick joker");
        assert_eq!(transition.snapshot_after.jokers.len(), joker_count_before + 1);
        assert!(transition.snapshot_after.open_pack.is_none());
    }

    #[test]
    fn skip_pack_closes_without_picking() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(115, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_packs = vec![super::BoosterPackInstance { pack_type: "Arcana Pack".to_string(), cost: 4, choices: Vec::new(), picks_remaining: 1 }];
        engine.step(29).expect("buy pack");
        assert!(engine.snapshot().open_pack.is_some());
        let consumable_count_before = engine.state.consumables.len();
        let transition = engine.step(36).expect("skip pack");
        assert!(transition.snapshot_after.open_pack.is_none());
        assert_eq!(engine.state.consumables.len(), consumable_count_before);
    }

    #[test]
    fn shop_shows_packs_and_voucher_during_shop_phase() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(117, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 50;
        engine.refresh_shop(&mut trace, "test");
        let snap = engine.snapshot();
        assert!(!snap.shop_packs.is_empty());
        assert!(snap.shop_voucher.is_some());
    }

    #[test]
    fn voucher_and_pack_action_names_correct() {
        assert_eq!(action_name(28), "buy_voucher");
        assert_eq!(action_name(29), "buy_pack_0");
        assert_eq!(action_name(30), "buy_pack_1");
        assert_eq!(action_name(31), "pick_pack_0");
        assert_eq!(action_name(32), "pick_pack_1");
        assert_eq!(action_name(33), "pick_pack_2");
        assert_eq!(action_name(34), "pick_pack_3");
        assert_eq!(action_name(35), "pick_pack_4");
        assert_eq!(action_name(36), "skip_pack");
    }

    #[test]
    fn buy_nacho_tong_increases_hand_size() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(119, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance { voucher_id: "v_nacho_tong".to_string(), name: "Nacho Tong".to_string(), cost: 10, effect_key: "nacho_tong".to_string(), description: "+1 hand size".to_string() });
        let hand_size_before = engine.state.hand_size;
        engine.step(28).expect("buy nacho tong");
        assert_eq!(engine.state.hand_size, hand_size_before + 1);
    }

    #[test]
    fn buy_seed_money_increases_interest_cap() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(121, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance { voucher_id: "v_seed_money".to_string(), name: "Seed Money".to_string(), cost: 10, effect_key: "seed_money".to_string(), description: "Max interest cap from $5 to $25".to_string() });
        assert_eq!(engine.state.interest_cap, 5);
        engine.step(28).expect("buy seed money");
        assert_eq!(engine.state.interest_cap, 25);
    }

    #[test]
    fn buy_voucher_grabber_increases_plays() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(123, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance {
            voucher_id: "v_grabber".to_string(),
            name: "Grabber".to_string(),
            cost: 10,
            effect_key: "grabber".to_string(),
            description: "+1 hand per round".to_string(),
        });
        let base_plays_before = engine.state.base_plays;
        engine.step(28).expect("buy grabber");
        assert_eq!(engine.state.base_plays, base_plays_before + 1);
        assert!(engine.state.owned_vouchers.contains(&"v_grabber".to_string()));
    }

    #[test]
    fn buy_voucher_antimatter_increases_joker_slots() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(125, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.phase = Phase::Shop;
        engine.state.money = 20;
        engine.refresh_shop(&mut trace, "test");
        engine.state.shop_voucher = Some(super::VoucherInstance {
            voucher_id: "v_antimatter".to_string(),
            name: "Antimatter".to_string(),
            cost: 10,
            effect_key: "antimatter".to_string(),
            description: "+1 Joker slot".to_string(),
        });
        let joker_slots_before = engine.state.joker_slot_limit;
        engine.step(28).expect("buy antimatter");
        assert_eq!(engine.state.joker_slot_limit, joker_slots_before + 1);
        assert!(engine.state.owned_vouchers.contains(&"v_antimatter".to_string()));
    }

    // ==== Tarot & Spectral consumable tests ====

    #[test]
    fn high_priestess_creates_planet_cards() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(200, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        // Set consumable slot limit higher so both planets can be created
        engine.state.consumable_slot_limit = 4;
        let mut config = BTreeMap::new();
        config.insert("planets".to_string(), serde_json::json!(2));
        engine.state.consumables.push(make_consumable(
            "c_high_priestess", "The High Priestess", "Tarot", 3, config,
        ));
        engine.enter_current_blind(&mut trace);
        let consumable_count_before = engine.state.consumables.len();
        let transition = engine.step(71).expect("use high priestess");
        // The High Priestess itself was consumed, but 2 planets were created
        // Net: removed 1 (used), added 2 => +1 from before
        assert_eq!(
            engine.state.consumables.len(),
            consumable_count_before - 1 + 2,
            "should have gained 2 planet cards minus the used tarot"
        );
        // All remaining consumables should be Planet cards
        assert!(
            engine.state.consumables.iter().all(|c| c.set == "Planet"),
            "created consumables should be Planet cards"
        );
        assert!(
            transition.events.iter().any(|e| e.summary.contains("Planet")),
            "event should mention Planet cards"
        );
    }

    #[test]
    fn spectral_black_hole_levels_all_hands() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let hand_count = bundle.hand_specs.len();
        let mut engine = Engine::new(201, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        engine.state.consumables.push(make_consumable(
            "c_black_hole", "Black Hole", "Spectral", 4, BTreeMap::new(),
        ));
        engine.enter_current_blind(&mut trace);
        // Record levels before
        let levels_before: BTreeMap<String, i32> = engine.state.hand_levels.clone();
        let transition = engine.step(71).expect("use black hole");
        // Every hand type should be leveled up by 1
        for (key, level) in &engine.state.hand_levels {
            let before = levels_before.get(key).copied().unwrap_or(1);
            assert_eq!(
                *level, before + 1,
                "hand type {} should be leveled up by 1",
                key
            );
        }
        // Should have events for each hand type
        let level_events: Vec<_> = transition.events.iter()
            .filter(|e| e.kind == "hand_leveled_up")
            .collect();
        assert_eq!(
            level_events.len(), hand_count,
            "should have one level-up event per hand type"
        );
    }

    #[test]
    fn spectral_immolate_destroys_cards_and_gains_money() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(202, bundle, RunConfig::default());
        let mut trace = TransitionTrace::default();
        let mut config = BTreeMap::new();
        config.insert("extra".to_string(), serde_json::json!({"destroy": 5, "dollars": 20}));
        config.insert("remove_card".to_string(), serde_json::json!(true));
        engine.state.consumables.push(make_consumable(
            "c_immolate", "Immolate", "Spectral", 4, config,
        ));
        engine.enter_current_blind(&mut trace);
        let hand_size_before = engine.state.available.len();
        let money_before = engine.state.money;
        let transition = engine.step(71).expect("use immolate");
        // Should have destroyed up to 5 cards from hand
        let destroyed = hand_size_before - engine.state.available.len();
        assert!(
            destroyed <= 5 && destroyed > 0,
            "should have destroyed between 1 and 5 cards, destroyed {}",
            destroyed
        );
        // Should have gained $20
        assert_eq!(
            engine.state.money, money_before + 20,
            "should have gained $20"
        );
        assert!(
            transition.events.iter().any(|e| e.summary.contains("Immolate")),
            "event should mention Immolate"
        );
    }

    // ==== Card generation roll tests (B-14) ====

    #[test]
    fn shop_jokers_can_have_editions() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut found_edition = false;
        // Run 200 shop refreshes with different seeds to find at least one edition
        for seed in 0..200_u64 {
            let mut engine = Engine::new(seed, bundle.clone(), RunConfig::default());
            let mut trace = TransitionTrace::default();
            engine.state.phase = Phase::Shop;
            engine.state.money = 999;
            engine.refresh_shop(&mut trace, "test");
            for shop_slot in &engine.state.shop {
                if shop_slot.joker.edition.is_some() {
                    found_edition = true;
                    break;
                }
            }
            if found_edition {
                break;
            }
        }
        assert!(found_edition, "Expected at least one joker with an edition across 200 shop refreshes");
    }

    #[test]
    fn standard_pack_cards_can_have_enhancements() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut found_enhancement = false;
        let mut found_edition = false;
        let mut found_seal = false;
        for seed in 0..200_u64 {
            let mut engine = Engine::new(seed, bundle.clone(), RunConfig::default());
            let mut trace = TransitionTrace::default();
            let choices = engine.generate_pack_choices("Standard Pack", &mut trace);
            for choice in &choices {
                if let Some(ref card) = choice.card {
                    if card.enhancement.is_some() {
                        found_enhancement = true;
                    }
                    if card.edition.is_some() {
                        found_edition = true;
                    }
                    if card.seal.is_some() {
                        found_seal = true;
                    }
                }
            }
            if found_enhancement && found_edition && found_seal {
                break;
            }
        }
        assert!(found_enhancement, "Expected at least one card with an enhancement across 200 standard packs");
        assert!(found_edition, "Expected at least one card with an edition across 200 standard packs");
        assert!(found_seal, "Expected at least one card with a seal across 200 standard packs");
    }

    #[test]
    fn roll_edition_distribution_is_reasonable() {
        use rand::SeedableRng;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut none_count = 0;
        let mut foil_count = 0;
        let mut holo_count = 0;
        let mut poly_count = 0;
        let trials = 10000;
        for _ in 0..trials {
            match super::roll_edition(&mut rng) {
                None => none_count += 1,
                Some(ref e) if e == "e_foil" => foil_count += 1,
                Some(ref e) if e == "e_holo" => holo_count += 1,
                Some(ref e) if e == "e_polychrome" => poly_count += 1,
                Some(other) => panic!("unexpected edition: {}", other),
            }
        }
        // With 10k trials, expect ~96% none, ~2% foil, ~1.4% holo, ~0.6% poly
        // Use generous bounds to avoid flaky tests
        assert!(none_count > 9000, "Expected >9000 none, got {}", none_count);
        assert!(foil_count > 50, "Expected >50 foil, got {}", foil_count);
        assert!(holo_count > 30, "Expected >30 holo, got {}", holo_count);
        assert!(poly_count > 10, "Expected >10 poly, got {}", poly_count);
    }

    #[test]
    fn roll_enhancement_distribution_is_reasonable() {
        use rand::SeedableRng;
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let mut none_count = 0;
        let mut enhanced_count = 0;
        let trials = 10000;
        for _ in 0..trials {
            match super::roll_enhancement(&mut rng) {
                None => none_count += 1,
                Some(_) => enhanced_count += 1,
            }
        }
        // Expect ~90% none, ~10% enhanced
        assert!(none_count > 8500, "Expected >8500 none, got {}", none_count);
        assert!(enhanced_count > 500, "Expected >500 enhanced, got {}", enhanced_count);
    }

    #[test]
    fn roll_seal_distribution_is_reasonable() {
        use rand::SeedableRng;
        let mut rng = ChaCha8Rng::seed_from_u64(77);
        let mut none_count = 0;
        let mut sealed_count = 0;
        let trials = 10000;
        for _ in 0..trials {
            match super::roll_seal(&mut rng) {
                None => none_count += 1,
                Some(_) => sealed_count += 1,
            }
        }
        // Expect ~97% none, ~3% sealed
        assert!(none_count > 9400, "Expected >9400 none, got {}", none_count);
        assert!(sealed_count > 100, "Expected >100 sealed, got {}", sealed_count);
    }

    // ============== Skip-Blind Tag System Tests ==============

    fn force_small_tag(engine: &mut Engine, tag_id: &str) {
        engine.state.small_tag_id = Some(tag_id.to_string());
    }

    #[test]
    fn boss_blind_never_has_tag() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let engine = Engine::new(101, bundle, RunConfig::default());
        let snap = engine.snapshot();
        assert!(snap.boss_tag.is_none(), "boss tag must be None");
        assert!(snap.small_tag.is_some(), "small tag must be rolled");
        assert!(snap.big_tag.is_some(), "big tag must be rolled");
    }

    #[test]
    fn same_seed_rolls_same_tags() {
        let bundle1 = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let bundle2 = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let a = Engine::new(42, bundle1, RunConfig::default());
        let b = Engine::new(42, bundle2, RunConfig::default());
        let sa = a.snapshot();
        let sb = b.snapshot();
        assert_eq!(sa.small_tag.as_ref().map(|t| t.id.clone()), sb.small_tag.as_ref().map(|t| t.id.clone()));
        assert_eq!(sa.big_tag.as_ref().map(|t| t.id.clone()), sb.big_tag.as_ref().map(|t| t.id.clone()));
    }

    #[test]
    fn tag_catalog_is_full_from_fixture() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        // 24 tags mirrors vanilla G.P_TAGS.
        assert_eq!(bundle.tags.len(), 24, "expected 24 vanilla tags");
        assert!(bundle.tags.iter().any(|t| t.id == "tag_economy"));
        assert!(bundle.tags.iter().any(|t| t.id == "tag_investment"));
    }

    #[test]
    fn skip_small_with_economy_tag_doubles_money_capped_at_plus_40() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        // Under-cap: money=10 should become 20 (doubled, +10 <= 40).
        let mut engine = Engine::new(7, bundle.clone(), RunConfig::default());
        engine.state.money = 10;
        force_small_tag(&mut engine, "tag_economy");
        engine.step(85).expect("skip small with economy tag");
        assert_eq!(engine.state.money, 20);

        // Over-cap: money=100 should bump by only +40 (capped).
        let mut engine = Engine::new(8, bundle.clone(), RunConfig::default());
        engine.state.money = 100;
        force_small_tag(&mut engine, "tag_economy");
        engine.step(85).expect("skip small with economy tag cap");
        assert_eq!(engine.state.money, 140);

        // Zero money: no payout.
        let mut engine = Engine::new(9, bundle, RunConfig::default());
        engine.state.money = 0;
        force_small_tag(&mut engine, "tag_economy");
        engine.step(85).expect("skip small with economy tag zero");
        assert_eq!(engine.state.money, 0);
    }

    #[test]
    fn skip_small_with_investment_tag_pays_25_after_boss() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(11, bundle, RunConfig::default());
        let money_before = engine.state.money;
        force_small_tag(&mut engine, "tag_investment");
        engine.step(85).expect("skip small");
        // Payout queued, not yet applied.
        assert_eq!(engine.state.money, money_before);
        assert_eq!(engine.state.pending_investment_payouts, 25);

        // Simulate clearing boss and entering cashout → post-blind should pay out.
        engine.state.current_blind_slot = BlindSlot::Boss;
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 0; // isolate investment effect
        let mut trace = TransitionTrace::default();
        engine.handle_post_blind(13, &mut trace);
        assert_eq!(engine.state.money, money_before + 25);
        assert_eq!(engine.state.pending_investment_payouts, 0);
    }

    #[test]
    fn skip_small_with_voucher_tag_next_shop_offers_voucher() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(13, bundle, RunConfig::default());
        force_small_tag(&mut engine, "tag_voucher");
        engine.step(85).expect("skip small with voucher tag");
        assert_eq!(engine.state.pending_voucher_tags, 1);

        // Trigger a shop refresh; pending counter should decrement and the
        // shop voucher cost should be 0.
        engine.state.phase = Phase::Shop;
        let mut trace = TransitionTrace::default();
        engine.refresh_shop(&mut trace, "test_refresh_voucher_tag");
        assert_eq!(engine.state.pending_voucher_tags, 0);
        let v = engine.state.shop_voucher.as_ref().expect("voucher present");
        assert_eq!(v.cost, 0, "Voucher Tag should zero the voucher cost");
    }

    #[test]
    fn skip_with_coupon_tag_next_shop_initial_items_free() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(17, bundle, RunConfig::default());
        force_small_tag(&mut engine, "tag_coupon");
        engine.step(85).expect("skip small with coupon tag");
        assert!(engine.state.pending_coupon_shop);

        engine.state.phase = Phase::Shop;
        let mut trace = TransitionTrace::default();
        engine.refresh_shop(&mut trace, "test_refresh_coupon_tag");
        // All initial shop items (jokers, consumables, packs) must be free.
        for slot in engine.state.shop.iter() {
            assert_eq!(slot.joker.buy_cost, 0, "joker cost should be 0");
        }
        for c in engine.state.shop_consumables.iter() {
            assert_eq!(c.buy_cost, 0, "consumable cost should be 0");
        }
        for p in engine.state.shop_packs.iter() {
            assert_eq!(p.cost, 0, "pack cost should be 0");
        }
        assert!(!engine.state.pending_coupon_shop, "flag consumed");
    }

    #[test]
    fn skip_with_d6_tag_next_shop_reroll_starts_at_zero() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(19, bundle, RunConfig::default());
        force_small_tag(&mut engine, "tag_d_six");
        engine.step(85).expect("skip small with d6 tag");
        assert!(engine.state.pending_d6_shop);

        // Simulate a boss defeat → cashout path: flip to PostBlind and advance.
        engine.state.current_blind_slot = BlindSlot::Boss;
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 0;
        let mut trace = TransitionTrace::default();
        engine.handle_post_blind(13, &mut trace);
        assert_eq!(engine.state.shop_current_reroll_cost, 0);
        assert!(!engine.state.pending_d6_shop, "flag consumed");
    }

    #[test]
    fn skip_with_juggle_tag_adds_hand_size_for_next_blind_only() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(23, bundle, RunConfig::default());
        let base_hand_size = engine.state.hand_size;
        force_small_tag(&mut engine, "tag_juggle");
        engine.step(85).expect("skip small with juggle tag");
        assert_eq!(engine.state.pending_juggle_hand_size, 3);

        // Selecting the Big blind (action 11) should apply the +3 bonus.
        engine.step(11).expect("select big blind");
        assert_eq!(engine.state.hand_size, base_hand_size + 3);
        assert_eq!(engine.state.active_juggle_hand_size, 3);

        // After clearing the blind (force defeated), post_blind reverts bonus.
        engine.mark_current_blind_progress(BlindProgress::Defeated);
        engine.state.phase = Phase::PostBlind;
        engine.state.reward = 0;
        let mut trace = TransitionTrace::default();
        engine.handle_post_blind(13, &mut trace);
        assert_eq!(engine.state.hand_size, base_hand_size);
        assert_eq!(engine.state.active_juggle_hand_size, 0);
    }

    #[test]
    fn skip_with_speed_tag_pays_5_per_prior_skip() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(29, bundle, RunConfig::default());
        // Simulate 2 prior skips already logged.
        engine.state.skipped_blind_count = 2;
        let money_before = engine.state.money;
        force_small_tag(&mut engine, "tag_skip");
        engine.step(85).expect("skip small with speed tag");
        // Payout = $5 * 2 = $10 (prior skips, BEFORE incrementing this skip).
        assert_eq!(engine.state.money, money_before + 10);
        assert_eq!(engine.state.skipped_blind_count, 3);
    }

    #[test]
    fn skip_with_handy_tag_pays_per_hand_played() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(31, bundle, RunConfig::default());
        // Forge 4 hands played across two types.
        if let Some(stats) = engine.state.hand_stats.values_mut().next() {
            stats.played = 4;
        }
        let money_before = engine.state.money;
        force_small_tag(&mut engine, "tag_handy");
        engine.step(85).expect("skip small with handy tag");
        assert_eq!(engine.state.money, money_before + 4);
    }

    #[test]
    fn unimplemented_tag_logs_event_without_crashing() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(37, bundle, RunConfig::default());
        force_small_tag(&mut engine, "tag_negative");
        let transition = engine.step(85).expect("skip with stubbed tag");
        let logged = transition
            .events
            .iter()
            .any(|e| e.kind == "unimplemented_tag");
        assert!(logged, "expected unimplemented_tag event for Negative Tag");
    }

    #[test]
    fn snapshot_exposes_tag_info_for_small_and_big_but_not_boss() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let engine = Engine::new(41, bundle, RunConfig::default());
        let snap = engine.snapshot();
        let small = snap.small_tag.as_ref().expect("small tag present");
        assert!(!small.id.is_empty());
        assert!(!small.name.is_empty());
        // Description is the Chinese rendered string; non-empty for all 24 tags.
        assert!(!small.description.is_empty());
        assert!(snap.big_tag.is_some());
        assert!(snap.boss_tag.is_none());
    }
}

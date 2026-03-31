use balatro_spec::{BlindSpec, JokerSpec, RulesetBundle, Seal};
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
    pub blind_states: BTreeMap<String, String>,
    pub selected_slots: Vec<usize>,
    pub won: bool,
    pub over: bool,
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
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            ante_start: 1,
            stake: 1,
            max_ante: 8,
        }
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
}

impl Engine {
    pub fn new(seed: u64, ruleset: RulesetBundle, config: RunConfig) -> Self {
        let initial_boss = ruleset
            .blinds
            .iter()
            .find(|blind| blind.boss)
            .expect("boss blind")
            .clone();
        let mut engine = Self {
            ruleset,
            rng: ChaCha8Rng::seed_from_u64(seed),
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
                won: false,
                over: false,
                rocket_extra_dollars: 0,
                egg_accumulated_sell: 0,
                unique_planets_used: 0,
                boss_blind_disabled: false,
            },
        };
        let mut init_trace = TransitionTrace::default();
        engine.prepare_round_start(&mut init_trace);
        engine
    }

    pub fn clone_seeded(&self, seed: Option<u64>) -> Self {
        let mut clone = self.clone();
        if let Some(seed) = seed {
            clone.rng = ChaCha8Rng::seed_from_u64(seed);
        }
        clone
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
            blind_states: self.blind_states_snapshot(),
            selected_slots: self.state.selected_slots.iter().copied().collect(),
            won: self.state.won,
            over: self.state.over,
        }
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
        match self.state.phase {
            Phase::PreBlind => {
                mask[self.state.current_blind_slot.action_index()] = 1;
                if self.state.current_blind_slot != BlindSlot::Boss {
                    mask[85] = 1;
                }
            }
            Phase::Blind => {
                for index in 0..self.state.available.len().min(HAND_LIMIT) {
                    mask[index] = 1;
                }
                if self.state.plays > 0 {
                    mask[8] = 1;
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
                if self.state.jokers.len() < JOKER_LIMIT {
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
                for slot in 0..self.state.jokers.len().min(JOKER_LIMIT) {
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
            self.mark_current_blind_progress(BlindProgress::Skipped);
            if self.advance_to_next_blind_slot() {
                self.prepare_preblind_state();
            }
            return vec![event(
                EventStage::BlindPrePlay,
                "blind_skipped",
                format!("Skipped {}", skipped_name),
            )];
        }

        Vec::new()
    }

    fn handle_blind(&mut self, action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        if action_index < HAND_LIMIT {
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

        // End-of-round Joker activation (before cashout money is added)
        self.apply_end_of_round_jokers(&mut events, trace);

        self.state.money += self.state.reward;
        let reward = self.state.reward;
        let cleared_boss = self.state.current_blind_slot == BlindSlot::Boss;
        if !cleared_boss {
            self.advance_to_next_blind_slot();
            self.set_active_preblind_progress();
        }
        self.state.phase = Phase::Shop;
        self.state.boss_blind_disabled = false;
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
                if self.state.jokers.len() < JOKER_LIMIT && self.state.money >= shop_slot.joker.cost {
                    self.state.money -= shop_slot.joker.cost;
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
        // Use consumable
        if (71..=78).contains(&action_index) {
            return self.handle_use_consumable(action_index - 71, trace);
        }
        if action_index == 79 {
            self.state.money -= self.state.shop_current_reroll_cost;
            self.state.shop_reroll_count += 1;
            self.state.shop_current_reroll_cost += self.state.shop_base_reroll_cost;
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
        trace.add_transient("DRAW_TO_HAND");
        self.draw_to_hand(HAND_LIMIT);
        self.maybe_fail_blind();
        vec![event(
            EventStage::EndOfHand,
            "discard",
            format!("Discarded {} card(s)", removed.len()),
        )]
    }

    fn play_selected(&mut self, trace: &mut TransitionTrace) -> Vec<Event> {
        let selected = self.selected_cards();
        let played = if selected.is_empty() && !self.state.available.is_empty() {
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
            .expect("known hand spec");

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

        trace.add_transient("HAND_PLAYED");
        trace.retrigger_supported = true;

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
            joker_slot_max: JOKER_LIMIT,
        };

        let mut xmult = 1.0_f64;
        let mut money_delta = 0_i32;

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
                        );
                    }
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

        // Apply xmult to final score
        let base_score = chips * mult;
        let gained = (base_score as f64 * xmult).round() as i32;
        self.state.money += money_delta;
        self.state.score += gained;
        self.state.plays -= 1;

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
            events.push(event(
                EventStage::EndOfHand,
                "blind_cleared",
                format!("Cleared {}", self.state.blind_name),
            ));
        } else {
            trace.add_transient("DRAW_TO_HAND");
            self.draw_to_hand(HAND_LIMIT);
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
            "Spectral" => {
                trace.add_note(format!("spectral_not_implemented: {}", consumable.name));
                vec![event(
                    EventStage::Shop,
                    "consumable_used",
                    format!("Used {} (spectral effect not yet implemented)", consumable.name),
                )]
            }
            _ => {
                trace.add_note(format!("consumable_unknown_set: {}", consumable.set));
                vec![]
            }
        };
        // Remove the consumed item
        if !events.is_empty() {
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
                    for hand_spec in &self.ruleset.hand_specs {
                        let hand_key = hand_spec.key.clone();
                        let level = self.state.hand_levels.entry(hand_key.clone()).or_insert(1);
                        *level += 1;
                        events.push(event(
                            EventStage::Shop,
                            "hand_leveled_up",
                            format!("{} leveled up to Lv.{}", hand_spec.name, level),
                        ));
                    }
                    return events;
                }
                return vec![];
            }
        };
        let hand_key = hand_type_to_key(&hand_type);
        let level = self.state.hand_levels.entry(hand_key.to_string()).or_insert(1);
        *level += 1;
        let new_level = *level;
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
        self.prepare_preblind_state();
    }

    fn prepare_preblind_state(&mut self) {
        self.state.phase = Phase::PreBlind;
        self.state.score = 0;
        self.state.plays = 4;
        self.state.discards = 3;
        self.state.reward = blind_reward_for_slot(self.state.current_blind_slot);
        self.state.selected_slots.clear();
        self.state.discarded.clear();
        self.state.available.clear();
        self.state.deck.clear();
        self.state.shop.clear();
        self.state.shop_consumables.clear();
        self.sync_current_blind_descriptor();
        self.set_active_preblind_progress();
    }

    fn enter_current_blind(&mut self, trace: &mut TransitionTrace) -> Vec<Event> {
        self.state.phase = Phase::Blind;
        self.state.score = 0;
        self.state.plays = 4;
        self.state.discards = 3;
        self.state.selected_slots.clear();
        self.state.discarded.clear();
        self.state.shop.clear();
        self.state.shop_consumables.clear();
        self.state.boss_blind_disabled = false;
        self.sync_current_blind_descriptor();
        self.mark_current_blind_progress(BlindProgress::Current);

        // Boss blind pre-play Joker activation
        let mut blind_select_events = Vec::new();
        self.apply_blind_select_jokers(&mut blind_select_events, trace);

        self.reset_deck(trace);
        self.draw_to_hand(HAND_LIMIT);
        blind_select_events
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
        match self.state.blind {
            BlindKind::Small => base,
            BlindKind::Big => ((base as f32) * 1.5).round() as i32,
            BlindKind::Boss(_) => base * 2,
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
                        edition: None,
                        slot_index: slot,
                        activation_class: spec.activation_class.clone(),
                        wiki_effect_text_en: spec.wiki_effect_text_en.clone(),
                        remaining_uses: initial_remaining_uses(spec),
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
                self.state.shop_consumables.push(ConsumableInstance {
                    consumable_id: spec.id.clone(),
                    name: spec.name.clone(),
                    set: spec.set.clone(),
                    cost: spec.cost,
                    buy_cost: spec.cost,
                    sell_value: (spec.cost / 2).max(1),
                    slot_index: slot,
                    config: spec.config.clone(),
                });
            }
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
                    let interest = (self.state.money / 5 * per_five).min(5);
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
                        if self.state.jokers.len() >= JOKER_LIMIT || common_jokers.is_empty() {
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
                        let new_joker = JokerInstance {
                            joker_id: chosen_spec.id.clone(),
                            name: chosen_spec.name.clone(),
                            base_cost: chosen_spec.base_cost,
                            cost: chosen_spec.cost,
                            buy_cost: chosen_spec.cost,
                            sell_value: (chosen_spec.cost / 2).max(1),
                            extra_sell_value: 0,
                            rarity: chosen_spec.rarity,
                            edition: None,
                            slot_index: self.state.jokers.len(),
                            activation_class: chosen_spec.activation_class.clone(),
                            wiki_effect_text_en: chosen_spec.wiki_effect_text_en.clone(),
                            remaining_uses: initial_remaining_uses(chosen_spec),
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
            if let Some(extra) = config_extra_obj(spec) {
                let base = extra.get("chips").and_then(|v| v.as_i64()).unwrap_or(100) as i32;
                *chips += base; trace.matched = base > 0;
                trace.summary = format!("{} added {} chips (decay TODO)", spec.name, base);
                events.push(event(EventStage::JokerPostScore, "joker_ice_cream", format!("{} added {} chips", spec.name, base)));
            }
        }
        "j_popcorn" => {
            trace.supported = true; trace.effect_key = Some("popcorn".to_string());
            let bonus = spec.config.get("mult").and_then(|v| v.as_i64()).unwrap_or(20) as i32;
            *mult += bonus; trace.matched = bonus > 0;
            trace.summary = format!("{} added {} mult (decay TODO)", spec.name, bonus);
            events.push(event(EventStage::JokerPostScore, "joker_popcorn", format!("{} added {} mult", spec.name, bonus)));
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
        "j_supernova" => { trace.supported = true; trace.effect_key = Some("supernova".to_string()); let times_played = 1_i32; *mult += times_played; trace.matched = true; trace.summary = format!("{} +{} mult (hand play count TODO)", spec.name, times_played); events.push(event(EventStage::JokerPostScore, "joker_supernova", format!("{} added {} mult", spec.name, times_played))); }
        "j_erosion" => {
            let per = config_extra_i64(spec).unwrap_or(4) as i32;
            let deficit = (52_i32 - ctx.full_deck_size).max(0);
            let gained = per * deficit;
            trace.supported = true; trace.matched = gained > 0; trace.effect_key = Some("erosion".to_string());
            trace.summary = format!("{} +{} mult ({} cards below 52)", spec.name, gained, deficit);
            if gained > 0 { *mult += gained; events.push(event(EventStage::JokerPostScore, "joker_erosion", format!("{} added {} mult", spec.name, gained))); }
        }
        "j_ramen" => { trace.supported = true; trace.effect_key = Some("ramen".to_string()); let xm = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(2.0); *xmult *= xm; trace.matched = true; trace.summary = format!("{} X{} (decay TODO)", spec.name, xm); events.push(event(EventStage::JokerPostScore, "joker_ramen", format!("{} applied X{} mult", spec.name, xm))); }
        "j_drivers_license" => { trace.supported = true; trace.matched = false; trace.effect_key = Some("drivers_license".to_string()); trace.summary = format!("{} (0 enhanced cards in default deck)", spec.name); }
        "j_constellation" => { trace.supported = true; trace.effect_key = Some("constellation".to_string()); let xm = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(1.0); if xm > 1.0 { *xmult *= xm; trace.matched = true; } trace.summary = format!("{} X{} (scaling TODO)", spec.name, xm); }
        "j_glass" => { trace.supported = true; trace.effect_key = Some("glass_joker".to_string()); let xm = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(1.0); if xm > 1.0 { *xmult *= xm; trace.matched = true; } trace.summary = format!("{} X{} (scaling TODO)", spec.name, xm); }
        "j_hologram" => { trace.supported = true; trace.effect_key = Some("hologram".to_string()); let xm = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(1.0); if xm > 1.0 { *xmult *= xm; trace.matched = true; } trace.summary = format!("{} X{} (scaling TODO)", spec.name, xm); }
        "j_throwback" => { trace.supported = true; trace.effect_key = Some("throwback".to_string()); trace.summary = format!("{} X1 (blind skip tracking TODO)", spec.name); }
        "j_campfire" => { trace.supported = true; trace.effect_key = Some("campfire".to_string()); trace.summary = format!("{} X1 (card sold tracking TODO)", spec.name); }
        "j_red_card" => { trace.supported = true; trace.effect_key = Some("red_card".to_string()); let bonus = spec.config.get("mult").and_then(|v| v.as_i64()).unwrap_or(0) as i32; if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_red_card", format!("{} added {} mult", spec.name, bonus))); } trace.summary = format!("{} +{} mult (skip tracking TODO)", spec.name, bonus); }
        "j_flash" => { trace.supported = true; trace.effect_key = Some("flash_card".to_string()); let bonus = spec.config.get("mult").and_then(|v| v.as_i64()).unwrap_or(0) as i32; if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_flash", format!("{} added {} mult", spec.name, bonus))); } trace.summary = format!("{} +{} mult (reroll tracking TODO)", spec.name, bonus); }
        "j_fortune_teller" => { trace.supported = true; trace.effect_key = Some("fortune_teller".to_string()); trace.summary = format!("{} +0 mult (tarot tracking TODO)", spec.name); }
        "j_green_joker" => { trace.supported = true; trace.effect_key = Some("green_joker".to_string()); trace.summary = format!("{} +0 mult (hand/discard tracking TODO)", spec.name); }
        "j_ride_the_bus" => { trace.supported = true; trace.effect_key = Some("ride_the_bus".to_string()); trace.summary = format!("{} +0 mult (consecutive tracking TODO)", spec.name); }
        "j_card_sharp" => { trace.supported = true; trace.effect_key = Some("card_sharp".to_string()); trace.summary = format!("{} (round hand tracking TODO)", spec.name); }
        "j_madness" => { trace.supported = true; trace.effect_key = Some("madness".to_string()); trace.summary = format!("{} X1 (scaling TODO)", spec.name); }
        "j_loyalty_card" => { trace.supported = true; trace.effect_key = Some("loyalty_card".to_string()); trace.summary = format!("{} (hand cycle tracking TODO)", spec.name); }
        "j_obelisk" => { trace.supported = true; trace.effect_key = Some("obelisk".to_string()); let xm = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(1.0); if xm > 1.0 { *xmult *= xm; trace.matched = true; } trace.summary = format!("{} X{} (scaling TODO)", spec.name, xm); }
        "j_vampire" => { trace.supported = true; trace.effect_key = Some("vampire".to_string()); let xm = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(1.0); if xm > 1.0 { *xmult *= xm; trace.matched = true; } trace.summary = format!("{} X{} (scaling TODO)", spec.name, xm); }
        "j_lucky_cat" => { trace.supported = true; trace.effect_key = Some("lucky_cat".to_string()); let xm = spec.config.get("Xmult").and_then(|v| v.as_f64()).unwrap_or(1.0); if xm > 1.0 { *xmult *= xm; trace.matched = true; } trace.summary = format!("{} X{} (scaling TODO)", spec.name, xm); }
        "j_ceremonial" => { trace.supported = true; trace.effect_key = Some("ceremonial".to_string()); let bonus = spec.config.get("mult").and_then(|v| v.as_i64()).unwrap_or(0) as i32; if bonus > 0 { *mult += bonus; trace.matched = true; events.push(event(EventStage::JokerPostScore, "joker_ceremonial", format!("{} added {} mult", spec.name, bonus))); } trace.summary = format!("{} +{} mult (growth TODO)", spec.name, bonus); }
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
        "j_runner" => { trace.supported = true; trace.effect_key = Some("runner".to_string()); if let Some(extra) = config_extra_obj(spec) { let current = extra.get("chips").and_then(|v| v.as_i64()).unwrap_or(0) as i32; if current > 0 { *chips += current; trace.matched = true; } } trace.summary = format!("{} (scaling chips TODO)", spec.name); }
        "j_square" => { trace.supported = true; trace.effect_key = Some("square_joker".to_string()); if let Some(extra) = config_extra_obj(spec) { let current = extra.get("chips").and_then(|v| v.as_i64()).unwrap_or(0) as i32; if current > 0 { *chips += current; trace.matched = true; } } trace.summary = format!("{} (scaling chips TODO)", spec.name); }
        "j_trousers" => { trace.supported = true; trace.effect_key = Some("spare_trousers".to_string()); trace.summary = format!("{} (scaling mult TODO)", spec.name); }
        "j_castle" => { trace.supported = true; trace.effect_key = Some("castle".to_string()); if let Some(extra) = config_extra_obj(spec) { let current = extra.get("chips").and_then(|v| v.as_i64()).unwrap_or(0) as i32; if current > 0 { *chips += current; trace.matched = true; } } trace.summary = format!("{} (scaling chips TODO)", spec.name); }
        "j_hit_the_road" => { trace.supported = true; trace.effect_key = Some("hit_the_road".to_string()); trace.summary = format!("{} X1 (Jack discard tracking TODO)", spec.name); }
        "j_idol" => { trace.supported = true; trace.effect_key = Some("idol".to_string()); trace.summary = format!("{} (rank+suit tracking TODO)", spec.name); }
        "j_caino" => { trace.supported = true; trace.effect_key = Some("caino".to_string()); trace.summary = format!("{} (face card destruction tracking TODO)", spec.name); }
        "j_yorick" => { trace.supported = true; trace.effect_key = Some("yorick".to_string()); if let Some(extra) = config_extra_obj(spec) { let xm = extra.get("xmult").and_then(|v| v.as_f64()).unwrap_or(1.0); if xm > 1.0 { *xmult *= xm; trace.matched = true; } } trace.summary = format!("{} (discard tracking TODO)", spec.name); }
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
        28..=46 => format!("move_left_{}", index - 28),
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
        BlindProgress, BlindSlot, CardInstance, ConsumableInstance, Engine, EngineError,
        EventStage, JokerInstance, JokerResolutionTrace, Phase, Rank, RunConfig,
        ScoringContext, Suit, TransitionTrace, CONSUMABLE_SLOT_LIMIT, JOKER_LIMIT,
    };
    use balatro_spec::{JokerSpec, RulesetBundle};
    use std::collections::BTreeMap;
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
        JokerInstance { joker_id: id.to_string(), name: name.to_string(), base_cost: 5, cost: 5, buy_cost: 5, sell_value: 2, extra_sell_value: 0, rarity: 1, edition: None, slot_index: 0, activation_class: "joker_independent".to_string(), wiki_effect_text_en: String::new(), remaining_uses: None }
    }

    fn make_joker_for_retrigger(id: &str, name: &str, slot: usize) -> JokerInstance {
        JokerInstance { joker_id: id.to_string(), name: name.to_string(), base_cost: 0, cost: 0, buy_cost: 0, sell_value: 0, extra_sell_value: 0, rarity: 1, edition: None, slot_index: slot, activation_class: String::new(), wiki_effect_text_en: String::new(), remaining_uses: None }
    }

    fn make_joker_spec(id: &str, name: &str) -> JokerSpec {
        JokerSpec { id: id.to_string(), order: 0, name: name.to_string(), set: "Joker".to_string(), base_cost: 0, cost: 0, rarity: 1, effect: None, config: std::collections::BTreeMap::new(), wiki_effect_text_en: String::new(), activation_class: String::new(), source_refs: std::collections::BTreeMap::new(), unlocked: true, blueprint_compat: true, perishable_compat: true, eternal_compat: true, sprite: None }
    }

    fn make_joker_from_bundle(bundle: &RulesetBundle, joker_id: &str, slot_index: usize) -> JokerInstance {
        let spec = bundle.joker_by_id(joker_id).expect("joker spec");
        JokerInstance { joker_id: spec.id.clone(), name: spec.name.clone(), base_cost: spec.base_cost, cost: spec.cost, buy_cost: spec.cost, sell_value: (spec.cost / 2).max(1), extra_sell_value: 0, rarity: spec.rarity, edition: None, slot_index, activation_class: spec.activation_class.clone(), wiki_effect_text_en: spec.wiki_effect_text_en.clone(), remaining_uses: None }
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
        apply_joker_effect(&spec, &ctx, &mut chips, &mut mult, &mut xmult, &mut money, &mut events, &mut trace);
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
}

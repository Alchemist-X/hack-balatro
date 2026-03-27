use balatro_spec::{BlindSpec, JokerSpec, RulesetBundle};
use rand::prelude::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const ACTION_DIM: usize = 86;
pub const HAND_LIMIT: usize = 8;
pub const JOKER_LIMIT: usize = 5;
pub const SHOP_LIMIT: usize = 10;

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
    pub slot_index: usize,
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
    won: bool,
    over: bool,
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
                won: false,
                over: false,
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
                for slot in 0..self.state.jokers.len().min(JOKER_LIMIT) {
                    mask[80 + slot] = 1;
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
            self.enter_current_blind(trace);
            return vec![event(
                EventStage::BlindPrePlay,
                "blind_selected",
                format!("Selected {}", self.state.blind_name),
            )];
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
        Vec::new()
    }

    fn handle_post_blind(&mut self, _action_index: usize, trace: &mut TransitionTrace) -> Vec<Event> {
        self.state.money += self.state.reward;
        let reward = self.state.reward;
        let cleared_boss = self.state.current_blind_slot == BlindSlot::Boss;
        if !cleared_boss {
            self.advance_to_next_blind_slot();
            self.set_active_preblind_progress();
        }
        self.state.phase = Phase::Shop;
        self.shuffle_deck("deck.shuffle.cashout", trace);
        self.refresh_shop(trace, "cashout_shop_refresh");

        let mut events = vec![
            event(
                EventStage::CashOut,
                "cashout",
                format!("Collected ${reward} and entered Shop"),
            ),
        ];
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
        if action_index == 79 {
            self.state.money -= 1;
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
                self.state.money += sold.cost.max(1) / 2;
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

        let mut chips = hand_spec.base_chips + played.iter().map(CardInstance::chip_value).sum::<i32>();
        let mut mult = hand_spec.base_mult;
        let mut events = vec![
            event(
                EventStage::OnPlayed,
                "hand_played",
                format!("Played {}", hand_spec.name),
            ),
            event(
                EventStage::CardScored,
                "base_score",
                format!("Base {} chips x{} mult", chips, mult),
            ),
        ];

        trace.add_transient("HAND_PLAYED");
        trace.retrigger_supported = false;
        if !self.state.jokers.is_empty() {
            trace.add_note("joker_retrigger_not_implemented");
        }
        for joker in &self.state.jokers {
            let mut joker_trace = JokerResolutionTrace {
                order: trace.joker_resolution.len(),
                joker_id: joker.joker_id.clone(),
                joker_name: joker.name.clone(),
                slot_index: joker.slot_index,
                stage: "joker_main".to_string(),
                supported: false,
                matched: false,
                retrigger_count: 0,
                effect_key: None,
                summary: "native engine has no implementation for this Joker".to_string(),
            };
            if let Some(spec) = self.ruleset.joker_by_id(&joker.joker_id) {
                apply_joker_effect(
                    spec,
                    &hand.key,
                    &played,
                    self.state.discards,
                    &self.state.jokers,
                    &mut chips,
                    &mut mult,
                    &mut events,
                    &mut joker_trace,
                );
            } else {
                joker_trace.summary = "ruleset bundle missing Joker spec".to_string();
            }
            if !joker_trace.supported {
                trace.add_note(format!("joker_not_implemented: {}", joker_trace.joker_name));
            }
            trace.joker_resolution.push(joker_trace);
        }

        let gained = chips * mult;
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

    fn enter_current_blind(&mut self, trace: &mut TransitionTrace) {
        self.state.phase = Phase::Blind;
        self.state.score = 0;
        self.state.plays = 4;
        self.state.discards = 3;
        self.state.selected_slots.clear();
        self.state.discarded.clear();
        self.state.shop.clear();
        self.state.shop_consumables.clear();
        self.sync_current_blind_descriptor();
        self.mark_current_blind_progress(BlindProgress::Current);
        self.reset_deck(trace);
        self.draw_to_hand(HAND_LIMIT);
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
                    },
                });
            }
        }
        if !self.ruleset.consumables.is_empty() {
            trace.add_note("shop_consumables_not_implemented");
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

fn apply_joker_effect(
    spec: &JokerSpec,
    hand_key: &str,
    played: &[CardInstance],
    discards_left: i32,
    jokers: &[JokerInstance],
    chips: &mut i32,
    mult: &mut i32,
    events: &mut Vec<Event>,
    trace: &mut JokerResolutionTrace,
) {
    let effect = spec.effect.as_deref().unwrap_or_default();
    if effect == "Mult" {
        trace.supported = true;
        trace.effect_key = Some("mult".to_string());
        if let Some(flat) = spec.config.get("mult").and_then(|value| value.as_i64()) {
            *mult += flat as i32;
            trace.matched = true;
            trace.summary = format!("{} added {} mult", spec.name, flat);
            events.push(event(
                EventStage::JokerPostScore,
                "joker_mult",
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
            let matches = played
                .iter()
                .filter(|card| suit_label(&card.suit) == suit_name)
                .count() as i32;
            if matches > 0 {
                *mult += matches * suit_bonus;
                trace.matched = true;
                trace.summary = format!("{} matched {} suited card(s)", spec.name, matches);
                events.push(event(
                    EventStage::JokerPostScore,
                    "joker_suit_mult",
                    format!("{} added {} mult", spec.name, matches * suit_bonus),
                ));
            } else {
                trace.summary = format!("{} had no matching {} card", spec.name, suit_name);
            }
        } else {
            trace.summary = format!("{} declared Suit Mult but had no extra config", spec.name);
        }
        return;
    }

    if let Some(hand_type) = spec.config.get("type").and_then(|value| value.as_str()) {
        trace.supported = true;
        trace.effect_key = Some("hand_type".to_string());
        if hand_type_to_key(hand_type) == hand_key {
            if let Some(flat) = spec.config.get("t_mult").and_then(|value| value.as_i64()) {
                *mult += flat as i32;
                trace.matched = true;
                trace.summary = format!("{} matched hand type {}", spec.name, hand_type);
                events.push(event(
                    EventStage::JokerPostScore,
                    "joker_type_mult",
                    format!("{} added {} mult", spec.name, flat),
                ));
            }
            if let Some(flat) = spec.config.get("t_chips").and_then(|value| value.as_i64()) {
                *chips += flat as i32;
                trace.matched = true;
                trace.summary = format!("{} matched hand type {}", spec.name, hand_type);
                events.push(event(
                    EventStage::JokerPostScore,
                    "joker_type_chips",
                    format!("{} added {} chips", spec.name, flat),
                ));
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
            let gained = extra as i32 * discards_left;
            *chips += gained;
            trace.matched = gained > 0;
            trace.summary = format!("{} scaled with {} discards left", spec.name, discards_left);
            events.push(event(
                EventStage::JokerPostScore,
                "joker_discard_chips",
                format!("{} added {} chips", spec.name, gained),
            ));
        } else {
            trace.summary = format!("{} declared Discard Chips but had no extra payload", spec.name);
        }
        return;
    }

    if spec.name == "Abstract Joker" {
        let gained = jokers.len() as i32 * 3;
        trace.supported = true;
        trace.matched = true;
        trace.effect_key = Some("abstract_joker".to_string());
        trace.summary = format!("{} counted {} Joker(s)", spec.name, jokers.len());
        *mult += gained;
        events.push(event(
            EventStage::JokerPostScore,
            "joker_abstract",
            format!("{} added {} mult", spec.name, gained),
        ));
        return;
    }

    if spec.name == "Scary Face" {
        let matches = played
            .iter()
            .filter(|card| matches!(card.rank, Rank::Jack | Rank::Queen | Rank::King))
            .count() as i32;
        trace.supported = true;
        trace.effect_key = Some("scary_face".to_string());
        if matches > 0 {
            let gained = matches * 30;
            *chips += gained;
            trace.matched = true;
            trace.summary = format!("{} matched {} face card(s)", spec.name, matches);
            events.push(event(
                EventStage::JokerPostScore,
                "joker_scary_face",
                format!("{} added {} chips", spec.name, gained),
            ));
        } else {
            trace.summary = format!("{} had no face-card targets", spec.name);
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
        24..=46 => format!("move_left_{}", index - 24),
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
    use super::{action_name, BlindProgress, BlindSlot, Engine, EngineError, Phase, RunConfig, TransitionTrace};
    use balatro_spec::RulesetBundle;
    use std::path::PathBuf;

    fn fixture_bundle() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/ruleset/balatro-1.0.1o-full.json")
    }

    #[test]
    fn engine_is_deterministic_for_same_seed_and_actions() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut left = Engine::new(7, bundle.clone(), RunConfig::default());
        let mut right = Engine::new(7, bundle, RunConfig::default());
        for _ in 0..8 {
            let action = left
                .legal_actions()
                .into_iter()
                .find(|action| action.enabled)
                .expect("legal action")
                .index;
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
        assert_eq!(snapshot.lua_state, "BLIND_SELECT");
        assert_eq!(snapshot.blind_name, "Small Blind");
        assert_eq!(snapshot.blind_states.get("Small").map(String::as_str), Some("Select"));
        assert_eq!(snapshot.blind_states.get("Big").map(String::as_str), Some("Upcoming"));
        assert_eq!(snapshot.blind_states.get("Boss").map(String::as_str), Some("Upcoming"));
        assert!(snapshot.shop_jokers.is_empty());

        let legal = engine.legal_actions();
        assert!(legal.iter().any(|action| action.name == "select_blind_0" && action.enabled));
        assert!(legal.iter().any(|action| action.name == "skip_blind" && action.enabled));
        assert!(!legal.iter().any(|action| action.name == "select_blind_1" && action.enabled));
        assert!(!legal.iter().any(|action| action.name == "select_blind_2" && action.enabled));
    }

    #[test]
    fn skip_blind_advances_small_to_big_to_boss() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(11, bundle, RunConfig::default());

        engine.step(85).expect("skip small");
        let after_small_skip = engine.snapshot();
        assert_eq!(after_small_skip.stage, "Stage_PreBlind");
        assert_eq!(after_small_skip.blind_name, "Big Blind");
        assert_eq!(after_small_skip.blind_states.get("Small").map(String::as_str), Some("Skipped"));
        assert_eq!(after_small_skip.blind_states.get("Big").map(String::as_str), Some("Select"));

        engine.step(85).expect("skip big");
        let after_big_skip = engine.snapshot();
        assert_eq!(after_big_skip.stage, "Stage_PreBlind");
        assert_ne!(after_big_skip.blind_name, "Small Blind");
        assert_ne!(after_big_skip.blind_name, "Big Blind");
        assert_eq!(after_big_skip.blind_states.get("Big").map(String::as_str), Some("Skipped"));
        assert_eq!(after_big_skip.blind_states.get("Boss").map(String::as_str), Some("Select"));

        let legal = engine.legal_actions();
        assert!(legal.iter().any(|action| action.name == "select_blind_2" && action.enabled));
        assert!(!legal.iter().any(|action| action.name == "skip_blind" && action.enabled));
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

        engine.prepare_preblind_state();
        assert!(engine.snapshot().shop_jokers.is_empty());
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
        assert_eq!(engine.state.phase, Phase::Shop);
        assert_eq!(engine.state.big_progress, BlindProgress::Select);

        engine.state.current_blind_slot = BlindSlot::Boss;
        engine.state.small_progress = BlindProgress::Defeated;
        engine.state.big_progress = BlindProgress::Defeated;
        engine.state.boss_progress = BlindProgress::Defeated;
        engine.state.phase = Phase::Shop;
        engine.advance_round(&mut trace);
        assert_eq!(engine.state.ante, 2);
        assert_eq!(engine.state.current_blind_slot, BlindSlot::Small);
        assert_eq!(engine.state.small_progress, BlindProgress::Select);
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
        assert_eq!(transition.snapshot_after.lua_state, "SHOP");
        assert_eq!(transition.snapshot_after.blind_name, "Big Blind");
        assert_eq!(transition.snapshot_after.money, 7);
        assert!(!transition.snapshot_after.shop_jokers.is_empty());

        let next = engine.step(70).expect("leave shop for big blind");
        assert_eq!(next.snapshot_after.stage, "Stage_PreBlind");
        assert_eq!(next.snapshot_after.lua_state, "BLIND_SELECT");
        assert_eq!(next.snapshot_after.blind_name, "Big Blind");
        assert_eq!(next.snapshot_after.ante, 1);
    }

    #[test]
    fn select_blind_transition_emits_transient_states_and_shuffle_trace() {
        let bundle = RulesetBundle::load_from_path(fixture_bundle()).expect("bundle");
        let mut engine = Engine::new(29, bundle, RunConfig::default());

        let transition = engine.step(10).expect("select small blind");
        assert_eq!(transition.snapshot_after.stage, "Stage_Blind");
        assert!(transition.trace.transient_lua_states.contains(&"NEW_ROUND".to_string()));
        assert!(transition.trace.transient_lua_states.contains(&"DRAW_TO_HAND".to_string()));
        assert!(transition
            .trace
            .rng_calls
            .iter()
            .any(|entry| entry.domain == "deck.shuffle.enter_blind"));
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

        let transition = engine.step(13).expect("cashout after cleared blind");
        assert!(transition
            .trace
            .rng_calls
            .iter()
            .any(|entry| entry.domain == "deck.shuffle.cashout"));
        assert!(transition
            .trace
            .rng_calls
            .iter()
            .any(|entry| entry.domain.starts_with("cashout_shop_refresh.slot_0")));
        assert!(transition
            .trace
            .notes
            .contains(&"shop_consumables_not_implemented".to_string()));
    }
}

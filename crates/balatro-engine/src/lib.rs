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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlindKind {
    Small,
    Big,
    Boss(String),
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
    pub cost: i32,
    pub rarity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopSlot {
    pub slot: usize,
    pub joker: JokerInstance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub phase: Phase,
    pub stage: String,
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
    pub reward: i32,
    pub deck: Vec<CardInstance>,
    pub available: Vec<CardInstance>,
    pub selected: Vec<CardInstance>,
    pub discarded: Vec<CardInstance>,
    pub jokers: Vec<JokerInstance>,
    pub shop_jokers: Vec<JokerInstance>,
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
    pub payload: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transition {
    pub snapshot_before: Snapshot,
    pub action: ActionDescriptor,
    pub events: Vec<Event>,
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
    blind: BlindKind,
    blind_name: String,
    boss_effect: String,
    score: i32,
    plays: i32,
    discards: i32,
    money: i32,
    reward: i32,
    deck: Vec<CardInstance>,
    available: Vec<CardInstance>,
    selected_slots: BTreeSet<usize>,
    discarded: Vec<CardInstance>,
    jokers: Vec<JokerInstance>,
    shop: Vec<ShopSlot>,
    won: bool,
    over: bool,
}

impl Engine {
    pub fn new(seed: u64, ruleset: RulesetBundle, config: RunConfig) -> Self {
        let mut engine = Self {
            ruleset,
            rng: ChaCha8Rng::seed_from_u64(seed),
            state: EngineState {
                phase: Phase::PreBlind,
                round: 1,
                ante: config.ante_start,
                stake: config.stake,
                blind: BlindKind::Small,
                blind_name: "Small Blind".to_string(),
                boss_effect: "None".to_string(),
                score: 0,
                plays: 4,
                discards: 3,
                money: 4,
                reward: 3,
                deck: Vec::new(),
                available: Vec::new(),
                selected_slots: BTreeSet::new(),
                discarded: Vec::new(),
                jokers: Vec::new(),
                shop: Vec::new(),
                won: false,
                over: false,
            },
        };
        engine.reset_deck();
        engine.draw_to_hand(HAND_LIMIT);
        engine.refresh_shop();
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
            reward: self.state.reward,
            deck: self.state.deck.clone(),
            available: self.state.available.clone(),
            selected,
            discarded: self.state.discarded.clone(),
            jokers: self.state.jokers.clone(),
            shop_jokers: self.state.shop.iter().map(|slot| slot.joker.clone()).collect(),
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
                mask[10] = 1;
                mask[11] = 1;
                mask[12] = 1;
                mask[85] = 1;
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
                if self.state.money > 0 {
                    mask[79] = 1;
                }
                if self.state.jokers.len() < JOKER_LIMIT {
                    for slot in self.state.shop.iter().take(SHOP_LIMIT) {
                        let index = 14 + slot.slot;
                        if index < 24 && slot.joker.cost <= self.state.money {
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

        let events = self.apply_action(action_index);
        let after = self.snapshot();
        Ok(Transition {
            snapshot_before: before,
            action,
            events,
            terminal: after.over,
            snapshot_after: after,
        })
    }

    fn apply_action(&mut self, action_index: usize) -> Vec<Event> {
        match self.state.phase {
            Phase::PreBlind => self.handle_preblind(action_index),
            Phase::Blind => self.handle_blind(action_index),
            Phase::PostBlind => self.handle_post_blind(action_index),
            Phase::Shop => self.handle_shop(action_index),
            Phase::CashOut => self.handle_cashout(action_index),
            Phase::End => Vec::new(),
        }
    }

    fn handle_preblind(&mut self, action_index: usize) -> Vec<Event> {
        let mut events = vec![];
        self.state.selected_slots.clear();
        self.state.score = 0;
        self.state.plays = 4;
        self.state.discards = 3;
        self.state.discarded.clear();
        self.reset_deck();
        self.draw_to_hand(HAND_LIMIT);

        if action_index == 10 {
            self.state.blind = BlindKind::Small;
            self.state.blind_name = "Small Blind".to_string();
            self.state.boss_effect = "None".to_string();
        } else if action_index == 11 {
            self.state.blind = BlindKind::Big;
            self.state.blind_name = "Big Blind".to_string();
            self.state.boss_effect = "None".to_string();
        } else {
            let boss = self.pick_boss_blind();
            self.state.boss_effect = boss.name.clone();
            self.state.blind_name = boss.name.clone();
            self.state.blind = BlindKind::Boss(boss.id.clone());
        }

        self.state.phase = Phase::Blind;
        events.push(event(
            EventStage::BlindPrePlay,
            "blind_selected",
            format!("Selected {}", self.state.blind_name),
        ));
        events
    }

    fn handle_blind(&mut self, action_index: usize) -> Vec<Event> {
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
            return self.discard_selected();
        }
        if action_index == 8 {
            return self.play_selected();
        }
        Vec::new()
    }

    fn handle_post_blind(&mut self, _action_index: usize) -> Vec<Event> {
        self.state.money += self.state.reward;
        self.state.phase = Phase::Shop;
        self.refresh_shop();
        vec![event(
            EventStage::CashOut,
            "cashout",
            format!("Collected ${}", self.state.reward),
        )]
    }

    fn handle_shop(&mut self, action_index: usize) -> Vec<Event> {
        if (14..24).contains(&action_index) {
            let slot = action_index - 14;
            if let Some(shop_slot) = self.state.shop.iter().find(|entry| entry.slot == slot).cloned() {
                if self.state.jokers.len() < JOKER_LIMIT && self.state.money >= shop_slot.joker.cost {
                    self.state.money -= shop_slot.joker.cost;
                    self.state.jokers.push(shop_slot.joker.clone());
                    return vec![event(
                        EventStage::Shop,
                        "buy_joker",
                        format!("Bought {}", shop_slot.joker.name),
                    )];
                }
            }
            return vec![];
        }
        if action_index == 79 {
            self.state.money -= 1;
            self.refresh_shop();
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
                self.state.money += sold.cost.max(1) / 2;
                return vec![event(
                    EventStage::Shop,
                    "sell_joker",
                    format!("Sold {}", sold.name),
                )];
            }
            return vec![];
        }
        self.advance_round();
        vec![event(
            EventStage::EndOfRound,
            "next_round",
            format!("Advanced to ante {}", self.state.ante),
        )]
    }

    fn handle_cashout(&mut self, _action_index: usize) -> Vec<Event> {
        self.advance_round();
        vec![event(
            EventStage::EndOfRound,
            "next_round",
            format!("Advanced to ante {}", self.state.ante),
        )]
    }

    fn discard_selected(&mut self) -> Vec<Event> {
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
        self.draw_to_hand(HAND_LIMIT);
        self.maybe_fail_blind();
        vec![event(
            EventStage::EndOfHand,
            "discard",
            format!("Discarded {} card(s)", removed.len()),
        )]
    }

    fn play_selected(&mut self) -> Vec<Event> {
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

        for joker in &self.state.jokers {
            if let Some(spec) = self.ruleset.joker_by_id(&joker.joker_id) {
                apply_joker_effect(spec, &hand.key, &played, self.state.discards, &self.state.jokers, &mut chips, &mut mult, &mut events);
            }
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
        self.draw_to_hand(HAND_LIMIT);
        events.push(event(
            EventStage::JokerPostScore,
            "score_total",
            format!("Scored {} points", gained),
        ));
        if self.state.score >= self.required_score() {
            self.state.phase = Phase::PostBlind;
            self.state.reward = blind_reward(&self.state.blind);
            events.push(event(
                EventStage::EndOfHand,
                "blind_cleared",
                format!("Cleared {}", self.state.blind_name),
            ));
        } else {
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

    fn advance_round(&mut self) {
        self.state.round += 1;
        if self.state.blind_name == "Boss Blind" || matches!(self.state.blind, BlindKind::Boss(_)) {
            self.state.ante += 1;
        }
        if self.state.ante > 8 {
            self.state.phase = Phase::End;
            self.state.over = true;
            self.state.won = true;
            return;
        }
        self.state.phase = Phase::PreBlind;
        self.state.score = 0;
        self.state.plays = 4;
        self.state.discards = 3;
        self.state.boss_effect = "None".to_string();
        self.state.blind = BlindKind::Small;
        self.state.blind_name = "Small Blind".to_string();
        self.state.reward = 3;
        self.state.discarded.clear();
        self.state.selected_slots.clear();
        self.reset_deck();
        self.draw_to_hand(HAND_LIMIT);
        self.refresh_shop();
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

    fn reset_deck(&mut self) {
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
        self.state.deck.shuffle(&mut self.rng);
    }

    fn draw_to_hand(&mut self, target: usize) {
        while self.state.available.len() < target && !self.state.deck.is_empty() {
            let card = self.state.deck.pop().expect("card draw");
            self.state.available.push(card);
        }
    }

    fn pick_boss_blind(&mut self) -> BlindSpec {
        let mut pool: Vec<&BlindSpec> = self
            .ruleset
            .blinds
            .iter()
            .filter(|blind| blind.boss && blind.showdown == (self.state.ante >= 8))
            .filter(|blind| blind.min_ante.map(|min| self.state.ante >= min).unwrap_or(true))
            .filter(|blind| blind.max_ante.map(|max| self.state.ante <= max).unwrap_or(true))
            .collect();
        if pool.is_empty() {
            pool = self.ruleset.blinds.iter().filter(|blind| blind.boss).collect();
        }
        (*pool.choose(&mut self.rng).expect("boss blind")).clone()
    }

    fn refresh_shop(&mut self) {
        self.state.shop.clear();
        let mut common = Vec::new();
        let mut uncommon = Vec::new();
        let mut rare = Vec::new();
        let mut legendary = Vec::new();
        for joker in &self.ruleset.jokers {
            if !joker.unlocked {
                continue;
            }
            match joker.rarity {
                1 => common.push(joker),
                2 => uncommon.push(joker),
                3 => rare.push(joker),
                _ => legendary.push(joker),
            }
        }
        for slot in 0..2 {
            let rarity_roll = self.rng.gen_range(0.0..100.0);
            let pool = if rarity_roll < self.ruleset.shop_weights.common {
                &common
            } else if rarity_roll < self.ruleset.shop_weights.common + self.ruleset.shop_weights.uncommon {
                &uncommon
            } else if rarity_roll < self.ruleset.shop_weights.common + self.ruleset.shop_weights.uncommon + self.ruleset.shop_weights.rare {
                &rare
            } else {
                &legendary
            };
            if let Some(spec) = pool.choose(&mut self.rng) {
                self.state.shop.push(ShopSlot {
                    slot,
                    joker: JokerInstance {
                        joker_id: spec.id.clone(),
                        name: spec.name.clone(),
                        cost: spec.cost,
                        rarity: spec.rarity,
                    },
                });
            }
        }
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
) {
    let effect = spec.effect.as_deref().unwrap_or_default();
    if effect == "Mult" {
        if let Some(flat) = spec.config.get("mult").and_then(|value| value.as_i64()) {
            *mult += flat as i32;
            events.push(event(
                EventStage::JokerPostScore,
                "joker_mult",
                format!("{} added {} mult", spec.name, flat),
            ));
        }
        return;
    }

    if effect == "Suit Mult" {
        if let Some(extra) = spec.config.get("extra").and_then(|value| value.as_object()) {
            let suit_name = extra.get("suit").and_then(|value| value.as_str()).unwrap_or_default();
            let suit_bonus = extra.get("s_mult").and_then(|value| value.as_i64()).unwrap_or(0) as i32;
            let matches = played
                .iter()
                .filter(|card| suit_label(&card.suit) == suit_name)
                .count() as i32;
            if matches > 0 {
                *mult += matches * suit_bonus;
                events.push(event(
                    EventStage::JokerPostScore,
                    "joker_suit_mult",
                    format!("{} added {} mult", spec.name, matches * suit_bonus),
                ));
            }
        }
        return;
    }

    if let Some(hand_type) = spec.config.get("type").and_then(|value| value.as_str()) {
        if hand_type_to_key(hand_type) == hand_key {
            if let Some(flat) = spec.config.get("t_mult").and_then(|value| value.as_i64()) {
                *mult += flat as i32;
                events.push(event(
                    EventStage::JokerPostScore,
                    "joker_type_mult",
                    format!("{} added {} mult", spec.name, flat),
                ));
            }
            if let Some(flat) = spec.config.get("t_chips").and_then(|value| value.as_i64()) {
                *chips += flat as i32;
                events.push(event(
                    EventStage::JokerPostScore,
                    "joker_type_chips",
                    format!("{} added {} chips", spec.name, flat),
                ));
            }
        }
        return;
    }

    if effect == "Discard Chips" {
        if let Some(extra) = spec.config.get("extra").and_then(|value| value.as_i64()) {
            let gained = extra as i32 * discards_left;
            *chips += gained;
            events.push(event(
                EventStage::JokerPostScore,
                "joker_discard_chips",
                format!("{} added {} chips", spec.name, gained),
            ));
        }
        return;
    }

    if spec.name == "Abstract Joker" {
        let gained = jokers.len() as i32 * 3;
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
        if matches > 0 {
            let gained = matches * 30;
            *chips += gained;
            events.push(event(
                EventStage::JokerPostScore,
                "joker_scary_face",
                format!("{} added {} chips", spec.name, gained),
            ));
        }
    }
}

fn event(stage: EventStage, kind: impl Into<String>, summary: impl Into<String>) -> Event {
    Event {
        stage,
        kind: kind.into(),
        summary: summary.into(),
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
    use super::{action_name, Engine, RunConfig};
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
}

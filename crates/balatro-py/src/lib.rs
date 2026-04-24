use balatro_engine::{action_name, ActionDescriptor, BoosterPackInstance, CardInstance, ConsumableInstance, Engine, JokerInstance, RunConfig, Snapshot, TagInfo, Transition, VoucherInstance};
use balatro_spec::RulesetBundle;
use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;

#[pyclass(name = "Card")]
#[derive(Clone)]
struct PyCard {
    inner: CardInstance,
}

#[pymethods]
impl PyCard {
    #[getter]
    fn card_id(&self) -> u32 {
        self.inner.card_id
    }

    #[getter]
    fn rank_index(&self) -> usize {
        self.inner.rank_index()
    }

    #[getter]
    fn suit_index(&self) -> usize {
        self.inner.suit_index()
    }

    #[getter]
    fn chip_value(&self) -> i32 {
        self.inner.chip_value()
    }

    #[getter]
    fn seal(&self) -> &str {
        self.inner.typed_seal().as_str()
    }

    #[getter]
    fn is_face_card(&self) -> bool {
        self.inner.is_face_card()
    }

    #[getter]
    fn enhancement(&self) -> Option<&str> {
        self.inner.enhancement.as_deref()
    }

    #[getter]
    fn edition(&self) -> Option<&str> {
        self.inner.edition.as_deref()
    }
}

#[pyclass(name = "Joker")]
#[derive(Clone)]
struct PyJoker {
    inner: JokerInstance,
}

#[pymethods]
impl PyJoker {
    #[getter]
    fn joker_id(&self) -> &str {
        &self.inner.joker_id
    }

    #[getter]
    fn joker_name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn joker_cost(&self) -> i32 {
        self.inner.cost
    }

    #[getter]
    fn joker_rarity(&self) -> i32 {
        self.inner.rarity
    }

    #[getter]
    fn remaining_uses(&self) -> Option<u32> {
        self.inner.remaining_uses
    }

    #[getter]
    fn activation_class(&self) -> &str {
        &self.inner.activation_class
    }
}

#[pyclass(name = "Consumable")]
#[derive(Clone)]
struct PyConsumable {
    inner: ConsumableInstance,
}

#[pymethods]
impl PyConsumable {
    #[getter]
    fn consumable_id(&self) -> &str {
        &self.inner.consumable_id
    }

    #[getter]
    fn consumable_name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn set(&self) -> &str {
        &self.inner.set
    }

    #[getter]
    fn cost(&self) -> i32 {
        self.inner.cost
    }

    #[getter]
    fn buy_cost(&self) -> i32 {
        self.inner.buy_cost
    }

    #[getter]
    fn sell_value(&self) -> i32 {
        self.inner.sell_value
    }
}

#[pyclass(name = "Voucher")]
#[derive(Clone)]
struct PyVoucher {
    inner: VoucherInstance,
}

#[pymethods]
impl PyVoucher {
    #[getter]
    fn voucher_id(&self) -> &str {
        &self.inner.voucher_id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn cost(&self) -> i32 {
        self.inner.cost
    }

    #[getter]
    fn effect_key(&self) -> &str {
        &self.inner.effect_key
    }

    #[getter]
    fn description(&self) -> &str {
        &self.inner.description
    }
}

#[pyclass(name = "Tag")]
#[derive(Clone)]
struct PyTag {
    inner: TagInfo,
}

#[pymethods]
impl PyTag {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn description(&self) -> &str {
        &self.inner.description
    }
}

#[pyclass(name = "BoosterPack")]
#[derive(Clone)]
struct PyBoosterPack {
    inner: BoosterPackInstance,
}

#[pymethods]
impl PyBoosterPack {
    #[getter]
    fn pack_type(&self) -> &str {
        &self.inner.pack_type
    }

    #[getter]
    fn cost(&self) -> i32 {
        self.inner.cost
    }

    #[getter]
    fn picks_remaining(&self) -> u32 {
        self.inner.picks_remaining
    }

    #[getter]
    fn choice_names(&self) -> Vec<String> {
        self.inner.choices.iter().map(|c| c.name.clone()).collect()
    }
}

#[pyclass(name = "Snapshot")]
#[derive(Clone)]
struct PySnapshot {
    inner: Snapshot,
}

#[pymethods]
impl PySnapshot {
    #[getter]
    fn stage(&self) -> &str {
        &self.inner.stage
    }

    #[getter]
    fn round(&self) -> i32 {
        self.inner.round
    }

    #[getter]
    fn score(&self) -> i32 {
        self.inner.score
    }

    #[getter]
    fn required_score(&self) -> i32 {
        self.inner.required_score
    }

    #[getter]
    fn plays(&self) -> i32 {
        self.inner.plays
    }

    #[getter]
    fn discards(&self) -> i32 {
        self.inner.discards
    }

    #[getter]
    fn money(&self) -> i32 {
        self.inner.money
    }

    #[getter]
    fn ante(&self) -> i32 {
        self.inner.ante
    }

    #[getter]
    fn reward(&self) -> i32 {
        self.inner.reward
    }

    #[getter]
    fn boss_effect(&self) -> &str {
        &self.inner.boss_effect
    }

    #[getter]
    fn deck(&self) -> Vec<PyCard> {
        self.inner
            .deck
            .iter()
            .cloned()
            .map(|inner| PyCard { inner })
            .collect()
    }

    #[getter]
    fn available(&self) -> Vec<PyCard> {
        self.inner
            .available
            .iter()
            .cloned()
            .map(|inner| PyCard { inner })
            .collect()
    }

    #[getter]
    fn selected(&self) -> Vec<PyCard> {
        self.inner
            .selected
            .iter()
            .cloned()
            .map(|inner| PyCard { inner })
            .collect()
    }

    #[getter]
    fn discarded(&self) -> Vec<PyCard> {
        self.inner
            .discarded
            .iter()
            .cloned()
            .map(|inner| PyCard { inner })
            .collect()
    }

    #[getter]
    fn jokers(&self) -> Vec<PyJoker> {
        self.inner
            .jokers
            .iter()
            .cloned()
            .map(|inner| PyJoker { inner })
            .collect()
    }

    #[getter]
    fn shop_jokers(&self) -> Vec<PyJoker> {
        self.inner
            .shop_jokers
            .iter()
            .cloned()
            .map(|inner| PyJoker { inner })
            .collect()
    }

    #[getter]
    fn consumables(&self) -> Vec<PyConsumable> {
        self.inner
            .consumables
            .iter()
            .cloned()
            .map(|inner| PyConsumable { inner })
            .collect()
    }

    #[getter]
    fn shop_consumables(&self) -> Vec<PyConsumable> {
        self.inner
            .shop_consumables
            .iter()
            .cloned()
            .map(|inner| PyConsumable { inner })
            .collect()
    }

    #[getter]
    fn consumable_slot_limit(&self) -> usize {
        self.inner.consumable_slot_limit
    }

    #[getter]
    fn owned_vouchers(&self) -> Vec<String> {
        self.inner.owned_vouchers.clone()
    }

    #[getter]
    fn shop_voucher(&self) -> Option<PyVoucher> {
        self.inner
            .shop_voucher
            .as_ref()
            .map(|inner| PyVoucher {
                inner: inner.clone(),
            })
    }

    #[getter]
    fn shop_packs(&self) -> Vec<PyBoosterPack> {
        self.inner
            .shop_packs
            .iter()
            .cloned()
            .map(|inner| PyBoosterPack { inner })
            .collect()
    }

    #[getter]
    fn open_pack(&self) -> Option<PyBoosterPack> {
        self.inner
            .open_pack
            .as_ref()
            .map(|inner| PyBoosterPack {
                inner: inner.clone(),
            })
    }

    #[getter]
    fn seed_str(&self) -> &str {
        &self.inner.seed_str
    }

    #[getter]
    fn deck_name(&self) -> &str {
        &self.inner.deck_name
    }

    #[getter]
    fn stake_name(&self) -> &str {
        &self.inner.stake_name
    }

    #[getter]
    fn deck_limit(&self) -> i32 {
        self.inner.deck_limit
    }

    #[getter]
    fn play_card_limit(&self) -> i32 {
        self.inner.play_card_limit
    }

    #[getter]
    fn pack_limit(&self) -> Option<i32> {
        self.inner.pack_limit
    }

    #[getter]
    fn pack_highlighted_limit(&self) -> Option<i32> {
        self.inner.pack_highlighted_limit
    }

    #[getter]
    fn small_tag(&self) -> Option<PyTag> {
        self.inner
            .small_tag
            .as_ref()
            .map(|inner| PyTag { inner: inner.clone() })
    }

    #[getter]
    fn big_tag(&self) -> Option<PyTag> {
        self.inner
            .big_tag
            .as_ref()
            .map(|inner| PyTag { inner: inner.clone() })
    }

    #[getter]
    fn boss_tag(&self) -> Option<PyTag> {
        self.inner
            .boss_tag
            .as_ref()
            .map(|inner| PyTag { inner: inner.clone() })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|err| PyValueError::new_err(err.to_string()))
    }
}

#[pyclass(name = "ActionDescriptor")]
#[derive(Clone)]
struct PyActionDescriptor {
    inner: ActionDescriptor,
}

#[pymethods]
impl PyActionDescriptor {
    #[getter]
    fn index(&self) -> usize {
        self.inner.index
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn enabled(&self) -> bool {
        self.inner.enabled
    }
}

#[pyclass(name = "Transition")]
#[derive(Clone)]
struct PyTransition {
    inner: Transition,
}

#[pymethods]
impl PyTransition {
    #[getter]
    fn snapshot_before(&self) -> PySnapshot {
        PySnapshot {
            inner: self.inner.snapshot_before.clone(),
        }
    }

    #[getter]
    fn snapshot_after(&self) -> PySnapshot {
        PySnapshot {
            inner: self.inner.snapshot_after.clone(),
        }
    }

    #[getter]
    fn terminal(&self) -> bool {
        self.inner.terminal
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|err| PyValueError::new_err(err.to_string()))
    }
}

#[pyclass(name = "Engine")]
struct PyEngine {
    inner: Engine,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (seed=42, ruleset_path=None, stake=1, deck="red", seed_str=""))]
    fn new(
        seed: u64,
        ruleset_path: Option<String>,
        stake: i32,
        deck: &str,
        seed_str: &str,
    ) -> PyResult<Self> {
        let bundle_path = ruleset_path.unwrap_or_else(default_ruleset_path);
        let bundle = RulesetBundle::load_from_path(&bundle_path)
            .map_err(|err| PyFileNotFoundError::new_err(err.to_string()))?;
        Ok(Self {
            inner: Engine::new(
                seed,
                bundle,
                RunConfig {
                    stake,
                    deck_key: deck.to_lowercase(),
                    seed_str: seed_str.to_string(),
                    ..RunConfig::default()
                },
            ),
        })
    }

    #[getter]
    fn state(&self) -> PySnapshot {
        PySnapshot {
            inner: self.inner.snapshot(),
        }
    }

    #[getter]
    fn is_over(&self) -> bool {
        self.inner.snapshot().over
    }

    #[getter]
    fn is_win(&self) -> bool {
        self.inner.snapshot().won
    }

    fn snapshot(&self) -> PySnapshot {
        PySnapshot {
            inner: self.inner.snapshot(),
        }
    }

    fn gen_action_space(&self) -> Vec<u8> {
        self.inner.gen_action_space()
    }

    fn legal_actions(&self) -> Vec<PyActionDescriptor> {
        self.inner
            .legal_actions()
            .into_iter()
            .map(|inner| PyActionDescriptor { inner })
            .collect()
    }

    fn handle_action_index(&mut self, index: usize) -> PyResult<()> {
        self.inner
            .step(index)
            .map(|_| ())
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn step(&mut self, index: usize) -> PyResult<PyTransition> {
        self.inner
            .step(index)
            .map(|inner| PyTransition { inner })
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[pyo3(signature = (seed=None))]
    fn clone_seeded(&self, seed: Option<u64>) -> PyEngine {
        PyEngine {
            inner: self.inner.clone_seeded(seed),
        }
    }

    #[pyo3(signature = (profile = "legacy_86x454"))]
    fn encode_observation(&self, profile: &str) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let snapshot = self.inner.snapshot();
            match profile {
                "structured" => {
                    let raw = serde_json::to_string(&snapshot)
                        .map_err(|err| PyValueError::new_err(err.to_string()))?;
                    Ok(raw.into_pyobject(py)?.unbind().into())
                }
                "legacy_86x454" => {
                    // env.state_encoder was moved to env/legacy/ as of 2026-04-24.
                    // This observe profile is unreachable from the primary path;
                    // only legacy Gym code would trigger it. Kept working in case
                    // someone resurrects BalatroEnv from env/legacy/.
                    let module = py.import("numpy")?;
                    let state_encoder = py.import("env.legacy.state_encoder")?;
                    let mask = self.inner.gen_action_space();
                    let py_mask = module
                        .call_method1("array", (mask,))?
                        .call_method1("astype", ("float32",))?;
                    let state = PySnapshot { inner: snapshot };
                    let encoded = state_encoder.call_method1("encode_pylatro_state", (state, py_mask))?;
                    Ok(encoded.into())
                }
                other => Err(PyValueError::new_err(format!("unknown observation profile {other}"))),
            }
        })
    }
}

#[pyfunction]
fn default_ruleset_path() -> String {
    format!(
        "{}/fixtures/ruleset/balatro-1.0.1o-full.json",
        env!("CARGO_MANIFEST_DIR")
            .replace("/crates/balatro-py", "")
    )
}

#[pyfunction]
fn action_label(index: usize) -> String {
    action_name(index)
}

#[pymodule]
fn balatro_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCard>()?;
    m.add_class::<PyJoker>()?;
    m.add_class::<PyConsumable>()?;
    m.add_class::<PyVoucher>()?;
    m.add_class::<PyTag>()?;
    m.add_class::<PyBoosterPack>()?;
    m.add_class::<PySnapshot>()?;
    m.add_class::<PyActionDescriptor>()?;
    m.add_class::<PyTransition>()?;
    m.add_class::<PyEngine>()?;
    m.add_function(wrap_pyfunction!(default_ruleset_path, m)?)?;
    m.add_function(wrap_pyfunction!(action_label, m)?)?;
    m.add("ACTION_DIM", 86)?;
    Ok(())
}

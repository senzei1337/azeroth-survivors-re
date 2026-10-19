//! Rebindable keys. Every survivors key that a player might want elsewhere is a [`Bind`] with two
//! slots (WoW's primary and secondary binding), kept across launches in
//! `benilla-config/survivors-keybinds.toml` and edited on the Options frame's Key Bindings page
//! ([`super::settings`]). Esc, Enter and the mouse stay fixed.
//!
//! The table lives in a process-wide lock rather than a Bevy resource so the many small readers
//! (pure helpers like [`super::wowcam::Keys::read`] among them) can ask it without new system
//! parameters. [`generation`] moves on every change, for the labels that show a key.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, RwLock};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// One rebindable action.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub(super) enum Bind {
    Forward,
    Backward,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
    Jump,
    Mobility,
    Toy1,
    Toy2,
    Toy3,
    Character,
    Meter,
    ToggleView,
    Grade,
    LevelUpStrip,
    Pick1,
    Pick2,
    Pick3,
    Reroll,
    Banish,
    Skip,
}

impl Bind {
    /// Every bind, in the Key Bindings page's order.
    pub(super) const ALL: [Self; 22] = [
        Self::Forward,
        Self::Backward,
        Self::TurnLeft,
        Self::TurnRight,
        Self::StrafeLeft,
        Self::StrafeRight,
        Self::Jump,
        Self::Mobility,
        Self::Toy1,
        Self::Toy2,
        Self::Toy3,
        Self::Character,
        Self::Meter,
        Self::ToggleView,
        Self::Grade,
        Self::LevelUpStrip,
        Self::Pick1,
        Self::Pick2,
        Self::Pick3,
        Self::Reroll,
        Self::Banish,
        Self::Skip,
    ];

    /// The name on the Key Bindings page.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Forward => "Move Forward",
            Self::Backward => "Move Backward",
            Self::TurnLeft => "Turn / Move Left",
            Self::TurnRight => "Turn / Move Right",
            Self::StrafeLeft => "Strafe Left",
            Self::StrafeRight => "Strafe Right",
            Self::Jump => "Jump / Self-Res",
            Self::Mobility => "Movement Spell",
            Self::Toy1 => "Toy 1",
            Self::Toy2 => "Toy 2",
            Self::Toy3 => "Toy 3",
            Self::Character => "Character Panel",
            Self::Meter => "Damage Meter",
            Self::ToggleView => "Swap Camera View",
            Self::Grade => "Picture Polish",
            Self::LevelUpStrip => "Level-up Hand (co-op)",
            Self::Pick1 => "Pick Card 1",
            Self::Pick2 => "Pick Card 2",
            Self::Pick3 => "Pick Card 3",
            Self::Reroll => "Reroll",
            Self::Banish => "Banish",
            Self::Skip => "Skip",
        }
    }

    /// The key in the settings file.
    fn id(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::TurnLeft => "turn_left",
            Self::TurnRight => "turn_right",
            Self::StrafeLeft => "strafe_left",
            Self::StrafeRight => "strafe_right",
            Self::Jump => "jump",
            Self::Mobility => "mobility",
            Self::Toy1 => "toy_1",
            Self::Toy2 => "toy_2",
            Self::Toy3 => "toy_3",
            Self::Character => "character",
            Self::Meter => "meter",
            Self::ToggleView => "toggle_view",
            Self::Grade => "grade",
            Self::LevelUpStrip => "level_up_strip",
            Self::Pick1 => "pick_1",
            Self::Pick2 => "pick_2",
            Self::Pick3 => "pick_3",
            Self::Reroll => "reroll",
            Self::Banish => "banish",
            Self::Skip => "skip",
        }
    }

    /// The stock keys, as the game shipped them.
    pub(super) fn defaults(self) -> [Option<KeyCode>; 2] {
        use KeyCode as K;
        match self {
            Self::Forward => [Some(K::KeyW), Some(K::ArrowUp)],
            Self::Backward => [Some(K::KeyS), Some(K::ArrowDown)],
            Self::TurnLeft => [Some(K::KeyA), Some(K::ArrowLeft)],
            Self::TurnRight => [Some(K::KeyD), Some(K::ArrowRight)],
            Self::StrafeLeft => [Some(K::KeyQ), None],
            Self::StrafeRight => [Some(K::KeyE), None],
            Self::Jump => [Some(K::Space), None],
            Self::Mobility => [Some(K::KeyF), None],
            Self::Toy1 => [Some(K::Digit7), None],
            Self::Toy2 => [Some(K::Digit8), None],
            Self::Toy3 => [Some(K::Digit9), None],
            Self::Character => [Some(K::KeyC), None],
            Self::Meter => [Some(K::KeyM), None],
            Self::ToggleView => [Some(K::KeyV), None],
            Self::Grade => [Some(K::KeyG), None],
            Self::LevelUpStrip => [Some(K::Tab), None],
            Self::Pick1 => [Some(K::Digit1), None],
            Self::Pick2 => [Some(K::Digit2), None],
            Self::Pick3 => [Some(K::Digit3), None],
            Self::Reroll => [Some(K::KeyR), None],
            Self::Banish => [Some(K::KeyB), None],
            Self::Skip => [Some(K::KeyX), None],
        }
    }

    /// Binds that are read on different screens can share a key without stealing it from each
    /// other: the level-up's card and tool keys only work while a hand is up, where nothing else
    /// does.
    fn group(self) -> u8 {
        match self {
            Self::Pick1 | Self::Pick2 | Self::Pick3 | Self::Reroll | Self::Banish | Self::Skip => 1,
            _ => 0,
        }
    }
}

/// The whole table: two slots per bind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Table(BTreeMap<Bind, [Option<KeyCode>; 2]>);

impl Default for Table {
    fn default() -> Self {
        Self(Bind::ALL.iter().map(|&b| (b, b.defaults())).collect())
    }
}

impl Table {
    pub(super) fn get(&self, bind: Bind) -> [Option<KeyCode>; 2] {
        self.0.get(&bind).copied().unwrap_or([None, None])
    }

    /// Puts `key` in `bind`'s `slot`, taking it off any other bind of the same group (WoW's own
    /// rule: a key does one thing). The other slot of the same bind loses it too.
    pub(super) fn set(&mut self, bind: Bind, slot: usize, key: Option<KeyCode>) {
        if let Some(key) = key {
            for (other, slots) in self.0.iter_mut() {
                if other.group() != bind.group() {
                    continue;
                }
                for s in slots.iter_mut() {
                    if *s == Some(key) {
                        *s = None;
                    }
                }
            }
        }
        let slots = self.0.entry(bind).or_insert([None, None]);
        slots[slot.min(1)] = key;
    }
}

static TABLE: RwLock<Option<Table>> = RwLock::new(None);
static GENERATION: AtomicU32 = AtomicU32::new(0);
/// The Key Bindings page is waiting for a key: no bind fires meanwhile.
static CAPTURING: AtomicBool = AtomicBool::new(false);
/// The key that just finished a capture; it fires nothing until it is let go.
static SWALLOW: Mutex<Option<KeyCode>> = Mutex::new(None);

/// A copy of the current table.
pub(super) fn table() -> Table {
    TABLE
        .read()
        .ok()
        .and_then(|t| (*t).clone())
        .unwrap_or_default()
}

fn store(table: Table) {
    if let Ok(mut t) = TABLE.write() {
        *t = Some(table);
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Moves on every change to the table, for anything that shows a key.
pub(super) fn generation() -> u32 {
    GENERATION.load(Ordering::Relaxed)
}

/// `bind`'s keys right now.
pub(super) fn keys_of(bind: Bind) -> [Option<KeyCode>; 2] {
    TABLE
        .read()
        .ok()
        .and_then(|t| (*t).as_ref().map(|t| t.get(bind)))
        .unwrap_or_else(|| bind.defaults())
}

fn swallowed(keys: &ButtonInput<KeyCode>, key: KeyCode) -> bool {
    let Ok(mut s) = SWALLOW.lock() else {
        return false;
    };
    match *s {
        Some(k) if k == key => {
            if keys.pressed(k) {
                true
            } else {
                *s = None;
                false
            }
        }
        _ => false,
    }
}

/// Whether either of `bind`'s keys is held.
pub(super) fn pressed(keys: &ButtonInput<KeyCode>, bind: Bind) -> bool {
    if CAPTURING.load(Ordering::Relaxed) {
        return false;
    }
    keys_of(bind)
        .into_iter()
        .flatten()
        .any(|k| keys.pressed(k) && !swallowed(keys, k))
}

/// Whether either of `bind`'s keys went down this frame.
pub(super) fn just_pressed(keys: &ButtonInput<KeyCode>, bind: Bind) -> bool {
    if CAPTURING.load(Ordering::Relaxed) {
        return false;
    }
    keys_of(bind)
        .into_iter()
        .flatten()
        .any(|k| keys.just_pressed(k) && !swallowed(keys, k))
}

/// The short name of `bind`'s first key, for the HUD ("F", "Tab"); empty when unbound.
pub(super) fn label(bind: Bind) -> String {
    keys_of(bind)
        .into_iter()
        .flatten()
        .next()
        .map(key_name)
        .unwrap_or_default()
}

/// The page starts or stops waiting for a key.
pub(super) fn set_capturing(on: bool) {
    CAPTURING.store(on, Ordering::Relaxed);
}

/// The page took `key`: it fires nothing until released.
pub(super) fn swallow(key: KeyCode) {
    if let Ok(mut s) = SWALLOW.lock() {
        *s = Some(key);
    }
}

/// Binds `key` (or clears the slot) and saves.
pub(super) fn assign(bind: Bind, slot: usize, key: Option<KeyCode>) {
    let mut t = table();
    t.set(bind, slot, key);
    save(&t);
    store(t);
}

/// Back to the stock keys, and saved.
pub(super) fn reset() {
    let t = Table::default();
    save(&t);
    store(t);
}

// ── The file ────────────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct File {
    binds: BTreeMap<String, Vec<String>>,
}

fn path() -> Option<PathBuf> {
    crate::local_state::home().map(|h| h.join("survivors-keybinds.toml"))
}

fn to_toml(table: &Table) -> String {
    let binds = Bind::ALL
        .iter()
        .map(|&b| {
            let keys = table
                .get(b)
                .iter()
                .map(|k| k.map(key_id).unwrap_or_default())
                .collect();
            (b.id().to_string(), keys)
        })
        .collect();
    format!(
        "# Warcraft Survivors key bindings (Options > Key Bindings edits these). Two keys per \
         action; \"\" is unbound.\n{}",
        toml::to_string(&File { binds }).unwrap_or_default()
    )
}

/// The table a file says; binds it does not name keep their stock keys.
fn from_toml(text: &str) -> Result<Table, toml::de::Error> {
    let file = toml::from_str::<File>(text)?;
    let mut table = Table::default();
    for &b in &Bind::ALL {
        let Some(keys) = file.binds.get(b.id()) else {
            continue;
        };
        let mut slots = [None, None];
        for (slot, name) in slots.iter_mut().zip(keys) {
            *slot = parse_key(name);
        }
        table.0.insert(b, slots);
    }
    Ok(table)
}

fn save(table: &Table) {
    let Some(path) = path() else { return };
    if let Err(e) = crate::local_state::write_atomic(&path, &to_toml(table)) {
        warn!("survivors: cannot save {}: {e}", path.display());
    }
}

/// Startup: the saved bindings, or the stock ones.
pub(super) fn load_keybinds() {
    let Some(path) = path() else {
        store(Table::default());
        return;
    };
    let table = match crate::local_state::read_to_string(&path) {
        Ok(text) => from_toml(&text).unwrap_or_else(|e| {
            warn!(
                "survivors: {} is malformed ({e}); the stock keys",
                path.display()
            );
            Table::default()
        }),
        Err(_) => Table::default(),
    };
    store(table);
}

// ── Key names ───────────────────────────────────────────────────────────────────────────────────

/// Every key that can be bound, with its file id and its display name.
const KEYS: &[(KeyCode, &str, &str)] = {
    use KeyCode as K;
    &[
        (K::KeyA, "KeyA", "A"),
        (K::KeyB, "KeyB", "B"),
        (K::KeyC, "KeyC", "C"),
        (K::KeyD, "KeyD", "D"),
        (K::KeyE, "KeyE", "E"),
        (K::KeyF, "KeyF", "F"),
        (K::KeyG, "KeyG", "G"),
        (K::KeyH, "KeyH", "H"),
        (K::KeyI, "KeyI", "I"),
        (K::KeyJ, "KeyJ", "J"),
        (K::KeyK, "KeyK", "K"),
        (K::KeyL, "KeyL", "L"),
        (K::KeyM, "KeyM", "M"),
        (K::KeyN, "KeyN", "N"),
        (K::KeyO, "KeyO", "O"),
        (K::KeyP, "KeyP", "P"),
        (K::KeyQ, "KeyQ", "Q"),
        (K::KeyR, "KeyR", "R"),
        (K::KeyS, "KeyS", "S"),
        (K::KeyT, "KeyT", "T"),
        (K::KeyU, "KeyU", "U"),
        (K::KeyV, "KeyV", "V"),
        (K::KeyW, "KeyW", "W"),
        (K::KeyX, "KeyX", "X"),
        (K::KeyY, "KeyY", "Y"),
        (K::KeyZ, "KeyZ", "Z"),
        (K::Digit0, "Digit0", "0"),
        (K::Digit1, "Digit1", "1"),
        (K::Digit2, "Digit2", "2"),
        (K::Digit3, "Digit3", "3"),
        (K::Digit4, "Digit4", "4"),
        (K::Digit5, "Digit5", "5"),
        (K::Digit6, "Digit6", "6"),
        (K::Digit7, "Digit7", "7"),
        (K::Digit8, "Digit8", "8"),
        (K::Digit9, "Digit9", "9"),
        (K::F1, "F1", "F1"),
        (K::F2, "F2", "F2"),
        (K::F3, "F3", "F3"),
        (K::F4, "F4", "F4"),
        (K::F5, "F5", "F5"),
        (K::F6, "F6", "F6"),
        (K::F7, "F7", "F7"),
        (K::F8, "F8", "F8"),
        (K::F9, "F9", "F9"),
        (K::F10, "F10", "F10"),
        (K::F11, "F11", "F11"),
        (K::F12, "F12", "F12"),
        (K::ArrowUp, "ArrowUp", "Up"),
        (K::ArrowDown, "ArrowDown", "Down"),
        (K::ArrowLeft, "ArrowLeft", "Left"),
        (K::ArrowRight, "ArrowRight", "Right"),
        (K::Space, "Space", "Space"),
        (K::Tab, "Tab", "Tab"),
        (K::ShiftLeft, "ShiftLeft", "L-Shift"),
        (K::ShiftRight, "ShiftRight", "R-Shift"),
        (K::ControlLeft, "ControlLeft", "L-Ctrl"),
        (K::ControlRight, "ControlRight", "R-Ctrl"),
        (K::AltLeft, "AltLeft", "L-Alt"),
        (K::AltRight, "AltRight", "R-Alt"),
        (K::CapsLock, "CapsLock", "Caps"),
        (K::Backquote, "Backquote", "`"),
        (K::Minus, "Minus", "-"),
        (K::Equal, "Equal", "="),
        (K::BracketLeft, "BracketLeft", "["),
        (K::BracketRight, "BracketRight", "]"),
        (K::Backslash, "Backslash", "\\"),
        (K::Semicolon, "Semicolon", ";"),
        (K::Quote, "Quote", "'"),
        (K::Comma, "Comma", ","),
        (K::Period, "Period", "."),
        (K::Slash, "Slash", "/"),
        (K::Insert, "Insert", "Ins"),
        (K::Delete, "Delete", "Del"),
        (K::Home, "Home", "Home"),
        (K::End, "End", "End"),
        (K::PageUp, "PageUp", "PgUp"),
        (K::PageDown, "PageDown", "PgDn"),
        (K::Numpad0, "Numpad0", "Num 0"),
        (K::Numpad1, "Numpad1", "Num 1"),
        (K::Numpad2, "Numpad2", "Num 2"),
        (K::Numpad3, "Numpad3", "Num 3"),
        (K::Numpad4, "Numpad4", "Num 4"),
        (K::Numpad5, "Numpad5", "Num 5"),
        (K::Numpad6, "Numpad6", "Num 6"),
        (K::Numpad7, "Numpad7", "Num 7"),
        (K::Numpad8, "Numpad8", "Num 8"),
        (K::Numpad9, "Numpad9", "Num 9"),
        (K::NumpadAdd, "NumpadAdd", "Num +"),
        (K::NumpadSubtract, "NumpadSubtract", "Num -"),
        (K::NumpadMultiply, "NumpadMultiply", "Num *"),
        (K::NumpadDivide, "NumpadDivide", "Num /"),
        (K::NumpadDecimal, "NumpadDecimal", "Num ."),
    ]
};

/// Whether `key` can be bound (Esc, Enter and Backspace stay the page's own).
pub(super) fn bindable(key: KeyCode) -> bool {
    KEYS.iter().any(|(k, ..)| *k == key)
}

fn key_id(key: KeyCode) -> String {
    KEYS.iter()
        .find(|(k, ..)| *k == key)
        .map(|(_, id, _)| (*id).to_string())
        .unwrap_or_default()
}

fn parse_key(id: &str) -> Option<KeyCode> {
    KEYS.iter().find(|(_, i, _)| *i == id).map(|(k, ..)| *k)
}

/// A key's short display name.
pub(super) fn key_name(key: KeyCode) -> String {
    KEYS.iter()
        .find(|(k, ..)| *k == key)
        .map(|(_, _, name)| (*name).to_string())
        .unwrap_or_else(|| format!("{key:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_keys_match_the_shipped_controls() {
        let t = Table::default();
        assert_eq!(
            t.get(Bind::Forward),
            [Some(KeyCode::KeyW), Some(KeyCode::ArrowUp)]
        );
        assert_eq!(t.get(Bind::Mobility), [Some(KeyCode::KeyF), None]);
        for b in Bind::ALL {
            for k in b.defaults().into_iter().flatten() {
                assert!(bindable(k), "{b:?}'s stock key {k:?} is bindable");
            }
        }
    }

    #[test]
    fn a_key_does_one_thing_within_its_group() {
        let mut t = Table::default();
        t.set(Bind::Mobility, 0, Some(KeyCode::KeyQ));
        assert_eq!(t.get(Bind::Mobility)[0], Some(KeyCode::KeyQ));
        assert_eq!(t.get(Bind::StrafeLeft), [None, None]);
        // The level-up keys share with play keys freely.
        t.set(Bind::Reroll, 0, Some(KeyCode::KeyF));
        assert_eq!(t.get(Bind::Mobility)[0], Some(KeyCode::KeyQ));
        assert_eq!(t.get(Bind::Reroll)[0], Some(KeyCode::KeyF));
    }

    #[test]
    fn the_table_survives_a_save_and_load() {
        let mut t = Table::default();
        t.set(Bind::Jump, 1, Some(KeyCode::Numpad0));
        t.set(Bind::Forward, 1, None);
        let text = to_toml(&t);
        assert_eq!(from_toml(&text).unwrap(), t, "{text}");
        assert_eq!(from_toml("").unwrap(), Table::default());
    }
}

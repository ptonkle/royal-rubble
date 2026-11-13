//! The view model's pieces: panels and the items inside them. The page has
//! one builder per item `type` and draws a panel without knowing what it
//! means; `kind` names the panel for the stylesheet only. After Crabble's
//! `view.rs`.
//!
//! Every control names an action (`id`) that the page sends back through
//! `act` with the control's new value: a button sends "", a toggle "on" or
//! "off", a slider, a number, a select or a text field the value it shows.

use std::collections::BTreeMap;

use serde::Serialize;

fn is_false(b: &bool) -> bool {
    !*b
}

/// One block of the screen. Empty fields are left out of the JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Panel {
    /// the panel's name for the stylesheet ("title", "settings")
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub heading: String,
    /// a line under the heading
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sub: String,
    pub items: Vec<Item>,
}

impl Panel {
    pub fn new(kind: &str, heading: String, items: Vec<Item>) -> Panel {
        Panel {
            kind: kind.into(),
            heading,
            sub: String::new(),
            items,
        }
    }
}

/// One choice of a select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub value: String,
    pub text: String,
}

/// One version on the What's New screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteEntry {
    pub version: String,
    pub date: String,
    pub minor: bool,
    pub lines: Vec<String>,
}

/// One item in a panel, by `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Item {
    Button {
        id: String,
        text: String,
        /// drawn smaller, for a less used action
        #[serde(skip_serializing_if = "is_false")]
        quiet: bool,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
        /// a short hover tip, 1-4 words or a label and a cost
        #[serde(skip_serializing_if = "String::is_empty")]
        tip: String,
        /// an icon's data URL (`app::hud_icon`), drawn before the text
        #[serde(skip_serializing_if = "String::is_empty")]
        icon: String,
    },
    Slider {
        id: String,
        label: String,
        min: u32,
        max: u32,
        step: u32,
        value: u32,
        /// the value as the reader reads it ("80%")
        text: String,
    },
    Toggle {
        id: String,
        label: String,
        value: bool,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
    },
    Select {
        id: String,
        label: String,
        value: String,
        options: Vec<Choice>,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
    },
    /// a one-line text field; the page sends its text when it changes
    Text {
        id: String,
        label: String,
        value: String,
        /// the most characters it takes
        max: u32,
        /// shown grayed while the field is empty
        #[serde(skip_serializing_if = "String::is_empty")]
        placeholder: String,
    },
    /// a whole number the page sends when it changes; the core checks it
    Number {
        id: String,
        label: String,
        value: u32,
        min: u32,
        max: u32,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
        /// shown in place of the value when not empty (the lobby's "???")
        #[serde(skip_serializing_if = "String::is_empty")]
        text: String,
    },
    /// short lines of text, one after another; `id`, when not empty, is
    /// for the checks to find them
    Lines {
        #[serde(skip_serializing_if = "String::is_empty")]
        id: String,
        lines: Vec<String>,
    },
    Notes {
        id: String,
        entries: Vec<NoteEntry>,
    },
    /// a Copy button: the page puts `copy` on the clipboard when it is
    /// pressed; `value`, when not empty, is shown big beside it under
    /// `label` (the room code)
    Copy {
        id: String,
        label: String,
        value: String,
        copy: String,
        /// the button's text
        text: String,
    },
    /// a box of a long pasted code: a field the page sends on every change,
    /// or, `read_only`, the code to copy out
    CodeBox {
        id: String,
        label: String,
        value: String,
        #[serde(skip_serializing_if = "is_false")]
        read_only: bool,
    },
    /// one seat of the lobby: who sits there, and the controls this browser
    /// may use on it
    Seat {
        id: String,
        /// "person", "bot" or "empty": the stylesheet's key
        look: String,
        name: String,
        /// who this is to the reader, and Ready: "You \u{b7} Ready", "Host", ""
        note: String,
        /// this browser's own seat
        #[serde(skip_serializing_if = "is_false")]
        me: bool,
        #[serde(skip_serializing_if = "is_false")]
        ready: bool,
        /// a bot's difficulty in words, shown when `controls` has no select
        #[serde(skip_serializing_if = "String::is_empty")]
        role: String,
        controls: Vec<Item>,
    },
    /// an upgrade card: a press sends `value`; `look` is the tier's key
    /// for the stylesheet
    Card {
        id: String,
        value: String,
        name: String,
        tier: String,
        look: String,
        text: String,
        /// the card's icon by what it changes
        #[serde(skip_serializing_if = "String::is_empty")]
        icon: String,
        #[serde(skip_serializing_if = "is_false")]
        picked: bool,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
    },
    /// a row of choices, one chosen (the material bar, the shot bar); a
    /// press sends the option's `value`
    Bar {
        id: String,
        options: Vec<BarOption>,
    },
    /// an icon and a short value, for the top bar (gold, round, target)
    Stat {
        id: String,
        icon: String,
        icon_name: String,
        text: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        tip: String,
    },
    /// the phase clock, drawn big; `urgent` in its last seconds
    Timer {
        id: String,
        icon: String,
        phase: String,
        text: String,
        #[serde(skip_serializing_if = "is_false")]
        urgent: bool,
    },
    /// a filled bar with an icon and a label (level and XP)
    Meter {
        id: String,
        icon: String,
        icon_name: String,
        label: String,
        value: u32,
        max: u32,
        text: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        tip: String,
    },
    /// every player's health, sorted, as a column of hearts and bars
    Health {
        id: String,
        icon: String,
        rows: Vec<HealthRow>,
    },
}

/// One choice on a bar: its key, its name, and whether it is the one chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BarOption {
    pub value: String,
    pub text: String,
    /// the key that picks it, shown small
    pub key: String,
    #[serde(skip_serializing_if = "is_false")]
    pub selected: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub disabled: bool,
    /// a short hover tip, 1-4 words or a label and a cost
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tip: String,
    /// an icon's data URL and its name for the stylesheet's color
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon_name: String,
    /// a count shown big on the slot: stock, ammo left, royals to place
    #[serde(skip_serializing_if = "String::is_empty")]
    pub count: String,
    /// the gold price shown under the slot; empty when it costs nothing
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cost: String,
    /// options with another group start a new run of slots after a gap
    #[serde(skip_serializing_if = "String::is_empty")]
    pub group: String,
}

/// One player in the health column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthRow {
    pub name: String,
    pub value: u32,
    pub max: u32,
    /// the health as text ("88")
    pub text: String,
    #[serde(skip_serializing_if = "is_false")]
    pub me: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub out: bool,
    /// "target" for the castle you attack, else empty
    #[serde(skip_serializing_if = "String::is_empty")]
    pub look: String,
}

/// A term and its definition, for the page's hover hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hint {
    pub term: String,
    pub definition: String,
}

/// Everything the page draws. The page keeps none of it between calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ui {
    pub screen: String,
    pub title: String,
    /// the version button's text
    pub version: String,
    /// the version button opens What's New (not during a match)
    pub version_opens: bool,
    /// the version button shows: on menus, never over a field (R1.3201)
    pub version_shown: bool,
    pub panels: Vec<Panel>,
    pub hints: Vec<Hint>,
    /// the tab icon, as a data URL
    pub icon: String,
    /// the save text for the page's store; it writes when this changes
    pub save: String,
    /// a save this build refused, for the page to keep beside the new one
    #[serde(skip_serializing_if = "String::is_empty")]
    pub keep_refused: String,
    /// the page offers the refused save as a file, then sends `save.offered`
    #[serde(skip_serializing_if = "is_false")]
    pub offer_old_file: bool,
    /// sound level, percent
    pub volume: u32,
    /// music level, percent, under the sound level (the sound module reads it)
    pub music: u32,
    /// the main gain for Web Audio, as a decimal ("0.150")
    pub master_gain: String,
    /// the music's gain under the main gain, as a decimal
    pub music_gain: String,
    /// the sound and music sliders' shares, 0 to 1, as decimals ("0.800"),
    /// for the sound module, which squares and scales them itself
    pub sound_share: String,
    pub music_share: String,
    /// the view model changes without an action: read it every frame
    pub live: bool,
    /// a match is showing: keys go to the core
    pub capture: bool,
    /// the input bit of each key code while `capture` is on
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, u32>,
    /// the address's `#room=CODE` while this browser is in a room
    pub hash: String,
    /// a status line, or nothing
    pub status: String,
    /// the music while no playfield shows: the menu theme on the menus,
    /// "" for none (R1.3802)
    pub menu_music: String,
    /// the playfield's canvas, while one shows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<crate::app::field::Frame>,
    /// An online match, for the two-browser check to compare: round, then
    /// the newest checksum's tick and value (`body[data-sync]`).
    pub sync: String,
    /// The action Esc sends on this screen, or empty.
    pub escape: String,
    /// Volume slider moves so far; the page plays `sample_cue` on each.
    pub samples: u32,
    pub sample_cue: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_says_its_type_and_leaves_out_what_is_off() {
        let b = Item::Button {
            id: "a".into(),
            text: "A".into(),
            quiet: false,
            disabled: false,
            tip: String::new(),
            icon: String::new(),
        };
        assert_eq!(
            serde_json::to_string(&b).unwrap(),
            r#"{"type":"button","id":"a","text":"A"}"#
        );
    }
}

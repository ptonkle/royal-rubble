//! How to Play (a tab bar over short label-value lines) and the upgrade
//! list (every dealt upgrade with its icon, filtered by tier). Both draw from
//! the strings and the upgrade table; neither changes a rule.

use crate::game::upgrades::{Tier, UPGRADES};
use crate::strings::Strings;
use crate::view::{BarOption, Entry, Item, Pair, Panel};

use super::arena::{self, CardText};
use super::hud_icon;

/// How to Play's tabs: the value a press sends, the tab's name key and its
/// lines' keys. Each line is "Label: text"; the core splits it.
pub const HOW_TABS: &[(&str, &str, &[&str])] = &[
    (
        "build",
        "phase.build",
        &[
            "how.build.place",
            "how.build.sell",
            "how.build.undo",
            "how.build.test",
            "how.build.royals",
            "how.build.producers",
        ],
    ),
    (
        "siege",
        "phase.siege",
        &[
            "how.siege.throw",
            "how.siege.ammo",
            "how.siege.tricks",
            "how.siege.wind",
            "how.siege.aim",
        ],
    ),
    (
        "upgrades",
        "how.tab.upgrades",
        &[
            "how.upgrades.pick",
            "how.upgrades.reroll",
            "how.upgrades.tiers",
            "how.upgrades.lasts",
        ],
    ),
    (
        "winning",
        "how.tab.winning",
        &[
            "how.winning.health",
            "how.winning.damage",
            "how.winning.gold",
            "how.winning.level",
            "how.winning.win",
        ],
    ),
];

/// The upgrade list's tier filter: "all", then each tier.
pub const TIER_FILTERS: &[&str] = &["all", "silver", "gold", "prismatic"];

/// Splits "Label: text" at its first colon; a line with none is all text.
fn pair(line: &str) -> Pair {
    match line.split_once(": ") {
        Some((l, t)) => Pair {
            label: l.to_string(),
            text: t.to_string(),
        },
        None => Pair {
            label: String::new(),
            text: line.to_string(),
        },
    }
}

fn back(s: &Strings) -> Item {
    Item::Button {
        id: "menu.back".into(),
        text: s.get("menu.back"),
        quiet: true,
        disabled: false,
        tip: String::new(),
        icon: String::new(),
        cost: String::new(),
    }
}

fn tab_bar(id: &str, options: Vec<(String, String, bool)>) -> Item {
    Item::Bar {
        id: id.into(),
        options: options
            .into_iter()
            .map(|(value, text, selected)| BarOption {
                value,
                text,
                key: String::new(),
                selected,
                disabled: false,
                tip: String::new(),
                icon: String::new(),
                icon_name: String::new(),
                count: String::new(),
                cost: String::new(),
                group: String::new(),
            })
            .collect(),
    }
}

/// The How to Play screen on tab `tab` (a value from `HOW_TABS`).
pub fn how_panels(s: &Strings, tab: &str) -> Vec<Panel> {
    let tabs = HOW_TABS
        .iter()
        .map(|(v, k, _)| (v.to_string(), s.get(k), *v == tab))
        .collect();
    let lines = HOW_TABS
        .iter()
        .find(|(v, _, _)| *v == tab)
        .map_or(&[][..], |(_, _, l)| *l);
    let mut items = vec![
        tab_bar("how.tab", tabs),
        Item::Pairs {
            id: format!("how.{tab}"),
            rows: lines.iter().map(|k| pair(&s.get(k))).collect(),
        },
    ];
    if tab == "upgrades" {
        items.push(Item::Button {
            id: "menu.upgrades".into(),
            text: s.get("how.all_upgrades"),
            quiet: false,
            disabled: false,
            tip: String::new(),
            icon: hud_icon::url("upgrade"),
            cost: String::new(),
        });
    }
    items.push(back(s));
    vec![Panel::new("how", s.get("how.heading"), items)]
}

fn tier_key(t: Tier) -> &'static str {
    arena::tier_name(t)
}

/// The upgrade list, showing tier `filter` (a value from `TIER_FILTERS`).
/// It lists the upgrades a match deals, in the table's order.
pub fn upgrades_panels(s: &Strings, cards: &CardText, filter: &str) -> Vec<Panel> {
    let tabs = TIER_FILTERS
        .iter()
        .map(|f| {
            let text = if *f == "all" {
                s.get("upgrades.all")
            } else {
                s.get(&format!("tier.{f}"))
            };
            (f.to_string(), text, *f == filter)
        })
        .collect();
    let entries = UPGRADES
        .iter()
        .filter(|r| r.dealable && (filter == "all" || tier_key(r.tier) == filter))
        .map(|r| {
            let (name, text) = cards.get(r.key);
            Entry {
                id: format!("upgrades.{}", r.key),
                name,
                tier: s.get(&format!("tier.{}", tier_key(r.tier))),
                look: tier_key(r.tier).to_string(),
                text,
                icon: hud_icon::url(arena::card_icon(r.effects)),
            }
        })
        .collect();
    vec![Panel::new(
        "upgrades",
        s.get("upgrades.heading"),
        vec![
            tab_bar("upgrades.tier", tabs),
            Item::Entries {
                id: "upgrades.list".into(),
                entries,
            },
            back(s),
        ],
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_how_line_splits_into_label_and_text_at_its_first_colon() {
        let p = pair("Undo: Z: again");
        assert_eq!((p.label.as_str(), p.text.as_str()), ("Undo", "Z: again"));
        let p = pair("Plain text");
        assert_eq!((p.label.as_str(), p.text.as_str()), ("", "Plain text"));
    }
}

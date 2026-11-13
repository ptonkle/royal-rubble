//! The lobby and the match as panels: the Players panel and the Match
//! panel. Every rule here is a question put to the room; this file only
//! words the answer. Ported from Fleeced's `app/lobby_view.rs`.

use super::App;
use crate::lobby::{Difficulty, Who, MAX_SEATS, MIN_PLAYERS};
use crate::online::room::{Kind, Room};
use crate::view::{Choice, Item, Panel};

impl App {
    /// A seat's name as it reads: the bot's word, or the person's name, or
    /// "Player 3" for a person who has given none.
    pub(super) fn seat_name(&self, room: &Room, n: usize) -> String {
        match &room.lobby.seats[n] {
            Who::Empty => String::new(),
            Who::Bot { .. } => self.strings.get("seat.bot"),
            Who::Person { name, .. } if name.is_empty() => self
                .strings
                .fill("seat.default_name", &[("n", (n + 1).to_string())]),
            Who::Person { name, .. } => name.clone(),
        }
    }

    fn level_word(&self, level: Difficulty) -> String {
        self.strings.get(&format!("bot.{}", level.key()))
    }

    fn seat_item(&self, room: &Room, n: usize) -> Item {
        let seat = &room.lobby.seats[n];
        let mine = room.lobby.seat_of(&room.client) == Some(n);
        let host = room.is_host();
        let mut controls = Vec::new();
        let mut role = String::new();
        if let Who::Bot { level } = seat {
            if host {
                controls.push(Item::Select {
                    id: format!("seat.{n}.level"),
                    label: String::new(),
                    value: level.key().to_string(),
                    options: Difficulty::ALL
                        .iter()
                        .map(|d| Choice {
                            value: d.key().to_string(),
                            text: self.level_word(*d),
                        })
                        .collect(),
                    disabled: false,
                });
            } else {
                role = self.level_word(*level);
            }
        }
        let is_host_seat = matches!(seat, Who::Person { id, .. } if *id == room.lobby.host);
        let ready = matches!(seat, Who::Person { ready: true, .. });
        let mut notes = Vec::new();
        if mine {
            notes.push(self.strings.get("seat.you"));
        } else if is_host_seat && room.kind != Kind::Solo {
            notes.push(self.strings.get("seat.host"));
        }
        if ready {
            notes.push(self.strings.get("seat.is_ready"));
        }
        Item::Seat {
            id: format!("seat.{n}"),
            look: match seat {
                Who::Empty => "empty",
                Who::Bot { .. } => "bot",
                Who::Person { .. } => "person",
            }
            .to_string(),
            name: self.seat_name(room, n),
            note: notes.join(" \u{b7} "),
            me: mine,
            ready,
            role,
            controls,
        }
    }

    /// A pasted code to send, and its Copy button; nothing until it is made.
    fn code_out(&self, id: &str, label: &str, code: &str) -> Vec<Item> {
        if code.is_empty() {
            return Vec::new();
        }
        vec![
            Item::CodeBox {
                id: id.into(),
                label: self.strings.get(label),
                value: code.to_string(),
                read_only: true,
            },
            Item::Copy {
                id: format!("{id}.copy"),
                label: String::new(),
                value: String::new(),
                copy: code.to_string(),
                text: self.strings.get("lobby.copy_code"),
            },
        ]
    }

    /// The host's side of meeting by pasted codes.
    fn invite_items(&self, room: &Room) -> Vec<Item> {
        let invite = room.pasted_code().unwrap_or_default();
        let mut items = self.code_out("online.offer_out", "online.invite_code", invite);
        items.push(Item::CodeBox {
            id: "online.answer_in".into(),
            label: self.strings.get("online.reply_code"),
            value: self.typed_reply.clone(),
            read_only: false,
        });
        items.push(Item::Button {
            id: "online.accept_answer".into(),
            text: self.strings.get("online.connect"),
            quiet: false,
            disabled: self.typed_reply.trim().is_empty(),
        });
        items
    }

    /// The joiner's side: stays up until the host's lobby is heard.
    pub(super) fn paste_panels(&self, room: Option<&Room>) -> Vec<Panel> {
        let reply = room.and_then(Room::pasted_code).unwrap_or_default();
        let busy = room.is_some_and(Room::making_code);
        let mut items = vec![
            Item::CodeBox {
                id: "online.offer_in".into(),
                label: self.strings.get("online.invite_code"),
                value: self.typed_invite.clone(),
                read_only: false,
            },
            Item::Button {
                id: "online.make_answer".into(),
                text: self.strings.get("online.make_reply"),
                quiet: false,
                disabled: busy || self.typed_invite.trim().is_empty(),
            },
        ];
        items.extend(self.code_out("online.answer_out", "online.reply_code", reply));
        items.push(self.quiet("online.paste_back", "menu.back"));
        vec![Panel::new("online", self.strings.get("online.paste_join"), items)]
    }

    fn seats_panel(&self, room: &Room) -> Panel {
        let mut items = Vec::new();
        if room.pasted_code().is_some() {
            if room.is_host() {
                items.extend(self.invite_items(room));
            }
        } else if room.kind != Kind::Solo {
            items.push(Item::Copy {
                id: "lobby.copy_code".into(),
                label: self.strings.get("lobby.code"),
                value: room.code.clone(),
                copy: room.code.clone(),
                text: self.strings.get("lobby.copy_code"),
            });
            items.push(Item::Copy {
                id: "lobby.copy_link".into(),
                label: String::new(),
                value: String::new(),
                copy: format!("{}#room={}", self.page_url, room.code),
                text: self.strings.get("lobby.copy_link"),
            });
        }
        let mine = room.lobby.seat_of(&room.client);
        for n in 0..MAX_SEATS {
            if room.lobby.seats[n] == Who::Empty {
                continue;
            }
            items.push(self.seat_item(room, n));
            if mine == Some(n) && room.kind != Kind::Solo {
                items.push(Item::Text {
                    id: "lobby.name".into(),
                    label: self.strings.get("lobby.name"),
                    value: self.save.name.clone(),
                    max: crate::lobby::MAX_NAME_CHARS as u32,
                    placeholder: self.strings.get("lobby.name_hint"),
                });
            }
        }
        let heading = if room.kind == Kind::Solo {
            "lobby.solo_heading"
        } else {
            "lobby.seats"
        };
        Panel::new("seats", self.strings.get(heading), items)
    }

    fn match_panel(&self, room: &Room) -> Panel {
        let host = room.is_host();
        let o = &room.lobby.options;
        let mut items = vec![
            Item::Number {
                id: "lobby.players".into(),
                label: self.strings.get("lobby.players"),
                value: o.players,
                min: MIN_PLAYERS as u32,
                max: MAX_SEATS as u32,
                disabled: !host,
                text: String::new(),
            },
            Item::Toggle {
                id: "lobby.wind".into(),
                label: self.strings.get("lobby.wind"),
                value: o.wind,
                disabled: !host,
            },
            Item::Toggle {
                id: "lobby.flat".into(),
                label: self.strings.get("lobby.flat"),
                value: o.flat,
                disabled: !host,
            },
        ];
        let problem = room.lobby.start_problem();
        if let Some(key) = problem {
            items.push(Item::Lines {
                id: "lobby.problem".into(),
                lines: vec![self.strings.get(key)],
            });
        }
        // Leave sits above, so Start or Ready is the last thing in the panel
        items.push(self.quiet("lobby.leave", "lobby.leave"));
        if host {
            items.push(Item::Button {
                id: "lobby.start".into(),
                text: self.strings.get("lobby.start"),
                quiet: false,
                disabled: problem.is_some(),
            });
        } else {
            // the button says what pressing it does
            let key = if room.am_ready() {
                "lobby.not_ready"
            } else {
                "lobby.ready"
            };
            items.push(self.button("lobby.ready", key));
        }
        Panel::new("match", self.strings.get("lobby.match"), items)
    }

    pub(super) fn lobby_panels(&self, room: &Room) -> Vec<Panel> {
        if room.heard {
            return vec![self.seats_panel(room), self.match_panel(room)];
        }
        vec![Panel::new(
            "finding",
            self.strings.get("lobby.heading"),
            vec![
                Item::Lines {
                    id: "lobby.code".into(),
                    lines: vec![room.code.clone()],
                },
                self.quiet("lobby.leave", "lobby.leave"),
            ],
        )]
    }

    /// The countdown before an online match. `state` is what the online
    /// check compares: the round, then the newest checksum with its tick.
    pub(super) fn play_panels(&self, room: &Room) -> Vec<Panel> {
        let mut items = Vec::new();
        if let Some(n) = room.counting(self.now_ms) {
            items.push(Item::Lines {
                id: "play.count".into(),
                lines: vec![self.strings.fill("play.starting", &[("n", n.to_string())])],
            });
        }
        items.push(Item::Lines {
            id: "state".into(),
            lines: vec![state_line(room)],
        });
        items.push(self.quiet("play.leave", "play.leave"));
        vec![Panel::new("play", self.strings.get("play.placeholder"), items)]
    }
}

pub(super) fn state_line(room: &Room) -> String {
    let round = room.game.as_ref().map_or(0, |game| game.play.rules.round);
    let (tick, sum) = room.session.newest_sum().unwrap_or((0, 0));
    format!("{round} : {tick} : {sum}")
}

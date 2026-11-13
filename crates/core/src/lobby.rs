//! The lobby: who sits in each seat and the host's match options. Pure rules
//! with no transport: `online::room` carries the lobby between browsers, and
//! a match against bots uses the same lobby with no one else in it. The
//! host's copy is the true one; a joiner draws the last roster it got.
//! Ported from Fleeced's `lobby.rs`, without its teams and roles.
//!
//! What the lobby holds, each with a test below:
//!
//! - the host picks the number of players, from [`MIN_PLAYERS`] to
//!   [`MAX_SEATS`]; the open seats are the first that many;
//! - every open seat holds a person or a bot, so bots fill empty seats, and a
//!   bot gives way to a joining person;
//! - each bot has a difficulty the host picks;
//! - the host can't pick fewer players than there are people;
//! - the match starts once every person but the host has pressed Ready.

use serde::{Deserialize, Serialize};

/// The most seats in a match, people and bots together (BRIEF: 2 to 8).
pub const MAX_SEATS: usize = 8;
/// The fewest players in a match.
pub const MIN_PLAYERS: usize = 2;
/// The longest player name, in characters.
pub const MAX_NAME_CHARS: usize = 16;

/// A status key: a string in `data/` says what stands in the way.
pub const NEED_READY: &str = "lobby.need_ready";

/// How well a bot plays.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    /// The action value, and `bot.<key>` in the strings.
    pub fn key(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }

    pub fn parse(text: &str) -> Option<Difficulty> {
        Difficulty::ALL.into_iter().find(|d| d.key() == text)
    }
}

/// Who sits in a seat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum Who {
    /// A closed seat: past the number of players.
    #[default]
    Empty,
    Person {
        /// the browser's client id, stable for its tab
        id: String,
        name: String,
        /// Ready. The host's own mark is unused: its Start is its Ready.
        ready: bool,
    },
    Bot {
        level: Difficulty,
    },
}

/// The host's match options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Options {
    pub players: u32,
    pub wind: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            players: MIN_PLAYERS as u32,
            wind: true,
        }
    }
}

impl Options {
    /// Options a host could have sent.
    pub fn valid(&self) -> bool {
        (MIN_PLAYERS as u32..=MAX_SEATS as u32).contains(&self.players)
    }
}

/// One seat as a match starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSeat {
    pub seat: u32,
    /// the person's client id, empty for a bot
    pub person: String,
    pub name: String,
    /// a bot's difficulty; None for a person
    pub bot: Option<Difficulty>,
}

/// Everything every browser needs to build the same match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchStart {
    pub seed: u64,
    pub seats: Vec<StartSeat>,
    pub options: Options,
}

/// The room before a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lobby {
    pub seats: Vec<Who>,
    pub options: Options,
    /// the host's client id
    pub host: String,
}

/// A name as it is kept: no control characters, no spaces at the ends, and
/// at most [`MAX_NAME_CHARS`] characters.
pub fn clean_name(text: &str) -> String {
    let kept: String = text.chars().filter(|c| !c.is_control()).collect();
    kept.trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

impl Lobby {
    /// A lobby with the host in the first seat and bots in the other open
    /// seats.
    pub fn new(host_id: &str, host_name: &str) -> Lobby {
        let mut lobby = Lobby {
            seats: vec![Who::Empty; MAX_SEATS],
            options: Options::default(),
            host: host_id.to_string(),
        };
        lobby.seats[0] = Who::Person {
            id: host_id.to_string(),
            name: clean_name(host_name),
            ready: false,
        };
        lobby.refill();
        lobby
    }

    /// A roster a host could have sent: the right number of seats.
    pub fn is_whole(&self) -> bool {
        self.seats.len() == MAX_SEATS && self.options.valid()
    }

    /// The seat a client id sits in.
    pub fn seat_of(&self, id: &str) -> Option<usize> {
        self.seats
            .iter()
            .position(|s| matches!(s, Who::Person { id: i, .. } if i == id))
    }

    fn people(&self) -> usize {
        self.seats
            .iter()
            .filter(|s| matches!(s, Who::Person { .. }))
            .count()
    }

    /// Open seats get a bot when empty; closed seats lose their bot.
    fn refill(&mut self) {
        let open = self.options.players as usize;
        for (n, s) in self.seats.iter_mut().enumerate() {
            match (n < open, &s) {
                (true, Who::Empty) => {
                    *s = Who::Bot {
                        level: Difficulty::default(),
                    }
                }
                (false, Who::Bot { .. }) => *s = Who::Empty,
                _ => {}
            }
        }
    }

    /// Seats a person in the first open bot seat. None when people fill
    /// every open seat. Someone already seated stays where they are.
    pub fn add_person(&mut self, id: &str, name: &str) -> Option<usize> {
        if let Some(seat) = self.seat_of(id) {
            return Some(seat);
        }
        let free = self.seats.iter().position(|s| matches!(s, Who::Bot { .. }))?;
        self.seats[free] = Who::Person {
            id: id.to_string(),
            name: clean_name(name),
            ready: false,
        };
        Some(free)
    }

    /// A person leaves; a bot takes the seat.
    pub fn remove_person(&mut self, id: &str) {
        if let Some(seat) = self.seat_of(id) {
            self.seats[seat] = Who::Empty;
            self.refill();
        }
    }

    pub fn set_name(&mut self, id: &str, name: &str) {
        if let Some(seat) = self.seat_of(id) {
            if let Who::Person { name: n, .. } = &mut self.seats[seat] {
                *n = clean_name(name);
            }
        }
    }

    pub fn set_ready(&mut self, id: &str, value: bool) {
        if let Some(seat) = self.seat_of(id) {
            if let Who::Person { ready, .. } = &mut self.seats[seat] {
                *ready = value;
            }
        }
    }

    /// A bot's difficulty. A person's seat is left as it is.
    pub fn set_level(&mut self, seat: usize, value: Difficulty) {
        if let Some(Who::Bot { level }) = self.seats.get_mut(seat) {
            *level = value;
        }
    }

    /// The number of players, kept within the limits and never below the
    /// people seated. A person sitting past the new count moves into a bot
    /// seat inside it first.
    pub fn set_players(&mut self, players: u32) {
        let fewest = self.people().max(MIN_PLAYERS) as u32;
        let players = players.clamp(fewest, MAX_SEATS as u32);
        let open = players as usize;
        for n in open..MAX_SEATS {
            if matches!(self.seats[n], Who::Person { .. }) {
                if let Some(free) = self.seats[..open]
                    .iter()
                    .position(|s| matches!(s, Who::Bot { .. }))
                {
                    self.seats.swap(n, free);
                }
            }
        }
        self.options.players = players;
        self.refill();
    }

    pub fn set_wind(&mut self, on: bool) {
        self.options.wind = on;
    }

    /// Every person but the host has pressed Ready.
    pub fn everyone_ready(&self) -> bool {
        self.seats.iter().all(|s| match s {
            Who::Person { id, ready, .. } => *ready || *id == self.host,
            _ => true,
        })
    }

    /// What stands in the way of starting, as a status key, or None.
    pub fn start_problem(&self) -> Option<&'static str> {
        (!self.everyone_ready()).then_some(NEED_READY)
    }

    /// The match this lobby starts from `seed`. The refusal is a status key.
    pub fn start(&self, seed: u64) -> Result<MatchStart, &'static str> {
        if let Some(problem) = self.start_problem() {
            return Err(problem);
        }
        let seats = self
            .seats
            .iter()
            .enumerate()
            .filter_map(|(n, s)| {
                let (person, name, bot) = match s {
                    Who::Empty => return None,
                    Who::Person { id, name, .. } => (id.clone(), name.clone(), None),
                    Who::Bot { level } => (String::new(), String::new(), Some(*level)),
                };
                Some(StartSeat {
                    seat: n as u32,
                    person,
                    name,
                    bot,
                })
            })
            .collect();
        Ok(MatchStart {
            seed,
            seats,
            options: self.options.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bots(l: &Lobby) -> usize {
        l.seats.iter().filter(|s| matches!(s, Who::Bot { .. })).count()
    }

    #[test]
    fn a_new_lobby_is_the_host_and_one_normal_bot_with_wind_on() {
        let l = Lobby::new("h", "Ann");
        assert_eq!(
            l.options,
            Options {
                players: 2,
                wind: true
            }
        );
        assert!(matches!(&l.seats[0], Who::Person { name, .. } if name == "Ann"));
        assert_eq!(
            l.seats[1],
            Who::Bot {
                level: Difficulty::Normal
            }
        );
        assert_eq!(l.seats[2], Who::Empty);
    }

    #[test]
    fn bots_fill_the_open_seats_and_give_way_to_a_person() {
        let mut l = Lobby::new("h", "Ann");
        l.set_players(5);
        assert_eq!(bots(&l), 4);
        assert_eq!(l.add_person("j", "Bo"), Some(1));
        assert_eq!(bots(&l), 3);
        l.remove_person("j");
        assert_eq!(bots(&l), 4);
    }

    #[test]
    fn a_full_room_turns_a_person_away() {
        let mut l = Lobby::new("h", "Ann");
        assert_eq!(l.add_person("j", "Bo"), Some(1));
        assert_eq!(l.add_person("k", "Cy"), None);
        assert_eq!(l.add_person("j", "Bo"), Some(1), "someone seated stays put");
    }

    #[test]
    fn players_stay_within_two_to_eight_and_never_below_the_people() {
        let mut l = Lobby::new("h", "Ann");
        l.set_players(1);
        assert_eq!(l.options.players, 2);
        l.set_players(12);
        assert_eq!(l.options.players, 8);
        l.add_person("j", "Bo");
        l.add_person("k", "Cy");
        l.set_players(2);
        assert_eq!(l.options.players, 3);
    }

    #[test]
    fn fewer_players_moves_a_late_seat_inward() {
        let mut l = Lobby::new("h", "Ann");
        l.set_players(4);
        l.seats.swap(1, 3);
        l.add_person("j", "Bo");
        assert_eq!(l.seat_of("j"), Some(1));
        l.seats.swap(1, 3);
        l.set_players(2);
        assert_eq!(l.seat_of("j"), Some(1));
        assert_eq!(l.seats[3], Who::Empty);
    }

    #[test]
    fn the_host_sets_a_bots_difficulty_and_not_a_persons() {
        let mut l = Lobby::new("h", "Ann");
        l.set_level(1, Difficulty::Hard);
        assert_eq!(
            l.seats[1],
            Who::Bot {
                level: Difficulty::Hard
            }
        );
        l.set_level(0, Difficulty::Easy);
        assert!(matches!(l.seats[0], Who::Person { .. }));
        assert_eq!(Difficulty::parse("easy"), Some(Difficulty::Easy));
        assert_eq!(Difficulty::parse("Easy"), None);
    }

    #[test]
    fn the_match_waits_for_every_joiner_to_be_ready() {
        let mut l = Lobby::new("h", "Ann");
        l.add_person("j", "Bo");
        assert_eq!(l.start(1).unwrap_err(), NEED_READY);
        l.set_ready("j", true);
        let start = l.start(7).unwrap();
        assert_eq!(start.seed, 7);
        assert_eq!(start.seats.len(), 2);
        assert_eq!(start.seats[1].person, "j");
    }

    #[test]
    fn a_start_lists_each_bot_with_its_difficulty() {
        let mut l = Lobby::new("h", "Ann");
        l.set_players(3);
        l.set_level(2, Difficulty::Easy);
        let start = l.start(1).unwrap();
        assert_eq!(start.seats[0].bot, None);
        assert_eq!(start.seats[1].bot, Some(Difficulty::Normal));
        assert_eq!(start.seats[2].bot, Some(Difficulty::Easy));
    }

    #[test]
    fn a_name_loses_control_characters_and_stops_at_its_limit() {
        assert_eq!(clean_name("  Ann\u{7}e  "), "Anne");
        assert_eq!(clean_name("abcdefghijklmnopqrst"), "abcdefghijklmnop");
    }
}

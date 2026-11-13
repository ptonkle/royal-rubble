//! The match screen: a real `play::Play` match against bots, on the
//! sandbox's playfield. The page sends pointer pixels and keys as it does
//! for the sandbox; this module turns them into `play::Input`s (R1.807),
//! steps the match at 60 ticks a second, and describes the field and the
//! HUD for the phase: Upgrade cards, Build with its tools, Siege with the
//! trebuchet and a corner view of your own castle, a replay of the hits on
//! your castle, and the results.
//!
//! Solo, the arena steps its own match through `play::online::step`. Online
//! (`Arena::online`), the room's session steps the match on every browser;
//! the arena hands the room this browser's inputs and follows the session's
//! match each frame (R1.1502).

use std::collections::BTreeMap;

use super::field::{
    body_turn, cloth_at, facing, ground_line, impact_marks, material_name, tree_marks, world_parts,
    world_right, BuildMarks, Camera, Cue, Engine, Figure, Flag, Frame, Inset, Label, Preview, Ring, SpotMark,
};
use super::hud_icon;
use super::sandbox::{num, pair, reach_end, snap_to};
use crate::game::flow::Phase;
use crate::game::ledger::{self, BuildingKind, Piece};
use crate::game::royals::RoyalKind;
use crate::game::upgrades::{AirKind, AmmoKind, Tier, UPGRADES};
use crate::game::PlayerId;
use crate::lobby::MatchStart;
use crate::physics::trebuchet::{pose, Phase as Arm};
use crate::physics::world::snap;
use crate::physics::{Fx, Material, Vec2, World, TUNE};
use crate::play::castle::{self, BodyOwner, CastleWorld};
use crate::play::online::{self, Logged, NetMatch, SiegeLog};
use crate::play::siege::{Ammo, Field};
use crate::play::units::GridPt;
use crate::play::{Input, Play};
use crate::strings::Strings;
use crate::terrain::{Plot, SpotKind};
use crate::view::{BarOption, HealthRow, Item, Panel};

/// Input bits, by key code (the page reads them from `Ui::keys`).
const KEY_SPACE: u32 = 1;
const KEY_DIGITS: [(&str, u32); 5] = [
    ("Digit1", 1 << 1),
    ("Digit2", 1 << 2),
    ("Digit3", 1 << 3),
    ("Digit4", 1 << 4),
    ("Digit5", 1 << 5),
];
const KEY_AIR: [(&str, u32); 4] = [
    ("KeyQ", 1 << 6),
    ("KeyW", 1 << 7),
    ("KeyE", 1 << 8),
    ("KeyR", 1 << 9),
];
/// Esc lets go of a segment's start; Z takes back the last piece placed.
const KEY_CANCEL: u32 = 1 << 10;
const KEY_UNDO: u32 = 1 << 11;

/// The keys the match listens to.
pub fn keys() -> BTreeMap<String, u32> {
    let mut k = BTreeMap::new();
    k.insert("Space".to_string(), KEY_SPACE);
    k.insert("Escape".to_string(), KEY_CANCEL);
    k.insert("KeyZ".to_string(), KEY_UNDO);
    for (code, bit) in KEY_DIGITS.iter().chain(KEY_AIR.iter()) {
        k.insert(code.to_string(), *bit);
    }
    k
}

/// Ticks per second.
const TICKS: i64 = crate::physics::tune::TICKS_PER_SECOND as i64;
/// Most ticks one display frame may catch up.
const MAX_CATCH_UP: i64 = 4;
/// `?fast`: the match runs this many times faster, so a check reaches the
/// results in seconds.
const FAST_SPEED: i64 = 8;
/// A trail point every this many ticks.
const TRAIL_EVERY: u32 = 2;
/// The replay runs this many ticks per match tick, for at most this long.
const REPLAY_SPEED: u32 = 2;
const REPLAY_MAX_TICKS: u32 = 600;
/// The corner window's size and gap from the screen's right edge, pixels.
const INSET_W: i32 = 480;
const INSET_H: i32 = 270;
const INSET_GAP: i32 = 24;
const INSET_TOP: i32 = 180;
/// After a hit on your castle the corner window shakes this many pixels
/// for this many ticks, and its frame flashes this long (R1.3208).
const INSET_SHAKE: i32 = 4;
const INSET_SHAKE_TICKS: u32 = 12;
const INSET_FLASH_TICKS: u32 = 40;
/// Cues kept for the page.
const CUES_KEPT: usize = 24;
/// Breaks in one tick that sound as a collapse.
const COLLAPSE_MIN: u32 = 3;
/// The countdown drum beats each second from this many seconds left
/// (R1.3800).
const DRUM_FROM: u32 = 10;
/// ...and beats harder from this many (R1.3800).
const DRUM_HARD: u32 = 3;
/// Siege camera (R1.1454), in world units: margin round the castle, how far
/// behind a shot or the trebuchet the view starts, widest flight view, the
/// aim view's width range, and an empty castle's stand-in size.
const CAM_MARGIN: i64 = 3;
const CAM_BEHIND: i64 = 6;
const CAM_FLY_MAX_W: i64 = 44;
/// The widest a high shot opens the view to keep the ground in it.
const CAM_FLY_TALL_W: i64 = 260;
const CAM_AIM_MIN_W: i64 = 24;
const CAM_AIM_MAX_W: i64 = 44;
const CAM_EMPTY_W: i64 = 10;
const CAM_MIN_TALL: i64 = 4;
/// The settled castle fills at least this share (num, den) of the height.
const CAM_CASTLE_SHARE: (i64, i64) = (35, 100);
/// Ticks after release the view stays on the trebuchet (R1.1800).
pub(crate) const CAM_HOLD: u32 = 24;
/// Each tick the view moves this fraction (1/n) of the way to its target.
pub(crate) const CAM_EASE: i64 = 8;
/// Ticks the view rests on the castle after your shot lands, and the most
/// while debris still moves (R1.3204).
const CAM_LOOK: u32 = 90;
const CAM_LOOK_MAX: u32 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Beam(Piece),
    Royal,
    Building(BuildingKind),
}

const BEAMS: [(Piece, &str); 4] = [
    (Piece::Segment(ledger::Material::Wood), "wood"),
    (Piece::Segment(ledger::Material::Stone), "stone"),
    (Piece::Segment(ledger::Material::Metal), "metal"),
    (Piece::Rope, "rope"),
];
const BUILDINGS: [(BuildingKind, &str); 4] = [
    (BuildingKind::Carpenter, "carpenter"),
    (BuildingKind::Quarry, "quarry"),
    (BuildingKind::Armory, "armory"),
    (BuildingKind::Mine, "mine"),
];

/// The countdown drum's beat when a timed phase's seconds left went from
/// `was` to `now`: one beat for each new second in the last ten, a harder one
/// in the last three; none otherwise (R1.3800).
pub(crate) fn drum_beat(was: u32, now: u32) -> Option<&'static str> {
    if now == was || now == 0 || now > DRUM_FROM {
        return None;
    }
    Some(if now <= DRUM_HARD {
        "drum_close"
    } else {
        "drum_count"
    })
}

/// The building songs, one a round in turn through the upgrade and build
/// phases; the siege keeps its own (R1.3802).
pub(crate) const BUILD_SONGS: [&str; 5] = [
    "music_build",
    "music_jig",
    "music_pastoral",
    "music_ballad",
    "music_mandolins",
];

/// Round `round`'s building song (rounds count from 1).
pub(crate) fn build_song_for(round: u32) -> &'static str {
    BUILD_SONGS[round.saturating_sub(1) as usize % BUILD_SONGS.len()]
}

/// A royal's cry as it dies, by rank (R1.3801).
pub(crate) fn death_cry(k: RoyalKind) -> &'static str {
    match k {
        RoyalKind::Squire => "royal_squire",
        RoyalKind::Knight => "royal_knight",
        RoyalKind::Princess => "royal_princess",
        RoyalKind::Prince => "royal_prince",
        RoyalKind::Queen => "royal_queen",
        RoyalKind::King => "royal_king",
    }
}

fn rank_name(k: RoyalKind) -> &'static str {
    match k {
        RoyalKind::Squire => "squire",
        RoyalKind::Knight => "knight",
        RoyalKind::Princess => "princess",
        RoyalKind::Prince => "prince",
        RoyalKind::Queen => "queen",
        RoyalKind::King => "king",
    }
}

fn building_name(k: BuildingKind) -> &'static str {
    BUILDINGS
        .iter()
        .find(|(b, _)| *b == k)
        .map_or("armory", |(_, n)| n)
}

fn spot_name(k: SpotKind) -> &'static str {
    match k {
        SpotKind::StoneDeposit => "stone_deposit",
        SpotKind::OreDeposit => "ore_deposit",
        SpotKind::GoldVein => "gold_vein",
        SpotKind::HolyWell => "holy_well",
        SpotKind::Forest => "forest",
        SpotKind::Ruins => "ruins",
        SpotKind::Watchtower => "watchtower",
        SpotKind::ArmoryRuin => "armory_ruin",
    }
}

fn ammo_name(a: Ammo) -> &'static str {
    match a {
        Ammo::Normal => "normal",
        Ammo::Special(AmmoKind::Heavy) => "heavy",
        Ammo::Special(AmmoKind::Fire) => "fire",
        Ammo::Special(AmmoKind::Exploding) => "exploding",
        Ammo::Special(AmmoKind::Spread) => "spread",
        Ammo::Special(AmmoKind::Chain) => "chain",
        Ammo::Special(AmmoKind::Meteor) => "meteor",
    }
}

fn air_name(a: AirKind) -> &'static str {
    match a {
        AirKind::Drop => "drop",
        AirKind::Reverse => "reverse",
        AirKind::Phase => "phase",
        AirKind::Split => "split",
        AirKind::Dive => "dive",
        AirKind::Boost => "boost",
        AirKind::Bounce => "bounce",
    }
}

/// The phase clock turns urgent at this many seconds left.
const TIMER_URGENT_SECONDS: u32 = 10;

/// A top-bar value with its icon.
fn stat(id: &str, icon: &str, text: String, tip: String) -> Item {
    Item::Stat {
        id: id.into(),
        icon: hud_icon::url(icon),
        icon_name: icon.into(),
        text,
        tip,
    }
}

/// An upgrade card's icon, by its first effect.
fn card_icon(effects: &[crate::game::upgrades::Effect]) -> &'static str {
    use crate::game::upgrades::Effect as E;
    match effects.first() {
        Some(E::Ammo(k, _)) => ammo_name(Ammo::Special(*k)),
        Some(E::Air(k, _)) => air_name(*k),
        Some(E::AirUses(_)) => "boost",
        Some(
            E::Gold(_)
            | E::Stock(..)
            | E::GoldPerRound(_)
            | E::XpPerRound(_)
            | E::StockPerRound(..)
            | E::InterestCap(_)
            | E::InterestStep(_)
            | E::KillGold(_)
            | E::Discount(..),
        ) => "economy",
        Some(E::GainBuilding(_) | E::ProducerOutput(_) | E::ArmoryShots(_)) => "carpenter",
        Some(
            E::Strength(..)
            | E::JointStrength(_)
            | E::GroundedStrength(_)
            | E::RopeStrength(_)
            | E::RopesUncuttable
            | E::SlowBurn(_)
            | E::HitImmunity(_),
        ) => "defense",
        Some(
            E::RoyalSave(..)
            | E::FallLimit(_)
            | E::Decoys(_)
            | E::RoyalSpacing(_)
            | E::FirstDeathSpared(_)
            | E::RoyalDamageCut(_),
        ) => "royal",
        Some(
            E::ShotsPerRound(_)
            | E::ShotDiscount(_)
            | E::LuckyShots { .. }
            | E::ShotsPerKill { .. }
            | E::ShotsPerLoss { .. },
        ) => "buy_shot",
        _ => "siege",
    }
}

/// The hover tip's string key for a HUD button, by the button's action.
fn button_tip(id: &str) -> Option<&'static str> {
    Some(match id {
        "match.ready" => "tip.ready",
        "match.buy_xp" => "tip.buy_xp",
        "match.buy_shot" => "tip.buy_shot",
        "match.test" => "tip.test",
        "match.scout" => "tip.scout",
        "match.skip" => "tip.skip",
        "match.reroll" => "tip.reroll",
        "play.leave" => "tip.leave",
        _ => return None,
    })
}

fn tier_name(t: Tier) -> &'static str {
    match t {
        Tier::Silver => "silver",
        Tier::Gold => "gold",
        Tier::Prismatic => "prismatic",
    }
}

/// A physics point on the build grid (rounded to the nearest step).
fn grid_of(p: Vec2) -> GridPt {
    let g = TUNE.grid.0;
    let r = |v: i64| (v + g / 2).div_euclid(g) as i32;
    GridPt::new(r(p.x.0), r(p.y.0))
}

/// The hits on your castle, played back after a siege, with the
/// attacker's inputs when a person threw them (R1.1206).
#[derive(Clone, Debug)]
struct Replay {
    field: Field,
    inputs: Vec<Logged>,
    tick: u32,
    end: u32,
}

/// The upgrade cards' names and texts, from data/upgrades.json.
#[derive(Clone, Debug, Default)]
pub struct CardText {
    pub map: BTreeMap<String, (String, String)>,
}

impl CardText {
    pub fn parse(json: &str) -> Result<CardText, String> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let rows = v["upgrades"]
            .as_object()
            .ok_or("The upgrades file has no \"upgrades\".")?;
        let map = rows
            .iter()
            .map(|(k, r)| {
                let name = r["name"].as_str().unwrap_or(k).to_string();
                let text = r["text"].as_str().unwrap_or("").to_string();
                (k.clone(), (name, text))
            })
            .collect();
        Ok(CardText { map })
    }

    fn get(&self, key: &str) -> (String, String) {
        self.map
            .get(key)
            .cloned()
            .unwrap_or_else(|| (key.to_string(), String::new()))
    }
}

#[derive(Clone, Debug)]
pub struct Arena {
    pub play: Play,
    pub me: PlayerId,
    start: MatchStart,
    names: Vec<String>,
    fast: bool,
    pub tool: Tool,
    drag: Option<Vec2>,
    cursor: Option<Vec2>,
    size: (i32, i32),
    /// Inputs waiting for the next tick.
    pending: Vec<Input>,
    /// Strain colors on (the Test button).
    pub testing: bool,
    strain: Option<Vec<Fx>>,
    /// The Test's live run: a copy of your castle stepping each tick, and
    /// the ticks it has left (R1.4303).
    trial: Option<(CastleWorld, u32)>,
    /// Your castle as built, for drawing and placing (rebuilt on change).
    built: Option<CastleWorld>,
    /// The castle being scouted in the build phase, if not your own.
    pub scout: Option<PlayerId>,
    clock: Option<i64>,
    stepped: i64,
    prev_keys: u32,
    cues: Vec<Cue>,
    serial: u32,
    path: Vec<Vec2>,
    trail: Vec<Vec2>,
    replay: Option<Replay>,
    /// The last siege as it began and the people's siege inputs, for the
    /// replay.
    log: SiegeLog,
    /// Online: the room's session steps the match, and this is the session
    /// tick last followed.
    driven: Option<u32>,
    /// Online: the swing a Release was sent for, so it is sent once.
    auto_released: Option<u32>,
    seen_phase: Phase,
    seen_round: u32,
    /// Seconds left in the phase when last looked, for the countdown drum.
    seen_secs: u32,
    /// Royals killed so far in each field you hear, by (owner, attacker).
    seen_kills: Vec<((PlayerId, PlayerId), usize)>,
    /// The siege view as shown, easing toward `camera_target` each tick.
    eye: Option<Camera>,
    /// Your shot in the air and its view (R1.3204).
    flight: Option<Flight>,
    /// The siege tick your last shot landed.
    landed_at: Option<u32>,
    /// Your own castle's broken segments and dead bodies, and the siege
    /// tick that count last rose (R1.3208).
    own_counts: (u32, u32),
    own_hit_at: Option<u32>,
    /// "Need {n} gold", filled by the cursor when a segment costs more
    /// than you have (R1.4301)
    need_text: String,
}

/// Your shot in the air: its ball, the view's width for the flight,
/// whether it has started falling, and whether it has hit something.
#[derive(Clone, Copy, Debug)]
struct Flight {
    ball: usize,
    width: Fx,
    fell: bool,
    landed: bool,
}

/// Sixteenths of a tick from `basis_us` (when a tick began) to `press_us`,
/// at `ticks_per_s`, held to 0-31 (R1.2103).
pub fn release_part(press_us: i64, basis_us: i64, ticks_per_s: i64) -> u8 {
    let parts = crate::physics::RELEASE_PARTS as i64;
    ((press_us - basis_us) * ticks_per_s * parts / 1_000_000).clamp(0, 2 * parts - 1) as u8
}

impl Arena {
    /// A match from the lobby's start: bots in their seats, `client` is you.
    pub fn new(start: &MatchStart, client: &str, fast: bool, s: &Strings) -> Result<Arena, String> {
        let me = start.seats.iter().position(|st| st.person == client).unwrap_or(0) as PlayerId;
        let bots = start.seats.iter().filter(|st| st.bot.is_some()).count();
        let mut bot_names = super::bot_names::draw(&super::bot_names::all(), start.seed, bots).into_iter();
        let names = start
            .seats
            .iter()
            .enumerate()
            .map(|(i, st)| {
                if i as PlayerId == me {
                    s.get("seat.you")
                } else if st.bot.is_some() {
                    bot_names.next().unwrap_or_default()
                } else {
                    st.name.clone()
                }
            })
            .collect();
        let play = online::new_play(start).map_err(|e| format!("The match cannot start: {e:?}."))?;
        let seen_phase = play.rules.phase;
        let seen_round = play.rules.round;
        let mut a = Arena {
            play,
            me,
            start: start.clone(),
            names,
            fast,
            tool: Tool::Beam(BEAMS[0].0),
            drag: None,
            cursor: None,
            size: (1920, 1080),
            pending: Vec::new(),
            testing: false,
            strain: None,
            trial: None,
            built: None,
            scout: None,
            clock: None,
            stepped: 0,
            prev_keys: 0,
            cues: Vec::new(),
            serial: 0,
            path: Vec::new(),
            trail: Vec::new(),
            replay: None,
            log: SiegeLog::default(),
            driven: None,
            auto_released: None,
            seen_phase,
            seen_round,
            seen_secs: 0,
            seen_kills: Vec::new(),
            eye: None,
            flight: None,
            landed_at: None,
            own_counts: (0, 0),
            own_hit_at: None,
            need_text: s.get("build.need"),
        };
        a.rebuild();
        Ok(a)
    }

    /// An online match: `game` is the room's, stepped by its session.
    pub fn online(
        start: &MatchStart,
        client: &str,
        s: &Strings,
        game: &NetMatch,
        tick: u32,
    ) -> Result<Arena, String> {
        let mut a = Arena::new(start, client, false, s)?;
        a.play = game.play.clone();
        a.log = game.log.clone();
        a.driven = Some(tick);
        a.seen_phase = a.play.rules.phase;
        a.seen_round = a.play.rules.round;
        a.rebuild();
        Ok(a)
    }

    /// Moves the clock on by `ms` paused, so a solo match does not catch up
    /// the time spent in the pause menu.
    pub fn hold(&mut self, ms: i64) {
        if let Some(c) = &mut self.clock {
            *c += ms;
        }
    }

    /// Ticks run so far by this browser's clock.
    #[cfg(test)]
    pub(crate) fn ticks_run(&self) -> i64 {
        self.stepped
    }

    pub fn is_online(&self) -> bool {
        self.driven.is_some()
    }

    /// The same seats again on a new seed.
    pub fn again(&self, seed: u64, s: &Strings) -> Result<Arena, String> {
        let mut start = self.start.clone();
        start.seed = seed;
        let client = self
            .start
            .seats
            .get(self.me as usize)
            .map_or("", |st| st.person.as_str());
        Arena::new(&start, client, self.fast, s)
    }

    pub fn phase(&self) -> Phase {
        self.play.rules.phase
    }

    /// The phase this player sees: the build view once they pick their
    /// upgrade card, while the shared clock still runs the upgrade phase
    /// (R1.3400).
    pub fn view_phase(&self) -> Phase {
        let phase = self.phase();
        if phase == Phase::Upgrade && self.play.rules.builds_now(self.me) {
            Phase::Build
        } else {
            phase
        }
    }

    pub fn over(&self) -> bool {
        self.phase() == Phase::Results
    }

    fn cue(&mut self, name: &'static str, count: u32) {
        self.serial += 1;
        self.cues.push(Cue {
            n: self.serial,
            name,
            count,
        });
        if self.cues.len() > CUES_KEPT {
            self.cues.remove(0);
        }
    }

    fn level(&self, p: PlayerId) -> u8 {
        self.play.rules.player(p).map_or(1, |pl| pl.purse.level)
    }

    fn plot(&self, p: PlayerId) -> &Plot {
        &self.play.rules.players[p as usize].terrain
    }

    /// Your castle rebuilt, and its strain if the Test is on.
    fn rebuild(&mut self) {
        let pl = &self.play.rules.players[self.me as usize];
        self.built = Some(self.play.castles[self.me as usize].world(pl, true));
        self.strain = None;
        self.trial = None;
        if self.testing {
            self.measure();
        }
    }

    fn measure(&mut self) {
        let Some(cw) = &self.built else { return };
        let by_id: BTreeMap<u32, u32> = self.play.strain(self.me).into_iter().collect();
        let per_seg = cw
            .seg_ids
            .iter()
            .map(|id| {
                let pm = by_id.get(id).copied().unwrap_or(0) as i64;
                // 1000 is broken: just past the top bucket's edge
                Fx(pm * Fx::ONE.0 / 1000 + i64::from(pm >= 1000))
            })
            .collect();
        self.strain = Some(per_seg);
    }

    pub fn set_size(&mut self, value: &str) -> Result<(), String> {
        let (w, h) = pair(value)?;
        self.size = (w.clamp(320, 7680), h.clamp(240, 4320));
        Ok(())
    }

    // ---- where things are ----

    /// A player's buildable area in world units: left, right, top.
    fn zone(&self, p: PlayerId) -> (Fx, Fx, Fx) {
        let plot = self.plot(p);
        let level = self.level(p);
        let cols = plot.buildable(level);
        let front = crate::play::units::terrain_to_fx(plot.heights[cols.start]);
        (
            Fx::from_int(cols.start as i64),
            Fx::from_int(cols.end as i64),
            front + crate::play::units::terrain_to_fx(Plot::build_height(level)),
        )
    }

    fn ground(&self, p: PlayerId, x: Fx) -> Fx {
        crate::play::units::physics_terrain(self.plot(p)).height_at(x)
    }

    fn build_camera(&self, p: PlayerId, w: i32, h: i32) -> Camera {
        let (zl, zr, _) = self.zone(p);
        let margin = Fx::from_int(2);
        let mut cam = Camera::spanning(zl - margin, zr - zl + margin * 2, Fx::ZERO, w, h);
        cam.bottom = self.ground(p, zl) - cam.height() / 5;
        cam.kept_in(world_right(self.plot(p)))
    }

    /// The castle box of field `f`: left, right, ground and top, from its
    /// segments and royals (the build zone's front when nothing stands).
    fn castle_box(&self, f: &Field) -> (Fx, Fx, Fx, Fx) {
        let (zl, _, _) = self.zone(f.owner);
        let w = &f.castle.world;
        let xs = w
            .nodes
            .iter()
            .map(|n| n.pos)
            .chain(w.bodies.iter().filter(|b| b.alive).map(|b| b.pos));
        let (mut l, mut r, mut top) = (None::<Fx>, None::<Fx>, None::<Fx>);
        for p in xs {
            l = Some(l.map_or(p.x, |v| v.min(p.x)));
            r = Some(r.map_or(p.x, |v| v.max(p.x)));
            top = Some(top.map_or(p.y, |v| v.max(p.y)));
        }
        let l = l.unwrap_or(zl);
        let r = r.unwrap_or(zl + Fx::from_int(CAM_EMPTY_W));
        let ground = self.ground(f.owner, (l + r) / 2);
        let top = top.unwrap_or(ground).max(ground + Fx::from_int(CAM_MIN_TALL));
        (l, r, ground, top)
    }

    /// The corner window's view of your own field `f` (R1.3202): your
    /// castle's box, whole, with a margin, the ground a sixth up; with
    /// nothing built, the royals and the plot's front.
    fn inset_camera(&self, f: &Field) -> Camera {
        let (cl, cr, ground, top) = self.castle_box(f);
        let m = Fx::from_int(CAM_MARGIN);
        let tall = (top - ground + m) * 6 / 5;
        let width = (cr - cl + m * 2).max(tall * Fx::from_int(INSET_W as i64) / Fx::from_int(INSET_H as i64));
        let mut cam = Camera::spanning((cl + cr) / 2 - width / 2, width, Fx::ZERO, INSET_W, INSET_H);
        cam.bottom = ground - cam.height() / 6;
        cam.kept_in(world_right(&f.plot))
    }

    /// Where the siege view of field `f` wants to be (R1.1454): aiming, the
    /// trebuchet with the castle's front at the right edge; in flight, the
    /// shot followed and the castle brought in as it nears; settled, the
    /// castle filling over a third of the screen's height. `Arena::tick`
    /// eases the shown view toward it.
    fn siege_camera(&self, f: &Field, settled: bool, w: i32, h: i32) -> Camera {
        let (cl, cr, ground, top) = self.castle_box(f);
        let edge = world_right(&f.plot);
        let castle = Camera::framing(
            cl,
            cr,
            ground,
            top,
            Fx::from_int(CAM_MARGIN),
            CAM_CASTLE_SHARE,
            w,
            h,
        );
        let world = &f.castle.world;
        let flying = f
            .shooter
            .live
            .iter()
            .filter_map(|&i| world.projectiles.get(i))
            .find(|b| b.alive);
        let base = f.base;
        let floor = base.y.min(ground);
        let m = Fx::from_int(CAM_MARGIN);
        let home = base.x - Fx::from_int(CAM_BEHIND);
        let flying_w = self.flight.filter(|fl| !fl.landed).map(|fl| fl.width);
        if let (Some(b), Some(width)) = (flying, flying_w) {
            // followed from release (R1.3204): one width for the whole
            // flight, so the view never zooms in and out under the shot;
            // the shot two fifths across and in the upper middle, and the
            // pan stops with the castle in view, so it is there before impact
            let mut cam = Camera::spanning(home, width, Fx::ZERO, w, h);
            let tall = cam.height();
            cam.left = (b.pos.x - width * 2 / 5).max(home).min(cr + m - width);
            cam.bottom = (floor - tall / 6).max(b.pos.y - tall * 3 / 5);
            return cam.kept_in(edge);
        }
        if settled {
            return castle.kept_in(edge);
        }
        let left = base.x - Fx::from_int(CAM_BEHIND);
        let width = (cl + Fx::from_int(CAM_MARGIN) - left)
            .min(Fx::from_int(CAM_AIM_MAX_W))
            .max(Fx::from_int(CAM_AIM_MIN_W));
        let mut cam = Camera::spanning(left, width, Fx::ZERO, w, h);
        cam.bottom = floor - cam.height() / 6;
        cam.kept_in(edge)
    }

    /// The flight view's width for shot `b` just thrown at field `f`
    /// (R1.3204): from the trebuchet to past the castle, widened so the
    /// shot's highest point sits under the top quarter, at most the widest.
    fn flight_width(&self, f: &Field, b: &crate::physics::balls::Ball) -> Fx {
        let (w, h) = self.size;
        let (_, cr, ground, _) = self.castle_box(f);
        let floor = f.base.y.min(ground);
        let home = f.base.x - Fx::from_int(CAM_BEHIND);
        let across = (cr + Fx::from_int(CAM_MARGIN) - home)
            .min(Fx::from_int(CAM_FLY_MAX_W))
            .max(Fx::from_int(CAM_AIM_MIN_W));
        // rise under gravity alone: vy^2 / 2g, in raw units
        let vy = b.vel.y.0.max(0);
        let rise = Fx(vy * vy / (2 * crate::physics::tune::G_RAW));
        let tall = (b.pos.y + rise + Fx::from_int(CAM_MARGIN) - floor) * 12 / 7;
        across
            .max(tall * Fx::from_int(w as i64) / Fx::from_int(h.max(1) as i64))
            .min(Fx::from_int(CAM_FLY_TALL_W))
    }

    /// Whether the view is back on the trebuchet after a shot, so the next
    /// swing may start (R1.3205).
    fn camera_home(&self) -> bool {
        let Some(f) = self.my_field() else { return true };
        if self.flight.is_some_and(|fl| !fl.landed) || self.looking(f) {
            return false;
        }
        let (Some(eye), target) = (self.eye, self.camera_target()) else {
            return true;
        };
        let near = |a: Fx, b: Fx, tol: Fx| (a - b).abs() <= tol;
        near(eye.left, target.left, Fx::ONE) && near(eye.ppu, target.ppu, target.ppu / 16)
    }

    /// Whether the view still rests on the castle after your shot landed:
    /// a while, longer while debris still moves (R1.3204).
    fn looking(&self, f: &Field) -> bool {
        let (Some(at), Some(s)) = (self.landed_at, self.play.siege.as_ref()) else {
            return false;
        };
        let since = s.tick.saturating_sub(at);
        since < CAM_LOOK || (since < CAM_LOOK_MAX && f.active())
    }

    /// The field you shoot at this siege.
    fn my_field(&self) -> Option<&Field> {
        self.play
            .siege
            .as_ref()?
            .fields
            .iter()
            .find(|f| f.attacker == self.me && !f.shooter.bot)
    }

    fn own_field(&self) -> Option<&Field> {
        self.play
            .siege
            .as_ref()?
            .fields
            .iter()
            .find(|f| f.owner == self.me)
    }

    /// The castle the build view shows: yours, or the one being scouted.
    fn shown(&self) -> PlayerId {
        self.scout.unwrap_or(self.me)
    }

    /// The view the camera heads for this tick.
    fn camera_target(&self) -> Camera {
        let (w, h) = self.size;
        if let Some(r) = &self.replay {
            return self.build_camera(r.field.owner, w, h);
        }
        match (self.phase(), self.my_field()) {
            (Phase::Siege | Phase::SiegeOver, Some(f)) => {
                self.siege_camera(f, f.shooter.swing_from.is_none() && self.looking(f), w, h)
            }
            _ => self.build_camera(self.shown(), w, h),
        }
    }

    /// The right edge of the world the camera shows (R1.2200).
    fn view_right(&self) -> Fx {
        if let Some(r) = &self.replay {
            return world_right(self.plot(r.field.owner));
        }
        match (self.phase(), self.my_field()) {
            (Phase::Siege | Phase::SiegeOver, Some(f)) => world_right(&f.plot),
            _ => world_right(self.plot(self.shown())),
        }
    }

    /// The view shown: eased during a siege, exact otherwise.
    fn camera(&self) -> Camera {
        match (self.phase(), self.my_field(), self.eye) {
            (Phase::Siege | Phase::SiegeOver, Some(_), Some(eye)) if self.replay.is_none() => eye,
            _ => self.camera_target(),
        }
    }

    // ---- input ----

    /// A segment's start is placed and waits for its end: Esc cancels it
    /// instead of pausing.
    pub fn start_pending(&self) -> bool {
        self.drag.is_some()
    }

    fn queue(&mut self, input: Input) {
        self.pending.push(input);
    }

    /// One display frame: keys, then the match up to `now_ms`. `press_us`
    /// is when the latest key went down and `net_tick_us` when the online
    /// session's last tick began (both microseconds), so a release is timed
    /// within a tick (R1.2103).
    pub fn pump(&mut self, now_ms: i64, keys: u32, press_us: Option<i64>, net_tick_us: Option<i64>) {
        let pressed = keys & !self.prev_keys;
        self.prev_keys = keys;
        if pressed & KEY_SPACE != 0 {
            let part = self.press_part(press_us, net_tick_us);
            self.space(part);
        }
        for (k, (_, bit)) in KEY_DIGITS.iter().enumerate() {
            if pressed & bit != 0 {
                self.digit(k);
            }
        }
        for (k, (_, bit)) in KEY_AIR.iter().enumerate() {
            if pressed & bit != 0 && self.phase() == Phase::Siege {
                self.queue(Input::Air(k as u8));
            }
        }
        if pressed & KEY_CANCEL != 0 {
            self.drag = None;
        }
        if pressed & KEY_UNDO != 0 {
            self.undo();
        }
        if self.driven.is_some() {
            return;
        }
        let start = *self.clock.get_or_insert(now_ms);
        let speed = if self.fast { FAST_SPEED } else { 1 };
        let due = (now_ms - start) * TICKS * speed / 1000;
        let most = MAX_CATCH_UP * speed;
        if due - self.stepped > most {
            self.stepped = due - most;
        }
        while self.stepped < due {
            self.stepped += 1;
            self.tick();
        }
    }

    /// Sixteenths of a tick from the start of the last tick run to the key
    /// press: the next tick applies the press, so this times the release
    /// within it ([`Input::Release`]). On the tick when anything is unknown.
    fn press_part(&self, press_us: Option<i64>, net_tick_us: Option<i64>) -> u8 {
        let Some(press) = press_us else {
            return crate::play::ON_TICK;
        };
        if self.driven.is_some() {
            let tps = crate::ticks::TICKS_PER_SECOND;
            return net_tick_us.map_or(crate::play::ON_TICK, |b| release_part(press, b, tps));
        }
        let Some(start) = self.clock else {
            return crate::play::ON_TICK;
        };
        let speed = if self.fast { FAST_SPEED } else { 1 };
        let tps = TICKS * speed;
        let basis = start * 1000 + self.stepped * 1_000_000 / tps;
        release_part(press, basis, tps)
    }

    fn space(&mut self, part: u8) {
        if self.replay.is_some() {
            self.replay = None;
            return;
        }
        let Some(f) = self.my_field() else { return };
        let swinging = f.shooter.swing_from.is_some();
        // after the timer only a swing under way may still throw (R1.3203)
        if self.phase() != Phase::Siege && !(swinging && self.phase() == Phase::SiegeOver) {
            return;
        }
        if !swinging {
            // the next swing waits for the view to come back (R1.3205)
            if !self.camera_home() {
                return;
            }
            self.queue(Input::Swing);
        } else {
            self.queue(Input::Release(part));
        }
    }

    fn digit(&mut self, k: usize) {
        match self.view_phase() {
            Phase::Siege => self.queue(Input::SelectAmmo(k as u8)),
            Phase::Build => {
                if let Some((p, _)) = BEAMS.get(k) {
                    self.tool = Tool::Beam(*p);
                } else if k == BEAMS.len() {
                    self.tool = Tool::Royal;
                }
            }
            _ => {}
        }
    }

    /// The tick a held swing throws by itself.
    fn swing_end() -> u32 {
        crate::physics::SWING_END
    }

    fn counts(f: Option<&Field>) -> (u32, u32) {
        f.map_or((0, 0), |f| {
            let w = &f.castle.world;
            (
                w.segments.iter().filter(|s| s.broken).count() as u32,
                w.bodies.iter().filter(|b| !b.alive).count() as u32,
            )
        })
    }

    /// One match tick with the inputs waiting, and what follows from it.
    fn tick(&mut self) {
        if self.over() {
            return;
        }
        self.release_held_swing();
        self.ready_when_done();
        let inputs: Vec<(PlayerId, Input)> = self.pending.drain(..).map(|i| (self.me, i)).collect();
        let before = Arena::counts(self.my_field());
        let results = online::step(&mut self.play, &mut self.log, &inputs);
        let mut changed = false;
        for ((_, input), r) in inputs.iter().zip(results.iter()) {
            match (input, r) {
                (Input::Swing, Ok(())) => self.cue("trebuchet_creak", 0),
                (Input::Release(_), Ok(())) => {
                    self.cue("trebuchet_release", 0);
                    self.path.clear();
                }
                (Input::Air(_), Ok(())) => self.cue("air_drop", 0),
                (
                    Input::PlaceSegment { .. }
                    | Input::PlaceBuilding { .. }
                    | Input::PlaceRoyal { .. }
                    | Input::BuyXp,
                    Ok(()),
                ) => {
                    self.cue("place_segment", 0);
                    changed = true;
                    if self.tool == Tool::Royal && self.unplaced_royal().is_none() {
                        self.tool = Tool::Beam(BEAMS[0].0);
                    }
                }
                (Input::Sell(_), Ok(())) => {
                    self.cue("sell_segment", 0);
                    changed = true;
                }
                _ => {}
            }
        }
        self.after_ticks(before, changed, 1);
    }

    /// A held swing throws by itself at the bottom of the arm.
    fn release_held_swing(&mut self) {
        let (Some(f), Some(s)) = (self.my_field(), self.play.siege.as_ref()) else {
            return;
        };
        let Some(from) = f.shooter.swing_from else { return };
        if s.tick - from >= Arena::swing_end()
            && !self.pending.iter().any(|i| matches!(i, Input::Release(_)))
            && self.auto_released != Some(from)
        {
            self.pending.push(Input::Release(crate::play::ON_TICK));
            if self.driven.is_some() {
                self.auto_released = Some(from);
            }
        }
    }

    /// Ready by yourself in a siege once you can throw nothing more (no
    /// ammo, or the castle cleared), your last shot has stopped and the
    /// view is back (R1.3207).
    fn ready_when_done(&mut self) {
        if self.phase() != Phase::Siege || self.pending.contains(&Input::Ready) {
            return;
        }
        let Some(f) = self.my_field() else { return };
        let spent = f.shooter.ammo.iter().all(|a| a.1 == 0) || f.cleared();
        let still = !f.shot_moving();
        let ready = self.play.rules.player(self.me).is_some_and(|p| p.ready);
        if spent && !ready && still && self.camera_home() {
            self.pending.push(Input::Ready);
        }
    }

    /// Online: this browser's inputs for the room to send, in order.
    pub fn take_inputs(&mut self) -> Vec<Input> {
        if self.over() {
            self.pending.clear();
            return Vec::new();
        }
        self.release_held_swing();
        self.ready_when_done();
        std::mem::take(&mut self.pending)
    }

    /// Online: the room's match after its session ran up to `tick`.
    pub fn follow(&mut self, game: &NetMatch, tick: u32) {
        let Some(seen) = self.driven else { return };
        if tick <= seen {
            return;
        }
        self.driven = Some(tick);
        let shot = |f: Option<&Field>| {
            f.map(|f| {
                (
                    f.shooter.swing_from.is_some(),
                    f.shooter.fired,
                    f.shooter.air_used,
                )
            })
        };
        let before_shot = shot(self.my_field());
        let before = Arena::counts(self.my_field());
        let castle = self.play.castles[self.me as usize].clone();
        let level = self.level(self.me);
        self.play = game.play.clone();
        self.log = game.log.clone();
        if let (Some(b), Some(a)) = (before_shot, shot(self.my_field())) {
            if !b.0 && a.0 {
                self.cue("trebuchet_creak", 0);
            }
            if a.1 > b.1 {
                self.cue("trebuchet_release", 0);
                self.path.clear();
            }
            if !b.2 && a.2 {
                self.cue("air_drop", 0);
            }
        }
        let changed = castle != self.play.castles[self.me as usize] || level != self.level(self.me);
        if changed {
            self.cue("place_segment", 0);
        }
        self.after_ticks(before, changed, tick - seen);
    }

    /// What follows `ran` match ticks: cues, the trail, the replay, a new
    /// phase, and your castle rebuilt if it `changed`.
    fn after_ticks(&mut self, before: (u32, u32), mut changed: bool, ran: u32) {
        let own = Arena::counts(self.own_field());
        if own.0 + own.1 > self.own_counts.0 + self.own_counts.1 {
            self.own_hit_at = self.play.siege.as_ref().map(|s| s.tick);
        }
        self.own_counts = own;
        let after = Arena::counts(self.my_field());
        let breaks = after.0.saturating_sub(before.0);
        if breaks >= COLLAPSE_MIN {
            self.cue("collapse", breaks);
        } else if breaks > 0 {
            self.cue("impact_wood", 0);
        }
        self.cry_kills();
        self.follow_shot();
        if let Some(r) = &mut self.replay {
            for _ in 0..REPLAY_SPEED * ran {
                if r.tick >= r.end {
                    break;
                }
                for l in r.inputs.iter().filter(|l| l.tick == r.tick) {
                    online::replay_input(&mut r.field, l.input, r.tick);
                }
                r.field.run_plan(r.tick);
                r.field.step();
                r.tick += 1;
            }
            if r.tick >= r.end {
                self.replay = None;
            }
        }
        if let Some((cw, left)) = &mut self.trial {
            let n = ran.min(*left);
            for _ in 0..n {
                cw.world.step();
            }
            *left -= n;
            if *left == 0 || !self.building_now() {
                self.trial = None;
            }
        }
        let phase = self.view_phase();
        let round = self.play.rules.round;
        let moved = phase != self.seen_phase || round != self.seen_round;
        let secs = self.seconds_left();
        if !moved && matches!(phase, Phase::Upgrade | Phase::Build | Phase::Siege) {
            if let Some(beat) = drum_beat(self.seen_secs, secs) {
                self.cue(beat, 0);
            }
        }
        self.seen_secs = secs;
        if moved {
            self.entered(phase);
            changed = true;
        }
        if changed {
            self.rebuild();
        }
        self.move_eye(moved, ran);
    }

    /// Royals killed in the fields you hear (the castle you attack and your
    /// own), each new one counted once.
    fn kills_heard(&self) -> Vec<((PlayerId, PlayerId), Vec<RoyalKind>)> {
        let Some(s) = self.play.siege.as_ref() else {
            return Vec::new();
        };
        s.fields
            .iter()
            .filter(|f| (f.attacker == self.me && !f.shooter.bot) || (f.owner == self.me && !f.ghost))
            .map(|f| {
                let royals = &self.play.rules.players[f.owner as usize].royals;
                let kinds = f
                    .killed
                    .iter()
                    .filter_map(|&b| match f.castle.bodies.get(b) {
                        Some(BodyOwner::Royal(i)) => royals.get(*i).map(|r| r.kind),
                        _ => None,
                    })
                    .collect();
                ((f.owner, f.attacker), kinds)
            })
            .collect()
    }

    /// A death cry for each royal killed since last looked (R1.3801).
    fn cry_kills(&mut self) {
        let now = self.kills_heard();
        let mut cries = Vec::new();
        for (key, kinds) in &now {
            let seen = self
                .seen_kills
                .iter()
                .find(|(k, _)| k == key)
                .map_or(0, |(_, n)| *n);
            cries.extend(kinds.iter().skip(seen).map(|&k| death_cry(k)));
        }
        self.seen_kills = now.iter().map(|(k, v)| (*k, v.len())).collect();
        for c in cries {
            self.cue(c, 0);
        }
    }

    /// The shown view after `ran` ticks: eased toward `camera_target`, and
    /// snapped there when the phase just changed (R1.1700).
    fn move_eye(&mut self, snap: bool, ran: u32) {
        let target = self.camera_target();
        let mut eye = match self.eye {
            Some(e) if !snap && self.my_field().is_some() => e,
            _ => target,
        };
        for _ in 0..ran.min(CAM_EASE as u32 * 4) {
            eye = eye.ease_toward(target, CAM_EASE);
        }
        // the left edge and the zoom ease apart, so keep each step inside
        self.eye = Some(eye.kept_in(self.view_right()));
    }

    /// The path of your last shot, for the trail.
    fn follow_shot(&mut self) {
        let Some(f) = self.my_field() else { return };
        let w = &f.castle.world;
        let live = f
            .shooter
            .live
            .iter()
            .copied()
            .find(|&i| w.projectiles.get(i).is_some_and(|b| b.alive));
        let now = self.play.siege.as_ref().map_or(0, |s| s.tick);
        let Some(i) = live else {
            if self.flight.take().is_some_and(|fl| !fl.landed) {
                self.landed_at = Some(now);
            }
            if !self.path.is_empty() {
                self.trail = std::mem::take(&mut self.path);
            }
            return;
        };
        let b = &w.projectiles[i];
        let mut fl = match self.flight {
            Some(fl) if fl.ball == i => fl,
            _ => Flight {
                ball: i,
                width: self.flight_width(f, b),
                fell: false,
                landed: false,
            },
        };
        // the flight ends at the first hit: falling, then not (R1.3204)
        let hit = fl.fell && b.vel.y >= Fx::ZERO && !fl.landed;
        fl.landed |= hit;
        fl.fell |= b.vel.y < Fx::ZERO;
        let point = b.age.is_multiple_of(TRAIL_EVERY).then_some(b.pos);
        self.flight = Some(fl);
        if hit {
            self.landed_at = Some(now);
        }
        if let Some(p) = point {
            self.path.push(p);
        }
    }

    /// A new phase began.
    fn entered(&mut self, phase: Phase) {
        let was = self.seen_phase;
        self.seen_phase = phase;
        self.seen_round = self.play.rules.round;
        self.drag = None;
        match phase {
            Phase::Siege => {
                // a replay still running from the last siege would hold the
                // view on its castle and eat the first Space (R1.3200)
                self.replay = None;
                self.flight = None;
                self.landed_at = None;
                self.own_counts = (0, 0);
                self.own_hit_at = None;
                self.scout = None;
                self.trail.clear();
                self.path.clear();
                self.cue("horn_siege", 0);
            }
            Phase::Build | Phase::Upgrade | Phase::Results => {
                if matches!(was, Phase::Siege | Phase::SiegeOver) {
                    if let Some((field, inputs)) = self.log.replay_of(self.me) {
                        self.replay = Some(Replay {
                            field,
                            inputs,
                            tick: 0,
                            end: REPLAY_MAX_TICKS,
                        });
                    }
                }
                if phase == Phase::Build {
                    self.cue("horn_build", 0);
                    let unplaced = self.unplaced_royal().is_some();
                    if unplaced {
                        self.tool = Tool::Royal;
                    }
                }
            }
            _ => {}
        }
    }

    /// This round's building song.
    fn build_song(&self) -> &'static str {
        build_song_for(self.play.rules.round)
    }

    /// The first royal of yours not yet placed.
    fn unplaced_royal(&self) -> Option<usize> {
        let pl = self.play.rules.player(self.me)?;
        let c = &self.play.castles[self.me as usize];
        (0..pl.royals.len())
            .find(|&i| pl.royals[i].spot.is_none() && c.royal_at.get(i).copied().flatten().is_none())
    }

    // ---- build actions ----

    fn building_now(&self) -> bool {
        self.view_phase() == Phase::Build && self.scout.is_none() && self.replay.is_none()
    }

    fn built_world(&self) -> Option<&World> {
        self.built.as_ref().map(|c| &c.world)
    }

    fn is_node(&self, p: Vec2) -> bool {
        self.built_world()
            .is_some_and(|w| w.nodes.iter().any(|n| n.pos == p))
    }

    fn in_zone(&self, p: Vec2) -> bool {
        let (l, r, top) = self.zone(self.me);
        let g = self.ground(self.me, p.x);
        p.x >= l && p.x <= r && p.y <= top && p.y >= g - TUNE.grid / 2
    }

    fn can_start(&self, p: Vec2) -> bool {
        self.in_zone(p) && (self.is_node(p) || p.y <= self.ground(self.me, p.x) + TUNE.grid / 2)
    }

    /// The placed node nearest `p` within `NODE_PULL`.
    fn node_near(&self, p: Vec2) -> Option<Vec2> {
        let w = self.built_world()?;
        let mut best: Option<(Fx, Vec2)> = None;
        for n in &w.nodes {
            let d = (n.pos - p).len();
            if d <= crate::play::tune::NODE_PULL && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, n.pos));
            }
        }
        best.map(|(_, n)| n)
    }

    /// The ground under `x`, on the physics grid.
    fn ground_pt(&self, x: Fx) -> Vec2 {
        Vec2::new(x, snap(self.ground(self.me, x)))
    }

    /// Where a press at `c` starts a segment: the placed node within
    /// `NODE_PULL`, else the nearest point on the player's grid, or the
    /// ground in that column when the ground is nearer (R1.4300).
    fn start_at(&self, c: Vec2) -> Vec2 {
        if let Some(n) = self.node_near(c) {
            return n;
        }
        let step = crate::play::tune::SNAP_STEP;
        let s = Vec2::new(snap_to(c.x, step), snap_to(c.y, step));
        let g = self.ground_pt(s.x);
        if (c.y - g.y).abs() <= (c.y - s.y).abs() {
            g
        } else {
            s
        }
    }

    /// Where a segment from `a` toward the cursor at `c` ends: a placed
    /// node or the ground near the cursor when within reach, else
    /// `reach_end` on the player's grid (R1.4300).
    fn end_at(&self, a: Vec2, c: Vec2) -> Vec2 {
        let max = self.max_len();
        let fits = |p: Vec2| p != a && (p - a).len() <= max;
        if let Some(n) = self.node_near(c).filter(|&n| fits(n)) {
            return n;
        }
        let e = reach_end(a, c, max, crate::play::tune::SNAP_STEP);
        let g = self.ground_pt(e.x);
        let near = (c - a).len() <= max + crate::play::tune::SNAP_STEP;
        if near && fits(g) && (c.y - g.y).abs() <= (c.y - e.y).abs() {
            g
        } else {
            e
        }
    }

    fn max_len(&self) -> Fx {
        match self.tool {
            Tool::Beam(piece) => {
                castle::physics_material(piece).map_or(Fx::ZERO, |m| TUNE.material(m).max_len)
            }
            _ => Fx::ZERO,
        }
    }

    /// Where a royal or a building would stand for the cursor at `c`: on
    /// the floor or ground nearest it in that column of the grid.
    fn stand_point(&self, c: Vec2) -> Option<GridPt> {
        let w = self.built_world()?;
        let x = snap(c.x);
        let (l, r, _) = self.zone(self.me);
        if x < l || x > r {
            return None;
        }
        // the nearest surface at or under the cursor (a little above counts),
        // else the nearest above it, so a royal always lands on something
        let mut best: Option<(bool, Fx, Fx)> = None;
        let mut consider = |y: Fx| {
            let above = y > c.y + Fx::ONE;
            let d = (y - c.y).abs();
            if best.is_none_or(|(ba, bd, _)| (above, d) < (ba, bd)) {
                best = Some((above, d, y));
            }
        };
        consider(self.ground(self.me, x));
        for s in &w.segments {
            if s.broken || TUNE.material(s.material).rope {
                continue;
            }
            let (pa, pb) = (w.nodes[s.a].pos, w.nodes[s.b].pos);
            let d = pb - pa;
            if d.x == Fx::ZERO || d.x.abs() < d.y.abs() {
                continue;
            }
            let u = (x - pa.x) / d.x;
            if u < Fx::ZERO || u > Fx::ONE {
                continue;
            }
            consider(pa.y + d.y * u);
        }
        best.map(|(_, _, y)| grid_of(Vec2::new(x, y)))
    }

    /// Whether royal `index` may stand at `at` (the rules' spacing check).
    fn royal_ok(&self, index: usize, at: GridPt) -> bool {
        let Some(pl) = self.play.rules.player(self.me) else {
            return false;
        };
        let level = pl.purse.level;
        if !castle::point_fits(&pl.terrain, level, at) {
            return false;
        }
        let spot = castle::rule_spot(&pl.terrain, level, at);
        crate::game::royals::valid_spot(&pl.royals, index, spot, castle::rule_plot(&pl.terrain, level))
            && self.play.royal_spaced(self.me, index, at)
    }

    /// The ledger id of the piece under `p`: a building, else the nearest
    /// segment within a grid step.
    fn piece_near(&self, p: Vec2) -> Option<u32> {
        let cw = self.built.as_ref()?;
        let w = &cw.world;
        for (i, b) in w.bodies.iter().enumerate() {
            if let Some(BodyOwner::Building(id)) = cw.bodies.get(i) {
                if (b.pos - p).len() <= b.radius {
                    return Some(*id);
                }
            }
        }
        let mut best: Option<(u32, Fx)> = None;
        for (i, s) in w.segments.iter().enumerate() {
            let (a, b) = (w.nodes[s.a].pos, w.nodes[s.b].pos);
            let d = b - a;
            let l2 = d.len2();
            if l2 == Fx::ZERO {
                continue;
            }
            let u = ((p - a).dot(d) / l2).clamp_to(Fx::ZERO, Fx::ONE);
            let dist = (p - (a + d * u)).len();
            if dist <= TUNE.grid && best.is_none_or(|(_, bd)| dist < bd) {
                best = Some((cw.seg_ids[i], dist));
            }
        }
        best.map(|(id, _)| id)
    }

    /// A press on the field: `x,y,button` in pixels. A segment takes two
    /// presses, its start then its end, and the next starts where it ended;
    /// button 2 lets go of a start, else sells the piece under it.
    pub fn down(&mut self, value: &str) -> Result<(), String> {
        let mut parts = value.split(',');
        let x = num(parts.next())?;
        let y = num(parts.next())?;
        let button = num(parts.next()).unwrap_or(0);
        if !self.building_now() {
            return Ok(());
        }
        let p = self.camera().world(x, y);
        self.cursor = Some(p);
        if button == 2 {
            if self.drag.take().is_none() {
                if let Some(id) = self.piece_near(p) {
                    self.queue(Input::Sell(id));
                }
            }
            return Ok(());
        }
        match self.tool {
            Tool::Beam(piece) => match self.drag {
                None => {
                    let s = self.start_at(p);
                    if self.can_start(s) {
                        self.drag = Some(s);
                    }
                }
                Some(a) => {
                    let b = self.end_at(a, p);
                    if a == b {
                        self.drag = None;
                    } else if self.segment_ok(a, b) {
                        self.queue(Input::PlaceSegment {
                            piece,
                            a: grid_of(a),
                            b: grid_of(b),
                        });
                        self.drag = Some(b);
                    }
                }
            },
            Tool::Royal => {
                if let (Some(index), Some(at)) = (self.unplaced_royal(), self.stand_point(p)) {
                    self.queue(Input::PlaceRoyal {
                        index: index as u8,
                        at,
                    });
                }
            }
            Tool::Building(kind) => {
                if let Some(at) = self.stand_point(p) {
                    self.queue(Input::PlaceBuilding { kind, at });
                }
            }
        }
        Ok(())
    }

    /// Whether a segment from `a` to `b` may be placed: both ends in the
    /// zone and not through a royal.
    fn segment_ok(&self, a: Vec2, b: Vec2) -> bool {
        if a == b || !self.in_zone(a) || !self.in_zone(b) {
            return false;
        }
        let (Some(w), Some(pl)) = (self.built_world(), self.play.rules.player(self.me)) else {
            return false;
        };
        !self.play.castles[self.me as usize]
            .royal_feet(w, pl)
            .into_iter()
            .any(|(_, f)| castle::beam_hits_royal(a, b, f))
    }

    /// Ready as sent, past the button's wait for every royal (tests of
    /// the online wire).
    #[cfg(test)]
    pub(crate) fn ready_now(&mut self) {
        self.queue(Input::Ready);
    }

    /// Takes back the newest piece placed this build, for a full refund.
    fn undo(&mut self) {
        if !self.building_now() {
            return;
        }
        if let Some(id) = self.play.last_placed(self.me) {
            self.drag = None;
            self.queue(Input::Sell(id));
        }
    }

    pub fn moved(&mut self, value: &str) -> Result<(), String> {
        let (x, y) = pair(value)?;
        self.cursor = Some(self.camera().world(x, y));
        Ok(())
    }

    pub fn up(&mut self, value: &str) -> Result<(), String> {
        let (x, y) = pair(value)?;
        self.cursor = Some(self.camera().world(x, y));
        Ok(())
    }

    /// A button or bar press on the match screen.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name {
            "match.tool" => {
                self.tool = match value {
                    "royal" => Tool::Royal,
                    v => {
                        if let Some((p, _)) = BEAMS.iter().find(|(_, n)| *n == v) {
                            Tool::Beam(*p)
                        } else if let Some((b, _)) = BUILDINGS.iter().find(|(_, n)| *n == v) {
                            Tool::Building(*b)
                        } else {
                            return Err(format!("\"{v}\" is not a tool."));
                        }
                    }
                };
            }
            "match.ammo" => {
                let k: u8 = value.parse().map_err(|_| format!("\"{value}\" is not a slot."))?;
                self.queue(Input::SelectAmmo(k));
            }
            "match.air" => {
                let k: u8 = value.parse().map_err(|_| format!("\"{value}\" is not a slot."))?;
                self.queue(Input::Air(k));
            }
            "match.ready" => {
                if !(self.phase() == Phase::Build && self.unplaced_royal().is_some()) {
                    self.queue(Input::Ready);
                }
            }
            "match.undo" => self.undo(),
            "match.buy_xp" => self.queue(Input::BuyXp),
            "match.buy_shot" => self.queue(Input::BuyShot),
            "match.reroll" => self.queue(Input::Reroll),
            "match.pick" => {
                let k: u8 = value.parse().map_err(|_| format!("\"{value}\" is not a card."))?;
                self.queue(Input::Pick(k));
            }
            "match.test" => {
                self.testing = !self.testing;
                self.rebuild();
                if self.testing {
                    let pl = &self.play.rules.players[self.me as usize];
                    let cw = self.play.castles[self.me as usize].world(pl, true);
                    self.trial = Some((cw, crate::play::tune::TEST_RUN_TICKS));
                }
            }
            "match.scout" => self.next_scout(),
            "match.skip" => self.replay = None,
            _ => return Err(format!("Unknown action \"{name}\".")),
        }
        Ok(())
    }

    /// Scout: your castle, then each other player still in, then yours again.
    fn next_scout(&mut self) {
        let alive = self.play.rules.alive();
        let others: Vec<PlayerId> = alive.into_iter().filter(|&p| p != self.me).collect();
        self.scout = match self.scout {
            None => others.first().copied(),
            Some(cur) => others
                .iter()
                .position(|&p| p == cur)
                .and_then(|i| others.get(i + 1).copied()),
        };
        self.drag = None;
    }

    // ---- what the page draws ----

    /// The figures of a castle world: royals by rank, buildings by kind.
    fn figures(
        &self,
        world: &World,
        bodies: &[BodyOwner],
        owner: PlayerId,
        cam: &Camera,
        treb: Vec2,
    ) -> Vec<Figure> {
        let treb_x = cam.px(treb)[0];
        let pl = &self.play.rules.players[owner as usize];
        world
            .bodies
            .iter()
            .zip(bodies.iter())
            .map(|(b, who)| {
                let p = cam.px(b.pos);
                let rank = match *who {
                    BodyOwner::Royal(i) => pl.royals.get(i).map_or("squire", |r| rank_name(r.kind)),
                    BodyOwner::Building(id) => pl
                        .ledger
                        .entries
                        .iter()
                        .find(|e| e.id == id)
                        .and_then(|e| match e.piece {
                            Piece::Building(k) => Some(building_name(k)),
                            _ => None,
                        })
                        .unwrap_or("armory"),
                };
                Figure {
                    x: p[0],
                    y: p[1],
                    r: cam.len(b.radius, 3),
                    tall: cam.figure_tall(),
                    rank,
                    dead: !b.alive,
                    face: facing(p[0], treb_x),
                    turn: body_turn(b, world.tick, facing(p[0], treb_x)),
                }
            })
            .collect()
    }

    fn spot_marks(&self, p: PlayerId, cam: &Camera) -> Vec<SpotMark> {
        let plot = self.plot(p);
        let w = cam.len(Fx::ratio(1, 3), 4);
        plot.spots
            .iter()
            .map(|s| {
                let x0 = Fx::from_int(s.cols.start as i64);
                let x1 = Fx::from_int(s.cols.end as i64);
                // a band just under the ground line, so it never reads as a beam
                let down = |q: [i32; 2]| [q[0], q[1] + w];
                SpotMark {
                    a: down(cam.px(Vec2::new(x0, self.ground(p, x0)))),
                    b: down(cam.px(Vec2::new(x1, self.ground(p, x1)))),
                    w,
                    kind: spot_name(s.kind),
                }
            })
            .collect()
    }

    fn engine(&self, base: Vec2, phase: Arm, loaded: Option<(Fx, bool)>, cam: &Camera) -> Engine {
        Engine::of(&pose(base, phase, false), loaded, cam)
    }

    fn flag(&self) -> Flag {
        let (w, _) = self.size;
        let wind = self.play.rules.wind;
        let x = w - 90;
        let top = [x, 70];
        let pole = [top, [x, 160]];
        let len = 24 + 22 * wind.abs();
        let cloth = if wind == 0 {
            vec![top, [x + 10, 110], [x, 110]]
        } else {
            vec![top, [x + wind.signum() * len, 82], [x, 94]]
        };
        Flag { pole, cloth }
    }

    /// A siege field as drawn, at a size: the shooter's trebuchet and shots.
    fn field_frame(&self, f: &Field, now: u32, cam: Camera, mine: bool) -> Frame {
        let plot = &f.plot;
        let (ground, water) = ground_line(plot, &cam);
        let world = &f.castle.world;
        let (beams, nodes, shots, debris) = world_parts(world, &cam, None, false);
        let royals = self.figures(world, &f.castle.bodies, f.owner, &cam, f.base);
        let (puffs, shake) = impact_marks(world, &cam);
        let s = &f.shooter;
        let phase = match (s.swing_from, s.thrown) {
            (Some(t), _) => Arm::Swing(now.saturating_sub(t)),
            (None, Some((at, when))) => Arm::After {
                at,
                since: now.saturating_sub(when),
            },
            (None, None) => Arm::Rest,
        };
        let loaded = (s.swing_from.is_some()
            || s.live
                .iter()
                .all(|&i| !world.projectiles.get(i).is_some_and(|b| b.alive)))
        .then(|| s.ammo.get(s.selected as usize).filter(|a| a.1 > 0))
        .flatten()
        .map(|(a, _)| {
            let t = crate::physics::ammo::shot_tune(a.kind());
            (t.radius, t.ignites)
        });
        let mut loops = Vec::new();
        if mine && shots.iter().len() > 0 {
            loops.push("shot_whoosh");
        }
        if world.segments.iter().any(|s| !s.broken && s.burning()) {
            loops.push("fire_crackle");
        }
        Frame {
            mode: "siege",
            size: [cam.w, cam.h],
            cloth: cloth_at(&cam),
            ground,
            water,
            trees: tree_marks(plot, &cam),
            zone: None,
            grid: 0,
            beams,
            nodes,
            royals,
            shots,
            debris,
            engine: self.engine(f.base, phase, loaded, &cam),
            trail: if mine {
                self.trail.iter().map(|&p| cam.px(p)).collect()
            } else {
                Vec::new()
            },
            preview: None,
            ring: None,
            flag: None,
            spots: Vec::new(),
            inset: None,
            cues: Vec::new(),
            music: "music_siege",
            loops,
            puffs,
            shake: if mine { shake } else { None },
            build: None,
        }
    }

    pub fn frame(&self) -> Frame {
        let (w, h) = self.size;
        let cam = self.camera();
        if let Some(r) = &self.replay {
            let mut f = self.field_frame(&r.field, r.tick, cam, false);
            f.cues = self.cues.clone();
            f.music = self.build_song();
            f.flag = Some(self.flag());
            return f;
        }
        let now = self.play.siege.as_ref().map_or(0, |s| s.tick);
        if let (Phase::Siege | Phase::SiegeOver, Some(mine)) = (self.phase(), self.my_field()) {
            let mut f = self.field_frame(mine, now, cam, true);
            f.flag = Some(self.flag());
            f.cues = self.cues.clone();
            if let Some(own) = self.own_field() {
                let icam = self.inset_camera(own);
                let mut inner = self.field_frame(own, now, icam, false);
                inner.loops.clear();
                // a hit on your castle flashes the frame and shakes it
                let since = self.own_hit_at.map_or(u32::MAX, |t| now.saturating_sub(t));
                let shake = if since < INSET_SHAKE_TICKS {
                    [-INSET_SHAKE, INSET_SHAKE][(since % 2) as usize]
                } else {
                    0
                };
                f.inset = Some(Box::new(Inset {
                    at: [w - INSET_W - INSET_GAP + shake, INSET_TOP],
                    frame: inner,
                    hit: since < INSET_FLASH_TICKS,
                }));
            }
            return f;
        }
        // build, upgrade, results, or a siege you sit out: a castle at rest
        let p = self.shown();
        let building = self.building_now();
        let owned_world;
        let (world, bodies): (&World, &[BodyOwner]) = if p == self.me {
            match self.trial.as_ref().map(|t| &t.0).or(self.built.as_ref()) {
                Some(cw) => (&cw.world, &cw.bodies),
                None => return self.empty_frame(),
            }
        } else {
            let pl = &self.play.rules.players[p as usize];
            owned_world = self.play.castles[p as usize].world(pl, true);
            (&owned_world.world, &owned_world.bodies)
        };
        let plot = self.plot(p);
        let (ground, water) = ground_line(plot, &cam);
        let strain = if building && self.testing && self.trial.is_none() {
            self.strain.as_deref()
        } else {
            None
        };
        let (beams, nodes, shots, debris) = world_parts(world, &cam, strain, building);
        let base = crate::play::units::trebuchet_base(plot);
        let royals = self.figures(world, bodies, p, &cam, base);
        let zone = building.then(|| {
            let (l, r, top) = self.zone(p);
            let lt = cam.px(Vec2::new(l, top));
            let rb = cam.px(Vec2::new(r, self.ground(p, l)));
            [lt[0], lt[1], rb[0], rb[1]]
        });
        let (preview, ring) = if building {
            self.build_marks(&cam)
        } else {
            (None, None)
        };
        Frame {
            mode: "build",
            size: [w, h],
            cloth: cloth_at(&cam),
            ground,
            water,
            trees: tree_marks(plot, &cam),
            zone,
            grid: if building {
                cam.len(crate::play::tune::SNAP_STEP, 1)
            } else {
                0
            },
            beams,
            nodes,
            royals,
            shots,
            debris,
            engine: self.engine(base, Arm::Rest, None, &cam),
            trail: Vec::new(),
            preview,
            ring,
            flag: Some(self.flag()),
            spots: self.spot_marks(p, &cam),
            inset: None,
            cues: self.cues.clone(),
            music: self.build_song(),
            loops: Vec::new(),
            puffs: Vec::new(),
            shake: None,
            build: building.then(|| self.marks_over(&cam)),
        }
    }

    /// The zone's outline, the royal spacing circles and the royal about to
    /// be placed.
    fn marks_over(&self, cam: &Camera) -> BuildMarks {
        let (l, r, top) = self.zone(self.me);
        let mut edge = vec![cam.px(Vec2::new(l, top)), cam.px(Vec2::new(r, top))];
        let (c0, c1) = (l.floor_int(), r.floor_int());
        let mut c = c1;
        while c >= c0 {
            let x = Fx::from_int(c);
            edge.push(cam.px(Vec2::new(x, self.ground(self.me, x))));
            c -= 1;
        }
        edge.push(cam.px(Vec2::new(l, top)));
        let mut marks = BuildMarks {
            zone_edge: edge,
            node_r: cam.len(crate::play::tune::NODE_PULL, 4),
            ..BuildMarks::default()
        };
        if let (Tool::Beam(_), Some(c)) = (self.tool, self.cursor) {
            self.node_marks(cam, c, &mut marks);
        }
        if self.tool != Tool::Royal {
            return marks;
        }
        let Some(index) = self.unplaced_royal() else {
            return marks;
        };
        let (Some(w), Some(pl)) = (self.built_world(), self.play.rules.player(self.me)) else {
            return marks;
        };
        let spacing = Fx::from_int(crate::game::royals::spacing_cells() as i64);
        for (_, f) in self.play.castles[self.me as usize].royal_feet(w, pl) {
            let p = cam.px(f);
            marks.keep_out.push(Ring {
                x: p[0],
                y: p[1],
                r: cam.len(spacing, 4),
                ok: false,
            });
        }
        if let Some(at) = self.cursor.and_then(|c| self.stand_point(c)) {
            let radius = crate::play::tune::ROYAL_RADIUS;
            let p = cam.px(at.to_fx() + Vec2::new(Fx::ZERO, radius));
            marks.ghost = Some(Figure {
                x: p[0],
                y: p[1],
                r: cam.len(radius, 3),
                tall: cam.figure_tall(),
                rank: pl.royals.get(index).map_or("squire", |r| rank_name(r.kind)),
                dead: false,
                face: facing(
                    p[0],
                    cam.px(crate::play::units::trebuchet_base(self.plot(self.me)))[0],
                ),
                turn: None,
            });
        }
        marks
    }

    /// The placed nodes near the cursor at `c`, the one a press would
    /// pick, and the gold you lack for the selected material (R1.4300,
    /// R1.4301).
    fn node_marks(&self, cam: &Camera, c: Vec2, marks: &mut BuildMarks) {
        if let Some(w) = self.built_world() {
            for n in &w.nodes {
                if (n.pos - c).len() <= crate::play::tune::NODE_NEAR && self.in_zone(n.pos) {
                    marks.near.push(cam.px(n.pos));
                }
            }
        }
        let pick = match self.drag {
            None => Some(self.start_at(c)),
            Some(a) => Some(self.end_at(a, c)).filter(|&b| b != a),
        };
        marks.hover = pick.filter(|&p| self.is_node(p)).map(|p| cam.px(p));
        let short = self.gold_short();
        if short > 0 {
            let p = cam.px(c);
            marks.need = Some(Label {
                x: p[0],
                y: p[1] - cam.len(Fx::ONE, 24),
                text: self.need_text.replace("{n}", &short.to_string()),
            });
        }
    }

    /// How much more gold the selected material's next segment costs than
    /// you have: 0 when it is stocked, affordable or no segment is chosen.
    fn gold_short(&self) -> u32 {
        let Tool::Beam(piece) = self.tool else {
            return 0;
        };
        let Some(me) = self.play.rules.player(self.me) else {
            return 0;
        };
        let stock = me.ledger.stock;
        let stocked = match piece {
            Piece::Segment(ledger::Material::Wood) => stock.wood,
            Piece::Segment(ledger::Material::Stone) => stock.stone,
            Piece::Segment(ledger::Material::Metal) => stock.metal,
            _ => 0,
        };
        if stocked > 0 {
            return 0;
        }
        ledger::price(piece, &me.mods).saturating_sub(me.purse.gold)
    }

    fn empty_frame(&self) -> Frame {
        let cam = self.build_camera(self.me, self.size.0, self.size.1);
        let (ground, water) = ground_line(self.plot(self.me), &cam);
        Frame {
            mode: "build",
            size: [self.size.0, self.size.1],
            cloth: cloth_at(&cam),
            ground,
            water,
            trees: tree_marks(self.plot(self.me), &cam),
            zone: None,
            grid: 0,
            beams: Vec::new(),
            nodes: Vec::new(),
            royals: Vec::new(),
            shots: Vec::new(),
            debris: Vec::new(),
            engine: self.engine(Vec2::new(Fx::ZERO, Fx::ZERO), Arm::Rest, None, &cam),
            trail: Vec::new(),
            preview: None,
            ring: None,
            flag: None,
            spots: Vec::new(),
            inset: None,
            cues: self.cues.clone(),
            music: self.build_song(),
            loops: Vec::new(),
            puffs: Vec::new(),
            shake: None,
            build: None,
        }
    }

    fn build_marks(&self, cam: &Camera) -> (Option<Preview>, Option<Ring>) {
        let Some(c) = self.cursor else {
            return (None, None);
        };
        match self.tool {
            Tool::Beam(piece) => {
                let m = castle::physics_material(piece).unwrap_or(Material::Wood);
                let broke = self.gold_short() > 0;
                let Some(a) = self.drag else {
                    let s = self.start_at(c);
                    let p = cam.px(s);
                    return (
                        None,
                        Some(Ring {
                            x: p[0],
                            y: p[1],
                            r: cam.len(crate::play::tune::SNAP_STEP / 2, 3),
                            ok: self.can_start(s) && !broke,
                        }),
                    );
                };
                let b = self.end_at(a, c);
                let pa = cam.px(a);
                let preview = Preview {
                    a: pa,
                    b: cam.px(b),
                    w: cam.len(TUNE.material(m).thickness, 2),
                    material: material_name(m),
                    ok: a != b && self.in_zone(b) && !broke,
                    len: 0,
                }
                .measured();
                let ring = Ring {
                    x: pa[0],
                    y: pa[1],
                    r: cam.len(TUNE.material(m).max_len, 4),
                    ok: true,
                };
                (Some(preview), Some(ring))
            }
            Tool::Royal => {
                // a ring at the ghost's feet: whether a press places it there
                let Some(index) = self.unplaced_royal() else {
                    return (None, None);
                };
                let at = self.stand_point(c);
                let p = cam.px(at.map_or(c, |g| g.to_fx()));
                let ok = at.is_some_and(|g| self.royal_ok(index, g));
                (
                    None,
                    Some(Ring {
                        x: p[0],
                        y: p[1],
                        r: cam.len(crate::play::tune::ROYAL_RADIUS, 3),
                        ok,
                    }),
                )
            }
            Tool::Building(_) => {
                let at = self.stand_point(c);
                let p = cam.px(at.map_or(c, |g| g.to_fx()));
                (
                    None,
                    Some(Ring {
                        x: p[0],
                        y: p[1],
                        r: cam.len(crate::play::tune::BUILDING_RADIUS, 4),
                        ok: at.is_some(),
                    }),
                )
            }
        }
    }

    // ---- the HUD ----

    fn seconds_left(&self) -> u32 {
        let t = self.play.rules.ticks_left;
        t.div_ceil(crate::game::tune::TICKS_PER_SECOND)
    }

    pub fn panels(&self, s: &Strings, cards: &CardText) -> Vec<Panel> {
        if self.over() {
            return self.results_panels(s);
        }
        let button = |id: &str, text: String, icon: &str, disabled: bool| Item::Button {
            id: id.into(),
            text,
            quiet: false,
            disabled,
            tip: String::new(),
            icon: hud_icon::url(icon),
            cost: String::new(),
        };
        let rules = &self.play.rules;
        let Some(me) = rules.player(self.me) else {
            return Vec::new();
        };
        let phase = self.view_phase();
        let phase_key = match self.phase() {
            Phase::Upgrade => "phase.upgrade",
            Phase::Build => "phase.build",
            _ => "phase.siege",
        };
        let left = self.seconds_left();
        let mut top = vec![
            Item::Timer {
                id: "match.phase".into(),
                icon: hud_icon::url("timer"),
                phase: s.get(phase_key),
                text: left.to_string(),
                urgent: left <= TIMER_URGENT_SECONDS && left > 0,
            },
            stat(
                "match.round",
                "timer",
                s.fill("hud.round", &[("n", rules.round.to_string())]),
                String::new(),
            ),
        ];
        let alive = me.alive();
        let ready_text = if me.ready {
            s.get("hud.waiting")
        } else {
            s.get("hud.ready")
        };
        let mut bar = Vec::new();
        let mut extra = Vec::new();
        if self.replay.is_some() {
            top.push(Item::Lines {
                id: "match.replay".into(),
                lines: vec![s.get("hud.replay")],
            });
            bar.push(button("match.skip", s.get("hud.skip"), "skip", false));
        }
        if alive {
            top.push(stat(
                "match.gold",
                "gold",
                me.purse.gold.to_string(),
                s.get("tip.gold"),
            ));
            let (value, max, xp) = match me.purse.xp_needed() {
                Some(n) => (
                    me.purse.xp,
                    n,
                    s.fill("hud.xp", &[("n", me.purse.xp.to_string()), ("m", n.to_string())]),
                ),
                None => (1, 1, s.get("hud.xp_max")),
            };
            top.push(Item::Meter {
                id: "match.purse".into(),
                icon: hud_icon::url("level"),
                icon_name: "level".into(),
                label: me.purse.level.to_string(),
                value,
                max,
                text: xp,
                tip: s.get("tip.level"),
            });
            if let Some((t, _)) = rules.target_of(self.me) {
                top.push(stat(
                    "match.target",
                    "target",
                    self.names[t as usize].clone(),
                    s.get("tip.target"),
                ));
            }
        }
        match phase {
            Phase::Build if alive => {
                if let Some(p) = self.scout {
                    top.push(stat(
                        "match.scouting",
                        "scout",
                        self.names[p as usize].clone(),
                        s.get("tip.scouting"),
                    ));
                }
                bar.push(self.tool_bar(s));
                let xp_cost = crate::game::tune::XP_BUY_COST;
                let mut buy_xp = button(
                    "match.buy_xp",
                    s.fill(
                        "hud.buy_xp",
                        &[("x", crate::game::tune::XP_BUY_AMOUNT.to_string())],
                    ),
                    "xp",
                    me.purse.xp_needed().is_none() || me.purse.gold < xp_cost,
                );
                if let Item::Button { cost, .. } = &mut buy_xp {
                    *cost = xp_cost.to_string();
                }
                bar.push(buy_xp);
                let shot = me.mods.shot_price();
                let mut buy_shot = button(
                    "match.buy_shot",
                    s.get("hud.buy_shot"),
                    "buy_shot",
                    me.purse.gold < shot || self.phase() != Phase::Build,
                );
                if let Item::Button { cost, .. } = &mut buy_shot {
                    *cost = shot.to_string();
                }
                bar.push(buy_shot);
                bar.push(button("match.test", s.get("sandbox.test"), "test", false));
                bar.push(button(
                    "match.undo",
                    s.get("hud.undo"),
                    "undo",
                    !self.building_now() || self.play.last_placed(self.me).is_none(),
                ));
                bar.push(button("match.scout", s.get("hud.scout"), "scout", false));
                // every royal stands before Ready (R1.3305)
                if self.unplaced_royal().is_some() {
                    bar.push(button("match.ready", s.get("hud.place_royals"), "ready", true));
                } else {
                    bar.push(button("match.ready", ready_text, "ready", me.ready));
                }
            }
            Phase::Upgrade if alive => {
                if self.replay.is_none() {
                    extra.push(self.cards_panel(s, cards));
                }
            }
            Phase::Siege | Phase::SiegeOver if alive => {
                if let Some(f) = self.my_field() {
                    let key = if f.cleared() {
                        "hud.cleared"
                    } else if f.shooter.swing_from.is_some() {
                        "sandbox.throw"
                    } else {
                        "sandbox.swing"
                    };
                    top.push(Item::Lines {
                        id: "match.hint".into(),
                        lines: vec![s.get(key)],
                    });
                    if self.own_field().is_some() {
                        // the corner window's label: it is yours (R1.3208)
                        extra.push(Panel::new(
                            "field-inset",
                            String::new(),
                            vec![Item::Lines {
                                id: "match.inset".into(),
                                lines: vec![s.get("hud.replay")],
                            }],
                        ));
                    }
                    // no Ready in the siege: you are done when your shots
                    // are (R1.3207)
                    bar.extend(self.siege_bars(f, s));
                }
            }
            _ => {
                top.push(Item::Lines {
                    id: "match.out".into(),
                    lines: vec![s.get("hud.knocked_out")],
                });
            }
        }
        let mut leave = button("play.leave", s.get("play.leave"), "leave", false);
        if let Item::Button { quiet, .. } = &mut leave {
            *quiet = true;
        }
        top.push(leave);
        let mut panels = vec![
            Panel::new("field-top", String::new(), top),
            Panel::new("field-side", String::new(), vec![self.health_list()]),
            Panel::new("field-bar", String::new(), bar),
        ];
        panels.extend(extra);
        for panel in &mut panels {
            for item in &mut panel.items {
                if let Item::Button { id, tip, .. } = item {
                    if let Some(key) = button_tip(id) {
                        *tip = s.get(key);
                    }
                }
            }
        }
        panels
    }

    fn health_list(&self) -> Item {
        let rules = &self.play.rules;
        let mut order: Vec<PlayerId> = rules.alive();
        order.sort_by_key(|&p| std::cmp::Reverse(rules.players[p as usize].health));
        order.extend(rules.knocked_out.iter().rev());
        let target = rules.target_of(self.me).map(|(t, _)| t);
        let max = crate::game::tune::START_HEALTH.max(1) as u32;
        Item::Health {
            id: "match.health".into(),
            icon: hud_icon::url("heart"),
            rows: order
                .iter()
                .map(|&p| {
                    let pl = &rules.players[p as usize];
                    let value = pl.health.max(0) as u32;
                    HealthRow {
                        name: self.names[p as usize].clone(),
                        value: value.min(max),
                        max,
                        text: value.to_string(),
                        me: p == self.me,
                        out: !pl.alive(),
                        look: if target == Some(p) {
                            "target".into()
                        } else {
                            String::new()
                        },
                    }
                })
                .collect(),
        }
    }

    fn tool_bar(&self, s: &Strings) -> Item {
        let me = &self.play.rules.players[self.me as usize];
        let level = me.purse.level;
        let mut options = Vec::new();
        for (k, (piece, name)) in BEAMS.iter().enumerate() {
            let stock = match piece {
                Piece::Segment(ledger::Material::Wood) => me.ledger.stock.wood,
                Piece::Segment(ledger::Material::Stone) => me.ledger.stock.stone,
                Piece::Segment(ledger::Material::Metal) => me.ledger.stock.metal,
                _ => 0,
            };
            let cost = ledger::price(*piece, &me.mods);
            let unlock = ledger::unlock_level(*piece);
            let tool = s.get(&format!("tool.{name}"));
            let tip = if level < unlock {
                s.fill("tip.locked", &[("n", unlock.to_string())])
            } else if stock > 0 {
                s.fill(
                    "tool.stocked",
                    &[("name", tool.clone()), ("n", stock.to_string())],
                )
            } else {
                s.fill("tool.priced", &[("name", tool.clone()), ("n", cost.to_string())])
            };
            options.push(BarOption {
                tip,
                icon: hud_icon::url(name),
                icon_name: name.to_string(),
                count: if stock > 0 {
                    stock.to_string()
                } else {
                    String::new()
                },
                cost: if stock > 0 {
                    String::new()
                } else {
                    cost.to_string()
                },
                group: "beam".into(),
                value: name.to_string(),
                text: tool,
                key: (k + 1).to_string(),
                selected: self.tool == Tool::Beam(*piece),
                disabled: level < unlock && stock == 0,
            });
        }
        let unplaced = (0..me.royals.len())
            .filter(|&i| {
                me.royals[i].spot.is_none()
                    && self.play.castles[self.me as usize]
                        .royal_at
                        .get(i)
                        .copied()
                        .flatten()
                        .is_none()
            })
            .count();
        options.push(BarOption {
            tip: s.get("tip.royal"),
            icon: hud_icon::url("royal"),
            icon_name: "royal".into(),
            count: unplaced.to_string(),
            cost: String::new(),
            group: "royal".into(),
            value: "royal".into(),
            text: s.get("tool.royal"),
            key: (BEAMS.len() + 1).to_string(),
            selected: self.tool == Tool::Royal,
            disabled: unplaced == 0,
        });
        for (kind, name) in BUILDINGS.iter() {
            let piece = Piece::Building(*kind);
            let unlock = ledger::unlock_level(piece);
            let made = match ledger::building_row(*kind).yields {
                crate::game::tune::Yield::Stock(_, n) => n + me.mods.producer_bonus,
                crate::game::tune::Yield::Shots(n) => me.mods.armory_shots.unwrap_or(n),
            };
            let tip = if level < unlock {
                s.fill("tip.locked", &[("n", unlock.to_string())])
            } else {
                s.fill(&format!("tip.{name}"), &[("n", made.to_string())])
            };
            options.push(BarOption {
                tip,
                icon: hud_icon::url(name),
                icon_name: name.to_string(),
                count: String::new(),
                cost: ledger::price(piece, &me.mods).to_string(),
                group: "make".into(),
                value: name.to_string(),
                text: s.get(&format!("tool.{name}")),
                key: String::new(),
                selected: self.tool == Tool::Building(*kind),
                disabled: level < unlock,
            });
        }
        Item::Bar {
            id: "match.tool".into(),
            options,
        }
    }

    fn siege_bars(&self, f: &Field, s: &Strings) -> Vec<Item> {
        let sh = &f.shooter;
        let busy = sh.swing_from.is_some();
        let ammo = Item::Bar {
            id: "match.ammo".into(),
            options: sh
                .ammo
                .iter()
                .enumerate()
                .take(KEY_DIGITS.len())
                .map(|(k, (a, n))| BarOption {
                    tip: s.get(&format!("tip.ammo.{}", ammo_name(*a))),
                    icon: hud_icon::url(ammo_name(*a)),
                    icon_name: ammo_name(*a).to_string(),
                    count: n.to_string(),
                    cost: String::new(),
                    group: String::new(),
                    value: k.to_string(),
                    text: s.get(&format!("ammo.{}", ammo_name(*a))),
                    key: (k + 1).to_string(),
                    selected: k == sh.selected as usize,
                    disabled: *n == 0 || (busy && k != sh.selected as usize),
                })
                .collect(),
        };
        let w = &f.castle.world;
        let flying = sh
            .live
            .iter()
            .any(|&i| w.projectiles.get(i).is_some_and(|b| b.alive));
        let mut out = vec![ammo];
        if !sh.air.is_empty() {
            out.push(Item::Bar {
                id: "match.air".into(),
                options: sh
                    .air
                    .iter()
                    .zip(KEY_AIR.iter())
                    .enumerate()
                    .map(|(k, (a, (code, _)))| BarOption {
                        tip: s.get(&format!("tip.air.{}", air_name(*a))),
                        icon: hud_icon::url(air_name(*a)),
                        icon_name: air_name(*a).to_string(),
                        count: String::new(),
                        cost: String::new(),
                        group: String::new(),
                        value: k.to_string(),
                        text: s.get(&format!("air.{}", air_name(*a))),
                        key: code.trim_start_matches("Key").to_string(),
                        selected: false,
                        disabled: !flying || sh.air_used,
                    })
                    .collect(),
            });
        }
        out
    }

    fn cards_panel(&self, s: &Strings, cards: &CardText) -> Panel {
        let me = &self.play.rules.players[self.me as usize];
        let mut items = Vec::new();
        if let Some(offer) = &me.offer {
            for (k, &i) in offer.cards.iter().enumerate() {
                let row = &UPGRADES[i];
                let (name, text) = cards.get(row.key);
                items.push(Item::Card {
                    id: format!("match.card.{k}"),
                    value: k.to_string(),
                    name,
                    tier: s.get(&format!("tier.{}", tier_name(row.tier))),
                    look: tier_name(row.tier).to_string(),
                    text,
                    icon: hud_icon::url(card_icon(row.effects)),
                    picked: offer.picked == Some(i),
                    disabled: offer.picked.is_some(),
                });
            }
            let cost = crate::game::upgrades::reroll_cost(offer.rerolls);
            let text = if cost == 0 {
                s.get("hud.reroll_plain")
            } else {
                s.fill("hud.reroll", &[("n", cost.to_string())])
            };
            items.push(Item::Button {
                id: "match.reroll".into(),
                text,
                quiet: true,
                disabled: offer.picked.is_some() || me.purse.gold < cost,
                tip: s.get("tip.reroll"),
                icon: hud_icon::url("reroll"),
                cost: String::new(),
            });
        }
        Panel::new("cards", s.get("phase.upgrade"), items)
    }

    fn results_panels(&self, s: &Strings) -> Vec<Panel> {
        let rules = &self.play.rules;
        let lines = rules
            .standings()
            .iter()
            .enumerate()
            .map(|(k, &p)| {
                s.fill(
                    "results.place",
                    &[
                        ("n", (k + 1).to_string()),
                        ("name", self.names[p as usize].clone()),
                    ],
                )
            })
            .collect();
        vec![Panel::new(
            "results",
            s.get("results.heading"),
            vec![
                Item::Lines {
                    id: "match.results".into(),
                    lines,
                },
                if self.is_online() {
                    Item::Button {
                        tip: String::new(),
                        icon: String::new(),
                        cost: String::new(),
                        id: "play.lobby".into(),
                        text: s.get("results.lobby"),
                        quiet: false,
                        disabled: false,
                    }
                } else {
                    Item::Button {
                        tip: String::new(),
                        icon: String::new(),
                        cost: String::new(),
                        id: "match.again".into(),
                        text: s.get("results.again"),
                        quiet: false,
                        disabled: false,
                    }
                },
                Item::Button {
                    tip: String::new(),
                    icon: String::new(),
                    cost: String::new(),
                    id: "play.leave".into(),
                    text: s.get("results.menu"),
                    quiet: true,
                    disabled: false,
                },
            ],
        )]
    }
}

#[cfg(test)]
mod tests;

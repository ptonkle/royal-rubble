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
    cloth_at, ground_line, material_name, tree_marks, world_parts, world_right, Camera, Cue, Engine, Figure,
    Flag, Frame, Inset, Preview, Ring, SpotMark,
};
use super::hud_icon;
use super::sandbox::{num, pair, reach_end};
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

/// The keys the match listens to.
pub fn keys() -> BTreeMap<String, u32> {
    let mut k = BTreeMap::new();
    k.insert("Space".to_string(), KEY_SPACE);
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
/// Cues kept for the page.
const CUES_KEPT: usize = 24;
/// Breaks in one tick that sound as a collapse.
const COLLAPSE_MIN: u32 = 3;
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
        AirKind::Hover => "hover",
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
    /// The siege view as shown, easing toward `camera_target` each tick.
    eye: Option<Camera>,
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
        let mut bot_n = 0;
        let names = start
            .seats
            .iter()
            .enumerate()
            .map(|(i, st)| {
                if i as PlayerId == me {
                    s.get("seat.you")
                } else if st.bot.is_some() {
                    bot_n += 1;
                    s.fill("seat.bot_n", &[("n", bot_n.to_string())])
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
            eye: None,
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
        // just after release the view stays on the trebuchet so the sling's
        // whip and the follow-through are seen (R1.1800)
        let held = flying.is_some_and(|b| b.age < CAM_HOLD);
        let flying = flying.filter(|_| !held);
        let base = f.base;
        if let Some(b) = flying {
            let left = b.pos.x - Fx::from_int(CAM_BEHIND);
            let right = (cr + Fx::from_int(CAM_MARGIN)).max(b.pos.x + Fx::from_int(CAM_MARGIN));
            let width = (right - left)
                .min(Fx::from_int(CAM_FLY_MAX_W))
                .max(castle.width());
            // the castle's frame once the shot is inside it
            let left = if left + width > right {
                (right - width).max(castle.left.min(left))
            } else {
                left
            };
            // a high shot widens the view so the ground stays in it, up to
            // a widest view; past that the view rises with the shot (R1.1700)
            let floor = base.y.min(ground);
            let tall = (b.pos.y + Fx::from_int(CAM_MARGIN) - floor) * 6 / 5;
            let width = width.max(
                (tall * Fx::from_int(w as i64) / Fx::from_int(h.max(1) as i64))
                    .min(Fx::from_int(CAM_FLY_TALL_W)),
            );
            // the trebuchet stays in while the widened view still holds the ball
            let home = base.x - Fx::from_int(CAM_BEHIND);
            let left = if home < left && b.pos.x + Fx::from_int(CAM_MARGIN) < home + width {
                home
            } else {
                left
            };
            let mut cam = Camera::spanning(left, width, Fx::ZERO, w, h);
            cam.bottom = floor - cam.height() / 6;
            let mut cam = cam.kept_in(edge);
            let above = b.pos.y + Fx::from_int(CAM_MARGIN) - (cam.bottom + cam.height());
            if above > Fx::ZERO {
                cam.bottom += above;
            }
            return cam;
        }
        if settled && !held {
            return castle.kept_in(edge);
        }
        let left = base.x - Fx::from_int(CAM_BEHIND);
        let width = (cl + Fx::from_int(CAM_MARGIN) - left)
            .min(Fx::from_int(CAM_AIM_MAX_W))
            .max(Fx::from_int(CAM_AIM_MIN_W));
        let mut cam = Camera::spanning(left, width, Fx::ZERO, w, h);
        cam.bottom = base.y.min(ground) - cam.height() / 6;
        cam.kept_in(edge)
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
                self.siege_camera(f, f.shooter.swing_from.is_none() && !self.trail.is_empty(), w, h)
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
        if self.phase() != Phase::Siege {
            return;
        }
        let Some(f) = self.my_field() else { return };
        if f.shooter.swing_from.is_none() {
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

    /// Online: this browser's inputs for the room to send, in order.
    pub fn take_inputs(&mut self) -> Vec<Input> {
        if self.over() {
            self.pending.clear();
            return Vec::new();
        }
        self.release_held_swing();
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
        let after = Arena::counts(self.my_field());
        let breaks = after.0.saturating_sub(before.0);
        if breaks >= COLLAPSE_MIN {
            self.cue("collapse", breaks);
        } else if breaks > 0 {
            self.cue("impact_wood", 0);
        }
        if after.1 > before.1 {
            self.cue("royal_knight", 0);
        }
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
        let phase = self.view_phase();
        let round = self.play.rules.round;
        let moved = phase != self.seen_phase || round != self.seen_round;
        if moved {
            self.entered(phase);
            changed = true;
        }
        if changed {
            self.rebuild();
        }
        self.move_eye(moved, ran);
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
            .filter_map(|&i| w.projectiles.get(i))
            .find(|b| b.alive);
        match live {
            Some(b) => {
                if b.age.is_multiple_of(TRAIL_EVERY) {
                    let p = b.pos;
                    self.path.push(p);
                }
            }
            None => {
                if !self.path.is_empty() {
                    self.trail = std::mem::take(&mut self.path);
                }
            }
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
        let mut best: Option<(Fx, Fx)> = None;
        let mut consider = |y: Fx| {
            let d = (y - c.y).abs();
            if d <= Fx::ONE && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, y));
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
        best.map(|(_, y)| grid_of(Vec2::new(x, y)))
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

    /// A press on the field: `x,y,button` in pixels (button 2 sells).
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
            if let Some(id) = self.piece_near(p) {
                self.queue(Input::Sell(id));
            }
            return Ok(());
        }
        match self.tool {
            Tool::Beam(_) => {
                let s = Vec2::new(snap(p.x), snap(p.y));
                if self.can_start(s) {
                    self.drag = Some(s);
                }
            }
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

    pub fn moved(&mut self, value: &str) -> Result<(), String> {
        let (x, y) = pair(value)?;
        self.cursor = Some(self.camera().world(x, y));
        Ok(())
    }

    pub fn up(&mut self, value: &str) -> Result<(), String> {
        let (x, y) = pair(value)?;
        let p = self.camera().world(x, y);
        self.cursor = Some(p);
        let Some(a) = self.drag.take() else {
            return Ok(());
        };
        let Tool::Beam(piece) = self.tool else {
            return Ok(());
        };
        let b = reach_end(a, p, self.max_len());
        if a != b && self.in_zone(b) {
            self.queue(Input::PlaceSegment {
                piece,
                a: grid_of(a),
                b: grid_of(b),
            });
        }
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
            "match.ready" => self.queue(Input::Ready),
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
    fn figures(&self, world: &World, bodies: &[BodyOwner], owner: PlayerId, cam: &Camera) -> Vec<Figure> {
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
        let royals = self.figures(world, &f.castle.bodies, f.owner, &cam);
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
        }
    }

    pub fn frame(&self) -> Frame {
        let (w, h) = self.size;
        let cam = self.camera();
        if let Some(r) = &self.replay {
            let mut f = self.field_frame(&r.field, r.tick, cam, false);
            f.cues = self.cues.clone();
            f.music = "music_build";
            f.flag = Some(self.flag());
            return f;
        }
        let now = self.play.siege.as_ref().map_or(0, |s| s.tick);
        if let (Phase::Siege | Phase::SiegeOver, Some(mine)) = (self.phase(), self.my_field()) {
            let mut f = self.field_frame(mine, now, cam, true);
            f.flag = Some(self.flag());
            f.cues = self.cues.clone();
            if let Some(own) = self.own_field() {
                let icam = self.build_camera(self.me, INSET_W, INSET_H);
                let mut inner = self.field_frame(own, now, icam, false);
                inner.loops.clear();
                f.inset = Some(Box::new(Inset {
                    at: [w - INSET_W - INSET_GAP, INSET_TOP],
                    frame: inner,
                }));
            }
            return f;
        }
        // build, upgrade, results, or a siege you sit out: a castle at rest
        let p = self.shown();
        let building = self.building_now();
        let owned_world;
        let (world, bodies): (&World, &[BodyOwner]) = if p == self.me {
            match &self.built {
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
        let strain = if building && self.testing {
            self.strain.as_deref()
        } else {
            None
        };
        let (beams, nodes, shots, debris) = world_parts(world, &cam, strain, building);
        let royals = self.figures(world, bodies, p, &cam);
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
        let base = crate::play::units::trebuchet_base(plot);
        Frame {
            mode: "build",
            size: [w, h],
            cloth: cloth_at(&cam),
            ground,
            water,
            trees: tree_marks(plot, &cam),
            zone,
            grid: if building { cam.len(TUNE.grid, 1) } else { 0 },
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
            music: "music_build",
            loops: Vec::new(),
        }
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
            music: "music_build",
            loops: Vec::new(),
        }
    }

    fn build_marks(&self, cam: &Camera) -> (Option<Preview>, Option<Ring>) {
        let Some(c) = self.cursor else {
            return (None, None);
        };
        match self.tool {
            Tool::Beam(piece) => {
                let m = castle::physics_material(piece).unwrap_or(Material::Wood);
                let Some(a) = self.drag else {
                    let s = Vec2::new(snap(c.x), snap(c.y));
                    let p = cam.px(s);
                    return (
                        None,
                        Some(Ring {
                            x: p[0],
                            y: p[1],
                            r: cam.len(TUNE.grid / 2, 3),
                            ok: self.can_start(s),
                        }),
                    );
                };
                let b = reach_end(a, c, self.max_len());
                let pa = cam.px(a);
                let preview = Preview {
                    a: pa,
                    b: cam.px(b),
                    w: cam.len(TUNE.material(m).thickness, 2),
                    material: material_name(m),
                    ok: a != b && self.in_zone(b),
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
                let index = self.unplaced_royal();
                let at = self.stand_point(c);
                let p = cam.px(at.map_or(c, |g| g.to_fx()));
                let spacing = Fx::from_int(crate::game::royals::spacing_cells() as i64);
                let ok = matches!((index, at), (Some(i), Some(g)) if self.royal_ok(i, g));
                (
                    None,
                    Some(Ring {
                        x: p[0],
                        y: p[1],
                        r: cam.len(spacing, 4),
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
                bar.push(button(
                    "match.buy_xp",
                    s.fill(
                        "hud.buy_xp",
                        &[
                            ("x", crate::game::tune::XP_BUY_AMOUNT.to_string()),
                            ("n", xp_cost.to_string()),
                        ],
                    ),
                    "xp",
                    me.purse.xp_needed().is_none() || me.purse.gold < xp_cost,
                ));
                let shot = me.mods.shot_price();
                bar.push(button(
                    "match.buy_shot",
                    s.fill("hud.buy_shot", &[("n", shot.to_string())]),
                    "buy_shot",
                    me.purse.gold < shot || self.phase() != Phase::Build,
                ));
                bar.push(button("match.test", s.get("sandbox.test"), "test", false));
                bar.push(button("match.scout", s.get("hud.scout"), "scout", false));
                bar.push(button("match.ready", ready_text, "ready", me.ready));
            }
            Phase::Upgrade if alive => {
                if self.replay.is_none() {
                    extra.push(self.cards_panel(s, cards));
                }
            }
            Phase::Siege | Phase::SiegeOver if alive => {
                if let Some(f) = self.my_field() {
                    let key = if f.shooter.swing_from.is_some() {
                        "sandbox.throw"
                    } else {
                        "sandbox.swing"
                    };
                    top.push(Item::Lines {
                        id: "match.hint".into(),
                        lines: vec![s.get(key)],
                    });
                    bar.extend(self.siege_bars(f, s));
                    bar.push(button("match.ready", ready_text, "ready", me.ready));
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
                disabled: level < unlock,
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
                        id: "play.lobby".into(),
                        text: s.get("results.lobby"),
                        quiet: false,
                        disabled: false,
                    }
                } else {
                    Item::Button {
                        tip: String::new(),
                        icon: String::new(),
                        id: "match.again".into(),
                        text: s.get("results.again"),
                        quiet: false,
                        disabled: false,
                    }
                },
                Item::Button {
                    tip: String::new(),
                    icon: String::new(),
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

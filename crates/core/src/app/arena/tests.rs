use super::*;
use crate::game::flow::{Difficulty, Seat};
use crate::lobby::{Options, StartSeat};
use crate::strings::tests::sample;

fn start(bots: usize, seed: u64) -> MatchStart {
    let mut seats = vec![StartSeat {
        seat: 0,
        person: "me".into(),
        name: "Ann".into(),
        bot: None,
    }];
    for k in 0..bots {
        seats.push(StartSeat {
            seat: k as u32 + 1,
            person: String::new(),
            name: String::new(),
            bot: Some(crate::lobby::Difficulty::Normal),
        });
    }
    MatchStart {
        seed,
        seats,
        options: Options {
            players: bots as u32 + 1,
            wind: true,
            flat: false,
        },
    }
}

fn arena(bots: usize) -> Arena {
    let s = Strings::parse(&sample()).unwrap();
    Arena::new(&start(bots, 7), "me", true, &s).unwrap()
}

#[test]
fn arena_starts_a_real_match_in_build_with_you_in_the_first_seat() {
    let a = arena(1);
    assert_eq!(a.phase(), Phase::Build);
    assert_eq!(a.me, 0);
    assert_eq!(a.play.rules.players.len(), 2);
    assert_eq!(a.play.rules.players[1].seat, Seat::Bot(Difficulty::Normal));
}

#[test]
fn arena_picking_a_card_shows_the_build_view_with_short_tips() {
    let mut a = arena(1);
    while a.phase() != Phase::Upgrade {
        a.queue(Input::Ready);
        a.tick();
    }
    a.replay = None;
    assert_eq!(a.view_phase(), Phase::Upgrade);
    // as if the bot were a person still choosing, so the phase runs on
    a.play.rules.players[1].ready = false;
    a.queue(Input::Pick(0));
    a.tick();
    assert_eq!(a.phase(), Phase::Upgrade);
    assert_eq!(a.view_phase(), Phase::Build);
    let s = Strings::parse(&sample()).unwrap();
    let panels = a.panels(&s, &CardText::default());
    let tips: Vec<String> = panels
        .iter()
        .flat_map(|p| p.items.iter())
        .filter_map(|i| match i {
            Item::Button { id, tip, .. } if id == "match.test" => Some(tip.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(tips, vec!["tip.test".to_string()]);
}

#[test]
fn arena_a_press_and_release_in_build_places_a_wood_segment_through_play() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    let (l, _, _) = a.zone(0);
    let x = l + Fx::from_int(30);
    let g = snap(a.ground(0, x));
    // a ground point, then a point on the half-cell grid 1 to 1.5 cells up
    // (the ground may sit on a quarter cell, R1.4300)
    let half = Fx::ratio(1, 2).0;
    let up = Fx((g + Fx::ratio(3, 2)).0.div_euclid(half) * half);
    let cam = a.camera();
    let p0 = cam.px(Vec2::new(x, g + Fx::ratio(1, 16)));
    let p1 = cam.px(Vec2::new(x, up));
    let before = a.play.castles[0].geo.len();
    // a press and release at the start places nothing
    a.down(&format!("{},{},0", p0[0], p0[1])).unwrap();
    a.up(&format!("{},{}", p0[0], p0[1])).unwrap();
    a.tick();
    assert_eq!(a.play.castles[0].geo.len(), before);
    // the second press places it, and the next segment starts at its end
    a.down(&format!("{},{},0", p1[0], p1[1])).unwrap();
    a.tick();
    assert_eq!(a.play.castles[0].geo.len(), before + 1);
    assert_eq!(a.drag, Some(Vec2::new(x, up)));
    // a right press lets go of the start and sells nothing
    a.down(&format!("{},{},2", p1[0], p1[1])).unwrap();
    a.tick();
    assert_eq!(a.drag, None);
    assert_eq!(a.play.castles[0].geo.len(), before + 1);
}

#[test]
fn arena_undo_sells_the_last_piece_back_for_all_it_cost() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    let (l, _, _) = a.zone(0);
    let x = l + Fx::from_int(30);
    let g = snap(a.ground(0, x));
    let cam = a.camera();
    let gold = a.play.rules.players[0].purse.gold;
    for y in [g, g + Fx::from_int(2)] {
        let p = cam.px(Vec2::new(x, y));
        a.down(&format!("{},{},0", p[0], p[1])).unwrap();
    }
    a.tick();
    assert_eq!(a.play.castles[0].geo.len(), 1);
    a.pump(0, KEY_UNDO, None, None);
    a.tick();
    assert!(a.play.castles[0].geo.is_empty());
    assert_eq!(a.play.rules.players[0].purse.gold, gold);
}

#[test]
fn arena_ready_waits_for_every_royal_then_the_royal_tool_is_put_away() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    assert!(a.unplaced_royal().is_some());
    a.act("match.ready", "").unwrap();
    assert!(a.pending.is_empty());
    // place the royal: the tool goes back to wood and Ready works
    a.tool = Tool::Royal;
    let (l, _, _) = a.zone(0);
    let x = l + Fx::from_int(20);
    let p = a.camera().px(Vec2::new(x, a.ground(0, x)));
    a.down(&format!("{},{},0", p[0], p[1])).unwrap();
    a.tick();
    assert_eq!(a.unplaced_royal(), None);
    assert_eq!(a.tool, Tool::Beam(BEAMS[0].0));
    a.act("match.ready", "").unwrap();
    assert_eq!(a.pending, vec![Input::Ready]);
}

/// Plays a whole match: Ready in every phase, and in the siege two taps of
/// Space for each shot. Returns the ticks it took.
fn play_out(a: &mut Arena) -> (u32, usize) {
    // a release that ends the match on this seed (R1.1802); with the steady
    // 110-tick swing (R1.1803) every release 40-190 does, in 36,500-39,300
    // ticks, and 120 is past the swing's end (the held swing throws itself)
    play_out_at(a, 70, 200_000)
}

/// Swing ticks to release at for `play_out`: run with `--ignored` after a
/// trebuchet change; it fails listing the releases that end the match.
#[test]
#[ignore]
fn arena_whole_match_release_sweep() {
    let mut ends = vec![];
    for r in (40u32..=190).step_by(10) {
        let mut a = arena(1);
        let (t, _) = play_out_at(&mut a, r, 40_000);
        if a.over() {
            ends.push((r, t));
        }
    }
    panic!("(release, ticks) that end the match: {ends:?}");
}

fn play_out_at(a: &mut Arena, release: u32, cap: u32) -> (u32, usize) {
    let mut ticks = 0;
    let mut bot_pieces = 0;
    while !a.over() && ticks < cap {
        match a.phase() {
            Phase::Siege => {
                let left = a.my_field().map_or(0, |f| f.shooter.shots_left());
                let swinging = a.my_field().is_some_and(|f| f.shooter.swing_from.is_some());
                if left == 0 {
                    a.queue(Input::Ready);
                } else if (!swinging && ticks % 200 == 0) || (swinging && ticks % 200 == release) {
                    a.space(crate::play::ON_TICK);
                }
            }
            _ => a.queue(Input::Ready),
        }
        a.tick();
        ticks += 1;
        bot_pieces = bot_pieces.max(a.play.castles[1].geo.len());
    }
    (ticks, bot_pieces)
}

#[test]
fn arena_a_match_vs_one_bot_reaches_the_results_with_one_player_left() {
    let mut a = arena(1);
    let (ticks, bot_pieces) = play_out(&mut a);
    // the bot's castle went up through play inputs: segments with places
    assert!(bot_pieces > 4, "the bot's castle had at most {bot_pieces} pieces");
    assert!(
        a.over(),
        "still in {:?} round {} after {ticks} ticks",
        a.phase(),
        a.play.rules.round
    );
    assert_eq!(a.play.rules.alive().len(), 1);
    let s = Strings::parse(&sample()).unwrap();
    let ids: Vec<String> = a
        .panels(&s, &CardText::default())
        .iter()
        .flat_map(|p| p.items.iter())
        .filter_map(|i| match i {
            Item::Button { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["match.again", "play.leave"]);
}

/// A few build ticks (the view follows the build), then Ready in every
/// phase until the siege's first tick.
fn to_siege(a: &mut Arena) {
    for _ in 0..10 {
        a.tick();
    }
    let mut n = 0;
    while a.phase() != Phase::Siege && n < 100_000 {
        a.queue(Input::Ready);
        a.tick();
        n += 1;
    }
}

/// Whether world point `p` lands on the screen in the shown view.
fn on_screen(a: &Arena, p: Vec2) -> bool {
    let [x, y] = a.camera().px(p);
    (0..1920).contains(&x) && (0..1080).contains(&y)
}

#[test]
fn arena_the_first_siege_tick_shows_the_trebuchet_and_the_ground_under_it() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    assert_eq!(a.phase(), Phase::Siege);
    let f = a.my_field().unwrap();
    let base = f.base;
    let ground = crate::play::units::physics_terrain(&f.plot).height_at(base.x);
    assert!(on_screen(&a, base), "base {:?} off {:?}", base, a.camera());
    assert!(
        on_screen(&a, Vec2::new(base.x, ground - Fx::ONE)),
        "ground off {:?}",
        a.camera()
    );
}

#[test]
fn arena_the_aim_view_shows_the_trebuchet_and_the_castle_front_up_close() {
    // R1.4200: the aim view stopped 38 units past the trebuchet, short of a
    // castle 60 to 80 away, and the flight then opened to tiny castles
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    for _ in 0..CAM_EASE * 8 {
        a.tick();
    }
    let f = a.my_field().unwrap();
    let (cl, _, ground, top) = a.castle_box(f);
    let base = f.base;
    assert!(on_screen(&a, base), "trebuchet off {:?}", a.camera());
    assert!(
        on_screen(&a, Vec2::new(cl, (ground + top) / 2)),
        "castle front {cl:?} base {base:?} off {:?}",
        a.camera()
    );
    // never wider than 110 units: over 17 pixels a unit
    assert!(a.camera().ppu > Fx::from_int(17), "{:?}", a.camera());
}

#[test]
fn arena_a_high_shot_is_followed_at_the_aim_zoom() {
    // R1.4200: a high shot opened the view to 260 units and the castles
    // shrank to a few pixels; now the flight keeps the aim width
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    // the first forward release throws nearly straight up (R1.1601)
    let up = (0..crate::physics::SWING_END)
        .find(|&t| crate::physics::trebuchet_launch(Vec2::ZERO, t, 0, false).vel.x > Fx::ZERO)
        .unwrap();
    a.space(crate::play::ON_TICK);
    for _ in 0..up {
        a.tick();
    }
    a.space(crate::play::ON_TICK);
    for _ in 0..80 {
        a.tick();
    }
    let f = a.my_field().unwrap();
    let w = &f.castle.world;
    let ball = f
        .shooter
        .live
        .iter()
        .filter_map(|&i| w.projectiles.get(i))
        .find(|b| b.alive)
        .unwrap();
    // from the run: the ball near 94 units up, the base near 40
    assert!(
        ball.pos.y > f.base.y + Fx::from_int(40),
        "ball only at {:?}",
        ball.pos
    );
    // the view heads for one with the ball in it (the eased eye lags it)
    let [bx, by] = a.camera_target().px(ball.pos);
    assert!(
        (0..1920).contains(&bx) && (0..1080).contains(&by),
        "ball {:?} at {bx},{by} off {:?}",
        ball.pos,
        a.camera_target()
    );
    let aim = a.aim_width(f, 1920, 1080);
    assert_eq!(a.camera_target().ppu, Fx::from_int(1920) / aim);
    assert!(aim <= Fx::from_int(110), "aim view {aim:?} wide");
}

#[test]
fn arena_the_view_keeps_the_trebuchet_in_just_after_release() {
    // R1.1800: the whip and follow-through are seen; since R1.3204 the view
    // follows from release, starting from the trebuchet
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    let t = crate::physics::SWING_END * 2 / 3;
    a.space(crate::play::ON_TICK);
    for _ in 0..t {
        a.tick();
    }
    a.space(crate::play::ON_TICK);
    let base = a.my_field().unwrap().base;
    let top = base + Vec2::new(Fx::ZERO, Fx::from_int(8));
    // the shot ages one tick as it leaves
    for k in 0..CAM_HOLD - 1 {
        a.tick();
        assert!(
            on_screen(&a, base) && on_screen(&a, top),
            "tick {k}: {:?}",
            a.camera()
        );
    }
}

#[test]
fn arena_release_part_counts_sixteenths_from_the_tick_start() {
    // R1.2103: at 60 ticks a second one sixteenth is 1,041.7 microseconds
    assert_eq!(release_part(1_000, 1_000, 60), 0);
    assert_eq!(release_part(1_000 + 16_667, 1_000, 60), ON_TICK_PART);
    assert_eq!(release_part(1_000 + 25_000, 1_000, 60), 24);
    assert_eq!(release_part(500, 1_000, 60), 0);
    assert_eq!(release_part(1_000 + 40_000, 1_000, 60), 31);
}

const ON_TICK_PART: u8 = crate::play::ON_TICK;

/// The shot's sideways speed after a solo swing whose release Space went
/// down `press_after_us` after the start of the last tick before it.
fn thrown_vx(press_after_us: i64) -> Fx {
    let mut a = arena(1);
    to_siege(&mut a);
    a.pump(1000, 0, None, None);
    a.pump(1000, KEY_SPACE, Some(1_000_000), None);
    // fast play runs 480 ticks a second: by 1137 ms, 65 ticks
    for ms in 1001..=1137 {
        a.pump(ms, 0, None, None);
    }
    let basis = 1_000_000 + 65 * 1_000_000 / 480;
    a.pump(1138, KEY_SPACE, Some(basis + press_after_us), None);
    for ms in 1139..=1141 {
        a.pump(ms, 0, None, None);
    }
    let f = a.my_field().unwrap();
    let w = &f.castle.world;
    let ball = f
        .shooter
        .live
        .iter()
        .filter_map(|&i| w.projectiles.get(i))
        .find(|b| b.alive);
    ball.expect("the shot is in the air").vel.x
}

#[test]
fn arena_release_press_time_throws_between_whole_ticks() {
    // R1.2103: presses 0, 8 and 16 sixteenths into the tick (a fast tick is
    // 2,083 microseconds): the middle one throws between the other two
    let (a, b, c) = (thrown_vx(100), thrown_vx(100 + 1_042), thrown_vx(100 + 2_084));
    assert!(a != c, "{a:?} {c:?}");
    assert!((a < b && b < c) || (a > b && b > c), "{a:?} {b:?} {c:?}");
}

/// Whether the shown view stays inside the world, `0..right` across.
fn view_in_world(a: &Arena) -> Result<(), String> {
    let f = a.my_field().ok_or("no field")?;
    let right = super::world_right(&f.plot);
    let cam = a.camera();
    // the far edge may pass by the rounding of a pixel
    let slack = Fx::ONE / 8;
    if cam.left < Fx::ZERO || cam.left + cam.width() > right + slack {
        return Err(format!("{cam:?} past 0..{right:?}"));
    }
    Ok(())
}

#[test]
fn arena_the_siege_view_never_shows_past_the_world_edges() {
    // R1.2200: the trebuchet stands near x 14, so a view built round it
    // showed empty linen left of the world
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    view_in_world(&a).unwrap();
    // aiming: the trebuchet in the left part of the screen
    let base = a.my_field().unwrap().base;
    let [bx, _] = a.camera().px(base);
    assert!((0..1920 / 3).contains(&bx), "trebuchet at x {bx}");
    let t = crate::physics::SWING_END * 2 / 3;
    a.space(crate::play::ON_TICK);
    for _ in 0..t {
        a.tick();
    }
    a.space(crate::play::ON_TICK);
    for k in 0..400 {
        a.tick();
        if let Err(e) = view_in_world(&a) {
            panic!("tick {k}: {e}");
        }
    }
}

fn timer_urgent(a: &Arena) -> Option<bool> {
    let s = Strings::parse(&sample()).unwrap();
    a.panels(&s, &CardText::default())
        .iter()
        .flat_map(|p| p.items.iter())
        .find_map(|i| match i {
            Item::Timer { urgent, .. } => Some(*urgent),
            _ => None,
        })
}

#[test]
fn arena_the_timer_turns_urgent_in_its_last_ten_seconds() {
    let mut a = arena(1);
    let tps = crate::game::tune::TICKS_PER_SECOND;
    a.play.rules.ticks_left = 11 * tps;
    assert_eq!(timer_urgent(&a), Some(false));
    a.play.rules.ticks_left = 10 * tps;
    assert_eq!(timer_urgent(&a), Some(true));
}

#[test]
fn arena_drum_beats_each_new_second_of_the_last_ten_harder_in_the_last_three() {
    assert_eq!(drum_beat(11, 10), Some("drum_count"));
    assert_eq!(drum_beat(5, 4), Some("drum_count"));
    assert_eq!(drum_beat(4, 3), Some("drum_close"));
    assert_eq!(drum_beat(2, 1), Some("drum_close"));
    assert_eq!(drum_beat(12, 11), None);
    assert_eq!(drum_beat(10, 10), None);
    assert_eq!(drum_beat(1, 0), None);
}

#[test]
fn arena_the_build_timer_reaching_ten_seconds_cues_the_drum() {
    let mut a = arena(1);
    let tps = crate::game::tune::TICKS_PER_SECOND;
    a.play.rules.ticks_left = 10 * tps + 2;
    a.tick();
    assert!(a.cues.iter().all(|c| c.name != "drum_count"));
    a.play.rules.ticks_left = 10 * tps + 1;
    a.tick();
    assert_eq!(a.phase(), Phase::Build);
    assert_eq!(a.cues.last().map(|c| c.name), Some("drum_count"));
}

#[test]
fn arena_a_royal_killed_in_your_siege_cries_by_its_rank() {
    let mut a = arena(1);
    to_siege(&mut a);
    let me = a.me;
    let s = a.play.siege.as_mut().unwrap();
    let f = s
        .fields
        .iter_mut()
        .find(|f| f.attacker == me && !f.shooter.bot)
        .unwrap();
    let i = f
        .castle
        .bodies
        .iter()
        .position(|b| matches!(b, BodyOwner::Royal(_)))
        .unwrap();
    let BodyOwner::Royal(r) = f.castle.bodies[i] else {
        unreachable!()
    };
    let owner = f.owner as usize;
    f.castle.world.bodies[i].alive = false;
    let kind = a.play.rules.players[owner].royals[r].kind;
    a.tick();
    let cries: Vec<&str> = a
        .cues
        .iter()
        .map(|c| c.name)
        .filter(|n| n.starts_with("royal_"))
        .collect();
    assert_eq!(cries, vec![death_cry(kind)]);
}

#[test]
fn arena_songs_building_songs_take_turns_round_by_round() {
    assert_eq!(build_song_for(1), "music_build");
    assert_eq!(build_song_for(2), "music_jig");
    assert_eq!(build_song_for(5), "music_mandolins");
    assert_eq!(build_song_for(6), "music_build");
    let mut a = arena(1);
    a.play.rules.round = 3;
    assert_eq!(a.frame().music, "music_pastoral");
}

/// Every track the core names is in the compiled sounds.
#[test]
fn arena_songs_every_track_the_core_names_is_compiled() {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../data/sound-programs.json")).unwrap();
    let ids: Vec<&str> = v["programs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["id"].as_str())
        .collect();
    for name in BUILD_SONGS
        .iter()
        .chain(["music_siege", "music_menu", "drum_count", "drum_close"].iter())
    {
        assert!(ids.contains(name), "{name} is not compiled");
    }
}

#[test]
fn flat_map_option_levels_every_column_and_drains_the_water() {
    let s = Strings::parse(&sample()).unwrap();
    let mut st = start(1, 7);
    st.options.flat = true;
    let a = Arena::new(&st, "me", true, &s).unwrap();
    let t = &a.play.rules.terrain;
    let h = t.heights[t.launch_col];
    assert!(t.heights.iter().all(|&v| v == h));
    assert!(t.water.cols.is_empty());
    assert!(!t.spots.is_empty(), "spots still land on level ground");
    // the same seed without the option keeps its hills
    let hilly = arena(1);
    let ht = &hilly.play.rules.terrain;
    assert!(ht.heights.iter().any(|&v| v != ht.heights[ht.launch_col]));
}

#[test]
fn bot_names_come_from_the_list_and_differ_by_seat() {
    let a = arena(3);
    let list = crate::app::bot_names::all();
    assert_eq!(a.names[0], "seat.you");
    for n in &a.names[1..] {
        assert!(list.contains(n), "{n} not in the list");
    }
    assert!(a.names[1] != a.names[2] && a.names[2] != a.names[3] && a.names[1] != a.names[3]);
}

#[test]
fn arena_an_eight_seat_second_siege_aims_from_your_trebuchet() {
    // R1.3200: the last siege's replay ran on into the next siege, so the
    // aim view sat over the replayed castle and the first Space only
    // skipped it
    let mut a = arena(7);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    let mut n = 0;
    while a.phase() == Phase::Siege && n < 100_000 {
        a.queue(Input::Ready);
        a.tick();
        n += 1;
    }
    to_siege(&mut a);
    assert_eq!(a.play.rules.round, 2);
    let base = a.my_field().unwrap().base;
    let [bx, _] = a.camera().px(base);
    assert!((0..1920 / 3).contains(&bx), "trebuchet at x {bx}");
    a.space(crate::play::ON_TICK);
    a.tick();
    assert!(
        a.my_field().unwrap().shooter.swing_from.is_some(),
        "the first Space swings"
    );
}

#[test]
fn arena_the_corner_window_frames_your_castle_box_not_the_whole_zone() {
    // R1.3202: it showed the whole build zone, the castle a few pixels wide
    let mut a = arena(7);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    let own = a.own_field().unwrap();
    let (cl, cr, ground, top) = a.castle_box(own);
    let cam = a.inset_camera(own);
    let [l, t] = cam.px(Vec2::new(cl, top));
    let [r, g] = cam.px(Vec2::new(cr, ground));
    assert!(
        l >= 0 && r <= INSET_W && t >= 0 && g <= INSET_H,
        "box {l},{t}..{r},{g}"
    );
    // the castle spans a good share of the window one way or the other
    assert!(
        (r - l) * 2 >= INSET_W || (g - t) * 2 >= INSET_H,
        "box {l},{t}..{r},{g}"
    );
}

#[test]
fn arena_a_swing_begun_as_the_siege_timer_ends_still_flies_before_the_round_ends() {
    // R1.3203: the round ended under a moving shot
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    a.space(crate::play::ON_TICK);
    a.tick();
    a.play.rules.ticks_left = 2;
    let round = a.play.rules.round;
    // the held swing throws by itself at the bottom of the arm
    let mut flew = false;
    for _ in 0..crate::physics::SWING_END + 600 {
        a.tick();
        if a.play.rules.round != round {
            break;
        }
        let f = a.my_field().unwrap();
        let w = &f.castle.world;
        flew |= f
            .shooter
            .live
            .iter()
            .filter_map(|&i| w.projectiles.get(i))
            .any(|b| b.alive);
    }
    assert!(flew, "the shot left the trebuchet");
    assert_ne!(a.play.rules.round, round, "the round ended once the shot stopped");
}

/// Your shot in the air, if any.
fn my_ball(a: &Arena) -> Option<crate::physics::balls::Ball> {
    let f = a.my_field()?;
    let w = &f.castle.world;
    f.shooter
        .live
        .iter()
        .filter_map(|&i| w.projectiles.get(i))
        .find(|b| b.alive)
        .cloned()
}

#[test]
fn arena_the_flight_view_follows_from_release_without_zooming_back_and_forth() {
    // R1.3204: the view jumped after a hold, zoomed in and out under the
    // shot and met the castle only after impact
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    let (cl, _, ground, top) = a.castle_box(a.my_field().unwrap());
    a.space(crate::play::ON_TICK);
    for _ in 0..crate::physics::SWING_END * 2 / 3 {
        a.tick();
    }
    a.space(crate::play::ON_TICK);
    let mut zoom: Vec<Fx> = Vec::new();
    let mut last = None;
    let mut peaked = false;
    let mut last_x = Fx::ZERO;
    for k in 0..600 {
        a.tick();
        let flying = a.flight.is_some_and(|fl| !fl.landed);
        let (Some(b), true) = (my_ball(&a), flying) else {
            if zoom.len() > 1 {
                break;
            }
            continue;
        };
        let cam = a.camera();
        zoom.push(cam.ppu);
        // in view from a few ticks after release; out of the bottom third
        // while it climbs
        if zoom.len() > 8 {
            let [x, y] = cam.px(b.pos);
            peaked |= b.vel.y <= Fx::ZERO;
            let low = if peaked { 1080 } else { 1080 * 2 / 3 };
            assert!(
                (0..1920).contains(&x) && (0..low).contains(&y),
                "tick {k}: ball at {x},{y}"
            );
        }
        last = Some(cam);
        last_x = b.pos.x;
    }
    assert!(zoom.len() > 20, "the shot flew {} ticks", zoom.len());
    // the zoom moves one way only
    let ins = zoom.windows(2).filter(|p| p[1] > p[0]).count();
    let outs = zoom.windows(2).filter(|p| p[1] < p[0]).count();
    assert!(ins == 0 || outs == 0, "zoomed in {ins} and out {outs} ticks");
    // the castle's front is in view on the last tick before impact, unless
    // the shot was already past its front (thicker, longer beams changed which
    // castle this seed's bot builds, R1.4303)
    let cam = last.unwrap();
    let [x, y] = cam.px(Vec2::new(cl, (ground + top) / 2));
    assert!(
        last_x > cl || ((0..1920).contains(&x) && (0..1080).contains(&y)),
        "castle front at {x},{y}"
    );
}

#[test]
fn arena_the_next_swing_waits_for_the_view_to_come_back_to_the_trebuchet() {
    // R1.3205
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    a.space(crate::play::ON_TICK);
    for _ in 0..crate::physics::SWING_END * 2 / 3 {
        a.tick();
    }
    a.space(crate::play::ON_TICK);
    a.tick();
    while a.landed_at.is_none() {
        a.tick();
    }
    a.space(crate::play::ON_TICK);
    a.tick();
    assert!(
        a.my_field().unwrap().shooter.swing_from.is_none(),
        "a swing began on the castle view"
    );
    let mut n = 0;
    while !a.camera_home() && n < 2000 {
        a.tick();
        n += 1;
    }
    assert!(n < 2000, "the view never came back");
    let base = a.my_field().unwrap().base;
    assert!(on_screen(&a, base));
    a.space(crate::play::ON_TICK);
    a.tick();
    assert!(
        a.my_field().unwrap().shooter.swing_from.is_some(),
        "the swing begins once home"
    );
}

/// The button ids and the text lines of the match HUD.
fn hud(a: &Arena) -> (Vec<String>, Vec<String>) {
    let s = Strings::parse(&sample()).unwrap();
    let mut ids = Vec::new();
    let mut lines = Vec::new();
    for i in a
        .panels(&s, &CardText::default())
        .iter()
        .flat_map(|p| p.items.clone())
    {
        match i {
            Item::Button { id, .. } => ids.push(id),
            Item::Lines { lines: l, .. } => lines.extend(l),
            _ => {}
        }
    }
    (ids, lines)
}

#[test]
fn arena_the_siege_hud_has_leave_and_no_ready() {
    // R1.3207
    let mut a = arena(1);
    to_siege(&mut a);
    let (ids, _) = hud(&a);
    assert!(!ids.contains(&"match.ready".to_string()), "{ids:?}");
    assert!(ids.contains(&"play.leave".to_string()), "{ids:?}");
}

#[test]
fn arena_a_cleared_castle_says_so_takes_no_more_shots_and_readies_you() {
    // R1.3206, R1.3207
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    let me = a.me;
    let s = a.play.siege.as_mut().unwrap();
    let f = s.fields.iter_mut().find(|f| f.attacker == me).unwrap();
    for b in f.castle.world.bodies.iter_mut() {
        b.alive = false;
    }
    let (_, lines) = hud(&a);
    assert!(lines.contains(&"hud.cleared".to_string()), "{lines:?}");
    a.space(crate::play::ON_TICK);
    a.tick();
    assert!(
        a.my_field().is_none_or(|f| f.shooter.swing_from.is_none()),
        "a swing began"
    );
    assert!(
        a.play.rules.player(me).unwrap().ready || a.phase() != Phase::Siege,
        "you were not readied"
    );
}

#[test]
fn arena_a_hit_on_your_castle_flashes_the_labeled_corner_window() {
    // R1.3208: the corner window read as an opponent's
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    to_siege(&mut a);
    a.tick();
    let (_, lines) = hud(&a);
    assert!(lines.contains(&"hud.replay".to_string()), "{lines:?}");
    assert!(!a.frame().inset.unwrap().hit);
    let me = a.me;
    let s = a.play.siege.as_mut().unwrap();
    let f = s.fields.iter_mut().find(|f| f.owner == me).unwrap();
    f.castle.world.bodies[0].alive = false;
    a.tick();
    assert!(a.frame().inset.unwrap().hit, "no flash");
    for _ in 0..INSET_FLASH_TICKS {
        a.tick();
    }
    assert!(!a.frame().inset.unwrap().hit, "the flash stays");
}

#[test]
fn arena_a_press_in_the_air_snaps_to_the_half_cell_grid() {
    let a = arena(1);
    let (l, _, _) = a.zone(0);
    let x0 = l + Fx::from_int(30);
    let y0 = Fx::from_int(a.ground(0, x0).floor_int() + 4);
    let c = Vec2::new(x0 + Fx::ratio(5, 16), y0 + Fx::ratio(5, 16));
    // the quarter-cell physics grid would give (x0 + 1/4, y0 + 1/4)
    assert_eq!(
        a.start_at(c),
        Vec2::new(x0 + Fx::ratio(1, 2), y0 + Fx::ratio(1, 2))
    );
}

#[test]
fn arena_a_press_near_a_quarter_cell_node_picks_that_node() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    let (l, _, _) = a.zone(0);
    let x = l + Fx::from_int(30);
    let g = snap(a.ground(0, x));
    let top = Vec2::new(x + Fx::ratio(1, 4), g + Fx::ratio(7, 4));
    a.queue(Input::PlaceSegment {
        piece: Piece::Segment(ledger::Material::Wood),
        a: grid_of(Vec2::new(x, g)),
        b: grid_of(top),
    });
    a.tick();
    assert!(a.is_node(top));
    // an eighth of a cell off: the half-cell grid alone would give (x, g + 2)
    let c = top + Vec2::new(-Fx::ratio(1, 8), Fx::ratio(1, 8));
    assert_eq!(a.start_at(c), top);
    // and the node is marked as the one a press picks
    a.cursor = Some(c);
    let cam = a.camera();
    let marks = a.marks_over(&cam);
    assert_eq!(marks.hover, Some(cam.px(top)));
    assert!(marks.near.contains(&cam.px(top)));
}

#[test]
fn arena_a_segment_you_cannot_afford_shows_the_gold_you_lack() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    a.need_text = "Need {n} gold".into();
    let (l, _, _) = a.zone(0);
    a.cursor = Some(Vec2::new(l + Fx::from_int(30), Fx::from_int(40)));
    a.tool = Tool::Beam(Piece::Segment(ledger::Material::Metal));
    let price = ledger::price(
        Piece::Segment(ledger::Material::Metal),
        &a.play.rules.players[0].mods,
    );
    let p = &mut a.play.rules.players[0];
    p.ledger.stock.metal = 0;
    p.purse.gold = 1;
    let cam = a.camera();
    let need = a.marks_over(&cam).need.expect("a Need mark");
    assert_eq!(need.text, format!("Need {} gold", price - 1));
    // with the gold, no mark
    a.play.rules.players[0].purse.gold = price;
    assert_eq!(a.marks_over(&cam).need, None);
}

#[test]
fn arena_test_runs_your_castle_live_then_drops_the_copy() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    let (l, _, _) = a.zone(0);
    let x = l + Fx::from_int(30);
    let g = snap(a.ground(0, x));
    a.queue(Input::PlaceSegment {
        piece: Piece::Segment(ledger::Material::Wood),
        a: grid_of(Vec2::new(x, g)),
        b: grid_of(Vec2::new(x, g + Fx::from_int(2))),
    });
    a.tick();
    a.act("match.test", "").unwrap();
    assert_eq!(a.trial.as_ref().map(|t| t.0.world.tick), Some(0));
    for _ in 0..10 {
        a.tick();
    }
    // the copy steps once a tick while the match runs on
    assert_eq!(a.trial.as_ref().map(|t| t.0.world.tick), Some(10));
    for _ in 10..crate::play::tune::TEST_RUN_TICKS {
        a.tick();
    }
    assert!(a.trial.is_none());
    assert!(a.testing);
}

/// Whether two world points are within a few raw fixed-point units.
fn close(a: Vec2, b: Vec2) -> bool {
    let tol = Fx(64);
    (a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol
}

#[test]
fn arena_view_wheel_zooms_in_at_the_cursor() {
    // R1.4800: one notch in is 9/8 the pixels a unit, and the world point
    // under the cursor stays under it
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    let before = a.camera();
    let under = before.world(700, 500);
    a.wheel("700,500,-1").unwrap();
    let after = a.camera();
    assert_eq!(after.ppu, before.ppu * 9 / 8);
    assert!(
        close(after.world(700, 500), under),
        "{under:?} moved to {:?}",
        after.world(700, 500)
    );
    // a notch out goes back to the auto view's zoom
    a.wheel("700,500,1").unwrap();
    assert!((a.camera().ppu - before.ppu).abs() <= Fx(2), "{:?}", a.camera());
}

#[test]
fn arena_view_zoom_stops_at_twelve_units_wide() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    for _ in 0..60 {
        a.wheel("960,540,-1").unwrap();
    }
    // 1920 pixels over 12 units
    assert_eq!(a.camera().ppu, Fx::from_int(160));
}

#[test]
fn arena_view_arrow_keys_pan_and_wasd_pans_in_the_build_view() {
    // R1.4800: 1200 pixels a second, so 100 ms held moves 120 pixels
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    a.pump(0, 0, None, None);
    let c0 = a.camera();
    a.pump(100, 1 << 13, None, None); // ArrowRight
    let c1 = a.camera();
    assert_eq!(c1.left, c0.left + Fx::from_int(120) / c0.ppu);
    assert_eq!(c1.bottom, c0.bottom);
    a.pump(200, 1 << 7, None, None); // W
    let c2 = a.camera();
    assert_eq!(c2.bottom, c1.bottom + Fx::from_int(120) / c0.ppu);
    a.pump(300, 1 << 16, None, None); // A
    assert_eq!(a.camera().left, c0.left);
    // the view goes no lower than the auto view's bottom
    a.pump(400, 1 << 15, None, None); // ArrowDown
    a.pump(500, 1 << 15, None, None);
    assert_eq!(a.camera().bottom, c0.bottom);
}

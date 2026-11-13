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
fn arena_a_press_and_release_in_build_places_a_wood_segment_through_play() {
    let mut a = arena(1);
    a.set_size("1920,1080").unwrap();
    let (l, _, _) = a.zone(0);
    let x = l + Fx::from_int(30);
    let g = snap(a.ground(0, x));
    // a ground point, then two cells up
    let cam = a.camera();
    let p0 = cam.px(Vec2::new(x, g + Fx::ratio(1, 16)));
    let p1 = cam.px(Vec2::new(x, g + Fx::from_int(2)));
    let before = a.play.castles[0].geo.len();
    a.down(&format!("{},{},0", p0[0], p0[1])).unwrap();
    a.up(&format!("{},{}", p1[0], p1[1])).unwrap();
    a.tick();
    assert_eq!(a.play.castles[0].geo.len(), before + 1);
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
fn arena_a_high_shot_widens_the_view_and_keeps_the_ground_in_it() {
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
        "ball off {:?}",
        a.camera_target()
    );
    let ground = crate::play::units::physics_terrain(&f.plot).height_at(ball.pos.x);
    assert!(
        on_screen(&a, Vec2::new(ball.pos.x, ground)),
        "ground off {:?}",
        a.camera()
    );
}

#[test]
fn arena_the_view_keeps_the_trebuchet_in_just_after_release() {
    // R1.1800: the whip and follow-through are seen before the view follows
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
        // still the aiming view, not the wider flight view
        let target = a.camera_target();
        assert!(
            target.width() <= Fx::from_int(CAM_AIM_MAX_W),
            "tick {k}: {target:?}"
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

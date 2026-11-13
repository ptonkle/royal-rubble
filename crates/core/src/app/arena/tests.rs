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
    let mut ticks = 0;
    let mut bot_pieces = 0;
    while !a.over() && ticks < 200_000 {
        match a.phase() {
            Phase::Siege => {
                let left = a.my_field().map_or(0, |f| f.shooter.shots_left());
                let swinging = a.my_field().is_some_and(|f| f.shooter.swing_from.is_some());
                if left == 0 {
                    a.queue(Input::Ready);
                } else if (!swinging && ticks % 90 == 0) || (swinging && ticks % 90 == 40) {
                    a.space();
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
    // a short swing throws nearly straight up
    a.space();
    for _ in 0..20 {
        a.tick();
    }
    a.space();
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

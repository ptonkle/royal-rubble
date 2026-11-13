use super::*;
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
                } else if !swinging && ticks % 90 == 0 {
                    a.space();
                } else if swinging && ticks % 90 == 40 {
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

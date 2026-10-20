// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A bot proves the level can be done, through the player's own input: it searches with the game
//! itself (copies of it, played a few ticks at a time with keys a player would press) for the way
//! to the next spark, then to the open door. It knows no coordinates: whatever it finds, it found
//! by playing. The way it finds is then played from the start in a fresh session, as a player
//! would play it, and must end at the open door.

use lantern::Lantern;
use polter::{Frame, Key, Session};
use std::collections::{BTreeSet, VecDeque};

/// What the bot can do: a few ticks of keys. Every move ends with the jump key up, so the next
/// one can press it again.
const TICKS: usize = 16;

fn moves() -> Vec<(&'static str, Vec<u64>)> {
    let (l, r, j) = (Key::A.bit(), Key::D.bit(), Key::Space.bit());
    let hold = |keys: u64, n: usize, then: u64| {
        let mut v = vec![keys; n];
        v.resize(TICKS, then);
        v
    };
    vec![
        ("right", vec![r; TICKS]),
        ("left", vec![l; TICKS]),
        ("jump right", hold(r | j, 14, r)),
        ("jump left", hold(l | j, 14, l)),
        ("jump", hold(j, 14, 0)),
        ("hop right", hold(r | j, 4, r)),
        ("hop left", hold(l | j, 4, l)),
        ("wait", vec![0; TICKS]),
    ]
}

fn game(s: &Session) -> &Lantern {
    s.game_as::<Lantern>().expect("LANTERN")
}

/// Where the game stands, coarsely: places already reached this way are not searched again.
fn place(g: &Lantern) -> (i32, i32, bool, usize) {
    (
        (g.x / 6.0) as i32,
        (g.y / 6.0) as i32,
        g.grounded,
        g.gathered(),
    )
}

/// The keys, tick by tick, from where the session is to the next spark (or, with all gathered,
/// through the door); none if there is no way.
fn search(session: &mut Session) -> Option<Vec<u64>> {
    let start = session.snapshot().expect("LANTERN copies itself");
    let before = game(session).gathered();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([(start, Vec::<u64>::new())]);
    let moves = moves();
    while let Some((at, path)) = queue.pop_front() {
        if path.len() > 4000 {
            break;
        }
        for (_, keys) in &moves {
            session.restore(&at);
            for &k in keys {
                session.step_frame(&Frame {
                    keys: k,
                    ..Frame::default()
                });
            }
            let g = game(session);
            let mut next = path.clone();
            next.extend_from_slice(keys);
            if g.gathered() > before || g.won {
                return Some(next);
            }
            if seen.insert(place(g)) {
                queue.push_back((session.snapshot().expect("a copy"), next));
            }
        }
    }
    None
}

#[test]
fn the_bot_finds_a_way_through_and_a_player_could_play_it() {
    let mut session = Session::new(Box::new(Lantern::new()), 1);
    // nobody looks at the bot's pictures
    session.ctx.picture = false;
    let total = game(&session).sparks.len();
    let mut keys = Vec::new();
    while !game(&session).won {
        let found = search(&mut session).unwrap_or_else(|| {
            let g = game(&session);
            panic!(
                "no way on from ({}, {}) with {} of {total} sparks",
                g.x,
                g.y,
                g.gathered()
            )
        });
        // go on from where the found way ends
        let at = session.snapshot().expect("a copy");
        session.restore(&at);
        keys.extend(found);
        let mut replay = Session::new(Box::new(Lantern::new()), 1);
        replay.ctx.picture = false;
        for &k in &keys {
            replay.step_frame(&Frame {
                keys: k,
                ..Frame::default()
            });
        }
        session = replay;
    }
    // played as a player would, from the start: every spark, the open door
    let mut player = Session::new(Box::new(Lantern::new()), 1);
    player.ctx.picture = false;
    for (tick, &k) in keys.iter().enumerate() {
        player.step_frame(&Frame {
            keys: k,
            ..Frame::default()
        });
        assert!(player.broken.is_empty(), "tick {tick}: {:?}", player.broken);
    }
    let g = game(&player);
    assert_eq!(g.gathered(), total);
    assert!(g.open && g.won);
    println!(
        "the bot's way: {} ticks ({:.1} s)",
        keys.len(),
        keys.len() as f64 / 60.0
    );
    assert!(keys.len() < 60 * 60, "a minute at most: {}", keys.len());
    // written as a haunt, for `lantern play --haunt` and the workshop
    if let Some(path) = std::env::var_os("LANTERN_BOT_HAUNT") {
        let mut out = String::from("# LANTERN: the bot's way through (tests/bot.rs)\n");
        let mut from = 0;
        for i in 1..=keys.len() {
            if i == keys.len() || keys[i] != keys[from] {
                if keys[from] != 0 {
                    let names: Vec<String> = Key::ALL
                        .iter()
                        .filter(|k| keys[from] & k.bit() != 0)
                        .map(|k| format!("key:{}", k.name()))
                        .collect();
                    out += &format!("{}-{} {}\n", from + 1, i, names.join(" "));
                }
                from = i;
            }
        }
        std::fs::write(path, out).unwrap();
    }
}

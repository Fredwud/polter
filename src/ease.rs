// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Movement that eases: curves from 0 to 1, tweens that run a value along one in ticks, springs
//! that follow a target. All in `polter::math`, so the same on every machine, and all plain
//! data, so they are part of a game's state and rewind with it.

use crate::math;

/// A curve from 0 at t = 0 to 1 at t = 1 (some overshoot or bounce on the way).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    Linear,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    InSine,
    OutSine,
    InOutSine,
    /// Pulls back a little before it goes.
    InBack,
    /// Overshoots a little and settles.
    OutBack,
    /// Lands and bounces, as a dropped ball.
    OutBounce,
    /// Overshoots and wobbles, as a spring let go.
    OutElastic,
}

impl Ease {
    pub const ALL: [Ease; 14] = [
        Ease::Linear,
        Ease::InQuad,
        Ease::OutQuad,
        Ease::InOutQuad,
        Ease::InCubic,
        Ease::OutCubic,
        Ease::InOutCubic,
        Ease::InSine,
        Ease::OutSine,
        Ease::InOutSine,
        Ease::InBack,
        Ease::OutBack,
        Ease::OutBounce,
        Ease::OutElastic,
    ];

    /// The curve at t (clamped to 0..1).
    pub fn at(self, t: f32) -> f32 {
        use std::f32::consts::PI;
        let t = t.clamp(0.0, 1.0);
        const BACK: f32 = 1.701_58;
        match self {
            Ease::Linear => t,
            Ease::InQuad => t * t,
            Ease::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Ease::InOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            Ease::InCubic => t * t * t,
            Ease::OutCubic => 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t),
            Ease::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - 4.0 * (1.0 - t) * (1.0 - t) * (1.0 - t)
                }
            }
            Ease::InSine => 1.0 - math::cos(t * PI / 2.0),
            Ease::OutSine => math::sin(t * PI / 2.0),
            Ease::InOutSine => (1.0 - math::cos(t * PI)) / 2.0,
            Ease::InBack => t * t * ((BACK + 1.0) * t - BACK),
            Ease::OutBack => {
                let u = t - 1.0;
                1.0 + u * u * ((BACK + 1.0) * u + BACK)
            }
            Ease::OutBounce => {
                let (n, d) = (7.5625, 2.75);
                if t < 1.0 / d {
                    n * t * t
                } else if t < 2.0 / d {
                    let u = t - 1.5 / d;
                    n * u * u + 0.75
                } else if t < 2.5 / d {
                    let u = t - 2.25 / d;
                    n * u * u + 0.9375
                } else {
                    let u = t - 2.625 / d;
                    n * u * u + 0.984_375
                }
            }
            Ease::OutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    math::exp2(-10.0 * t) * math::sin((t * 10.0 - 0.75) * (2.0 * PI / 3.0)) + 1.0
                }
            }
        }
    }
}

/// A value running from `from` to `to` along a curve, in `ticks` ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub ticks: u32,
    pub ease: Ease,
    /// Ticks run so far.
    pub at: u32,
}

impl Tween {
    pub fn new(from: f32, to: f32, ticks: u32, ease: Ease) -> Tween {
        Tween {
            from,
            to,
            ticks,
            ease,
            at: 0,
        }
    }

    /// One tick on; the value after it.
    pub fn tick(&mut self) -> f32 {
        self.at = (self.at + 1).min(self.ticks);
        self.value()
    }

    /// The value now: `from` before the first tick, exactly `to` once it is done.
    pub fn value(&self) -> f32 {
        if self.at >= self.ticks {
            return self.to;
        }
        let t = self.at as f32 / self.ticks as f32;
        self.from + (self.to - self.from) * self.ease.at(t)
    }

    pub fn done(&self) -> bool {
        self.at >= self.ticks
    }
}

/// A value that follows a target as if on a spring: `stiffness` pulls it, `damping` calms it
/// (critically damped at damping = 2 √stiffness: no overshoot, as fast as that allows).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    /// A spring at rest at `value`, critically damped with the given stiffness.
    pub fn new(value: f32, stiffness: f32) -> Spring {
        Spring {
            value,
            velocity: 0.0,
            target: value,
            stiffness,
            damping: 2.0 * stiffness.sqrt(),
        }
    }

    /// One tick (a 60th of a second); the value after it.
    pub fn tick(&mut self) -> f32 {
        let dt = 1.0 / 60.0;
        let force = self.stiffness * (self.target - self.value) - self.damping * self.velocity;
        self.velocity += force * dt;
        self.value += self.velocity * dt;
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curve_runs_from_0_to_1() {
        for e in Ease::ALL {
            assert!(e.at(0.0).abs() < 1e-6, "{e:?} at 0 is {}", e.at(0.0));
            assert!(
                (e.at(1.0) - 1.0).abs() < 1e-6,
                "{e:?} at 1 is {}",
                e.at(1.0)
            );
        }
        // the plain ones never go back
        for e in [
            Ease::Linear,
            Ease::InQuad,
            Ease::OutQuad,
            Ease::InOutQuad,
            Ease::InCubic,
            Ease::OutCubic,
            Ease::InOutCubic,
            Ease::InSine,
            Ease::OutSine,
            Ease::InOutSine,
            Ease::OutBounce,
        ] {
            if e == Ease::OutBounce {
                continue;
            }
            let mut last = 0.0;
            for k in 0..=100 {
                let v = e.at(k as f32 / 100.0);
                assert!(v >= last - 1e-6, "{e:?} goes back at {k}");
                last = v;
            }
        }
        // the ones that overshoot do
        assert!((0..=100).any(|k| Ease::OutBack.at(k as f32 / 100.0) > 1.0));
        assert!((0..=100).any(|k| Ease::InBack.at(k as f32 / 100.0) < 0.0));
    }

    #[test]
    fn a_tween_ends_exactly_on_its_target() {
        let mut t = Tween::new(10.0, -4.0, 30, Ease::OutBack);
        assert_eq!(t.value(), 10.0);
        let mut last = 0.0;
        for _ in 0..30 {
            last = t.tick();
        }
        assert_eq!(last, -4.0);
        assert!(t.done());
        assert_eq!(t.tick(), -4.0, "and stays there");
    }

    #[test]
    fn a_critically_damped_spring_settles_without_overshoot() {
        let mut s = Spring::new(0.0, 120.0);
        s.target = 100.0;
        let mut most: f32 = 0.0;
        for _ in 0..120 {
            most = most.max(s.tick());
        }
        assert!(most <= 100.0 + 1e-3, "overshot to {most}");
        assert!((s.value - 100.0).abs() < 0.5, "settled at {}", s.value);
    }
}

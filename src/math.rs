// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Maths that gives the same bits on every machine. The platform's `sin`, `exp`, `powf` and
//! their kin come from its maths library and differ in the last bit between macOS, Linux, Windows
//! and the web, and one bit in a game's state is a different game after a thousand ticks. These
//! are built from the operations IEEE 754 fixes exactly (+, −, ×, ÷, `sqrt`, rounding), worked in
//! `f64` and rounded once to `f32`: within one unit in the last place of the platform's own, and
//! the same everywhere.
//!
//! A game made by `polter new` has a *clippy.toml* that names the platform's functions, so a slip
//! is a warning when the game builds.
//!
//! ```
//! use polter::math;
//! let (s, c) = math::sin_cos(1.0);
//! assert!((s * s + c * c - 1.0).abs() < 1e-6);
//! assert_eq!(math::pow(2.0, 10.0), 1024.0);
//! ```

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, LN_2, LN_10, PI};

// π/2 in three parts, the first two with their low bits zero, so `k × part` is exact for
// |k| < 2²⁰: sin and cos keep their accuracy up to |x| ≈ 1.6 million (and stay deterministic
// beyond).
const PIO2_1: f64 = 1.570_796_326_734_125_6;
const PIO2_2: f64 = 6.077_100_506_303_966e-11;
const PIO2_3: f64 = 2.022_266_248_795_950_6e-21;
// ln 2 in two parts, the first with its low bits zero, so `k × LN2_HI` is exact.
const LN2_HI: f64 = 6.931_471_803_691_238e-1;
const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;
const INV_SQRT3: f64 = 0.577_350_269_189_625_8;
const TAN_PI_12: f64 = 0.267_949_192_431_122_7;

/// c₀ + x·(c₁ + x·(c₂ + …)), from the last coefficient back.
fn horner(x: f64, c: &[f64]) -> f64 {
    c.iter().rev().fold(0.0, |acc, &k| k + x * acc)
}

/// (−1)ⁿ/(2n+1)! for n = 0…7: sin r = r·Σ cₙ·r²ⁿ.
const SIN: [f64; 8] = [
    1.0,
    -1.0 / 6.0,
    1.0 / 120.0,
    -1.0 / 5_040.0,
    1.0 / 362_880.0,
    -1.0 / 39_916_800.0,
    1.0 / 6_227_020_800.0,
    -1.0 / 1_307_674_368_000.0,
];
/// (−1)ⁿ/(2n)! for n = 0…8: cos r = Σ cₙ·r²ⁿ.
const COS: [f64; 9] = [
    1.0,
    -1.0 / 2.0,
    1.0 / 24.0,
    -1.0 / 720.0,
    1.0 / 40_320.0,
    -1.0 / 3_628_800.0,
    1.0 / 479_001_600.0,
    -1.0 / 87_178_291_200.0,
    1.0 / 20_922_789_888_000.0,
];
/// 1/(n+1)! for n = 0…12: e^r − 1 = r·Σ cₙ·rⁿ.
const EXPM1: [f64; 13] = [
    1.0,
    1.0 / 2.0,
    1.0 / 6.0,
    1.0 / 24.0,
    1.0 / 120.0,
    1.0 / 720.0,
    1.0 / 5_040.0,
    1.0 / 40_320.0,
    1.0 / 362_880.0,
    1.0 / 3_628_800.0,
    1.0 / 39_916_800.0,
    1.0 / 479_001_600.0,
    1.0 / 6_227_020_800.0,
];
/// 1/(2n+1) for n = 0…11: 2·atanh s = 2s·Σ cₙ·s²ⁿ.
const ATANH: [f64; 12] = [
    1.0,
    1.0 / 3.0,
    1.0 / 5.0,
    1.0 / 7.0,
    1.0 / 9.0,
    1.0 / 11.0,
    1.0 / 13.0,
    1.0 / 15.0,
    1.0 / 17.0,
    1.0 / 19.0,
    1.0 / 21.0,
    1.0 / 23.0,
];

/// sin r and cos r for |r| ≤ π/4, as Taylor series (the first term left out is below 10⁻¹⁶).
fn sin_cos_small(r: f64) -> (f64, f64) {
    let z = r * r;
    (r * horner(z, &SIN), horner(z, &COS))
}

/// x = k·π/2 + r with |r| ≤ π/4: the quadrant k mod 4 and r.
fn reduce(x: f64) -> (i64, f64) {
    let k = (x * (2.0 / PI)).round();
    let r = ((x - k * PIO2_1) - k * PIO2_2) - k * PIO2_3;
    ((k as i64) & 3, r)
}

fn sin_cos64(x: f64) -> (f64, f64) {
    let (q, r) = reduce(x);
    let (s, c) = sin_cos_small(r);
    match q {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// The sine.
pub fn sin(x: f32) -> f32 {
    if x == 0.0 {
        return x;
    }
    sin_cos64(x as f64).0 as f32
}

/// The cosine.
pub fn cos(x: f32) -> f32 {
    sin_cos64(x as f64).1 as f32
}

/// Sine and cosine at once.
pub fn sin_cos(x: f32) -> (f32, f32) {
    let (s, c) = sin_cos64(x as f64);
    (if x == 0.0 { x } else { s as f32 }, c as f32)
}

/// The tangent.
pub fn tan(x: f32) -> f32 {
    if x == 0.0 {
        return x;
    }
    let (q, r) = reduce(x as f64);
    let (s, c) = sin_cos_small(r);
    (if q & 1 == 0 { s / c } else { -c / s }) as f32
}

/// e^r − 1 for |r| ≤ ln 2 / 2, as a Taylor series without its 1.
fn expm1_small(r: f64) -> f64 {
    r * horner(r, &EXPM1)
}

/// 2^k exactly, for k in f64's normal range.
fn two_to(k: i64) -> f64 {
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// e^x in f64 for the inputs that matter to an f32 (|x| ≤ 110: beyond, an f32 is 0 or infinite).
fn exp64(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > 110.0 {
        return f64::INFINITY;
    }
    if x < -110.0 {
        return 0.0;
    }
    let k = (x / LN_2).round();
    let r = (x - k * LN2_HI) - k * LN2_LO;
    (1.0 + expm1_small(r)) * two_to(k as i64)
}

/// e^x − 1, exact near 0 where e^x − 1 would cancel.
fn expm1_64(x: f64) -> f64 {
    if x.abs() <= LN_2 / 2.0 {
        expm1_small(x)
    } else {
        exp64(x) - 1.0
    }
}

/// e to the power of x.
pub fn exp(x: f32) -> f32 {
    exp64(x as f64) as f32
}

/// 2 to the power of x.
pub fn exp2(x: f32) -> f32 {
    let x = x as f64;
    if x.is_nan() {
        return x as f32;
    }
    if x > 160.0 {
        return f32::INFINITY;
    }
    if x < -160.0 {
        return 0.0;
    }
    let k = x.round();
    ((1.0 + expm1_small((x - k) * LN_2)) * two_to(k as i64)) as f32
}

/// e^x − 1, accurate for small x.
pub fn exp_m1(x: f32) -> f32 {
    if x == 0.0 {
        return x;
    }
    expm1_64(x as f64) as f32
}

/// ln((1 + s)/(1 − s)) = 2·atanh s, for |s| ≤ 0.172 (the series' first term left out is below
/// 10⁻¹⁸).
fn ln_ratio(s: f64) -> f64 {
    2.0 * s * horner(s * s, &ATANH)
}

/// ln x = e·ln 2 + ln m with m in [√½, √2): the exponent e and ln m.
fn ln_parts(x: f64) -> (f64, f64) {
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | (1023 << 52));
    if m > std::f64::consts::SQRT_2 {
        m /= 2.0;
        e += 1;
    }
    (e as f64, ln_ratio((m - 1.0) / (m + 1.0)))
}

/// ln x in f64 for a positive, finite x (an f32's value: always normal as an f64).
fn ln64(x: f64) -> f64 {
    let (e, lnm) = ln_parts(x);
    e * LN2_HI + (e * LN2_LO + lnm)
}

/// What ln, log2 and log10 give outside the positive numbers, if x is there.
fn log_special(x: f32) -> Option<f32> {
    if x.is_nan() || x < 0.0 {
        Some(f32::NAN)
    } else if x == 0.0 {
        Some(f32::NEG_INFINITY)
    } else if x == f32::INFINITY {
        Some(x)
    } else {
        None
    }
}

/// The natural logarithm.
pub fn ln(x: f32) -> f32 {
    log_special(x).unwrap_or_else(|| ln64(x as f64) as f32)
}

/// The logarithm to base 2 (exact for powers of two).
pub fn log2(x: f32) -> f32 {
    log_special(x).unwrap_or_else(|| {
        let (e, lnm) = ln_parts(x as f64);
        (e + lnm / LN_2) as f32
    })
}

/// The logarithm to base 10.
pub fn log10(x: f32) -> f32 {
    log_special(x).unwrap_or_else(|| (ln64(x as f64) / LN_10) as f32)
}

/// ln(1 + x), accurate for small x.
pub fn ln_1p(x: f32) -> f32 {
    if x.is_nan() || x < -1.0 {
        return f32::NAN;
    }
    if x == -1.0 {
        return f32::NEG_INFINITY;
    }
    if x == f32::INFINITY || x == 0.0 {
        return x;
    }
    let x = x as f64;
    if x > -0.29 && x < 0.41 {
        // 1 + x = (1 + s)/(1 − s) with s = x/(2 + x): no 1 + x, which would round small x away
        ln_ratio(x / (2.0 + x)) as f32
    } else {
        // 1 + x is exact in f64 for these
        ln64(1.0 + x) as f32
    }
}

/// x to the power of y.
pub fn pow(x: f32, y: f32) -> f32 {
    if y == 0.0 || x == 1.0 {
        return 1.0;
    }
    if x.is_nan() || y.is_nan() {
        return f32::NAN;
    }
    if y.is_infinite() {
        let a = x.abs();
        return if a == 1.0 {
            1.0
        } else if (a > 1.0) == (y > 0.0) {
            f32::INFINITY
        } else {
            0.0
        };
    }
    let integer = y == y.trunc();
    // odd only below 2²⁴: every larger f32 is even
    let odd = integer && y.abs() < 16_777_216.0 && (y as i64) & 1 == 1;
    if x == 0.0 {
        let zero = if y > 0.0 { 0.0 } else { f32::INFINITY };
        return if odd { zero.copysign(x) } else { zero };
    }
    if x.is_infinite() {
        let big = if y > 0.0 { f32::INFINITY } else { 0.0 };
        return if odd && x < 0.0 { -big } else { big };
    }
    if x < 0.0 && !integer {
        return f32::NAN;
    }
    let magnitude = exp64(y as f64 * ln64(x.abs() as f64)) as f32;
    if x < 0.0 && odd {
        -magnitude
    } else {
        magnitude
    }
}

/// x to the power of a whole number, by squaring (in f64, rounded once).
pub fn powi(x: f32, n: i32) -> f32 {
    let mut base = x as f64;
    let mut e = n.unsigned_abs();
    let mut out = 1.0f64;
    while e > 0 {
        if e & 1 == 1 {
            out *= base;
        }
        base *= base;
        e >>= 1;
    }
    (if n < 0 { 1.0 / out } else { out }) as f32
}

/// The cube root.
pub fn cbrt(x: f32) -> f32 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    (exp64(ln64(x.abs() as f64) / 3.0) as f32).copysign(x)
}

/// sqrt(x² + y²), without overflow.
pub fn hypot(x: f32, y: f32) -> f32 {
    if x.is_infinite() || y.is_infinite() {
        return f32::INFINITY;
    }
    let (x, y) = (x as f64, y as f64);
    (x * x + y * y).sqrt() as f32
}

/// atan in f64.
fn atan64(x: f64) -> f64 {
    let mut a = x.abs();
    let mut offset = 0.0;
    let inverted = a > 1.0;
    if inverted {
        a = 1.0 / a;
    }
    if a > TAN_PI_12 {
        a = (a - INV_SQRT3) / (1.0 + a * INV_SQRT3);
        offset = FRAC_PI_6;
    }
    // |a| ≤ tan(π/12): the series' first term left out is below 10⁻¹⁷
    let z = a * a;
    let mut series = 0.0;
    for n in (0..15).rev() {
        let term = 1.0 / (2 * n + 1) as f64;
        series = if n % 2 == 0 { term } else { -term } + z * series;
    }
    let mut out = offset + a * series;
    if inverted {
        out = FRAC_PI_2 - out;
    }
    out.copysign(x)
}

fn atan2_64(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if y == 0.0 {
        return if x.is_sign_positive() {
            y
        } else {
            PI.copysign(y)
        };
    }
    if x.is_infinite() {
        let a = if y.is_infinite() { FRAC_PI_4 } else { 0.0 };
        return if x > 0.0 {
            a.copysign(y)
        } else {
            (PI - a).copysign(y)
        };
    }
    if y.is_infinite() || x == 0.0 {
        return FRAC_PI_2.copysign(y);
    }
    let a = atan64(y / x);
    if x > 0.0 {
        a
    } else if y.is_sign_positive() {
        a + PI
    } else {
        a - PI
    }
}

/// The arc tangent.
pub fn atan(x: f32) -> f32 {
    atan64(x as f64) as f32
}

/// The angle of the point (x, y), in (−π, π].
pub fn atan2(y: f32, x: f32) -> f32 {
    atan2_64(y as f64, x as f64) as f32
}

/// The arc sine.
pub fn asin(x: f32) -> f32 {
    let x = x as f64;
    atan2_64(x, ((1.0 - x) * (1.0 + x)).sqrt()) as f32
}

/// The arc cosine.
pub fn acos(x: f32) -> f32 {
    let x = x as f64;
    atan2_64(((1.0 - x) * (1.0 + x)).sqrt(), x) as f32
}

/// The hyperbolic sine.
pub fn sinh(x: f32) -> f32 {
    if x == 0.0 {
        return x;
    }
    // (e^a − e^−a)/2 through e^a − 1, so small a doesn't cancel; worked for a = |x| (e^a − 1
    // near −1 would cancel) and given x's sign, so sinh(−x) = −sinh(x) exactly
    let m = expm1_64((x as f64).abs());
    let out = if m.is_infinite() {
        m
    } else {
        (m + m / (m + 1.0)) / 2.0
    };
    (out as f32).copysign(x)
}

/// The hyperbolic cosine.
pub fn cosh(x: f32) -> f32 {
    let e = exp64((x as f64).abs());
    ((e + 1.0 / e) / 2.0) as f32
}

/// The hyperbolic tangent.
pub fn tanh(x: f32) -> f32 {
    if x == 0.0 || x.is_nan() {
        return x;
    }
    if x.abs() > 20.0 {
        return 1.0f32.copysign(x);
    }
    // worked for |x|, so tanh(−x) = −tanh(x) exactly
    let m = expm1_64(2.0 * (x as f64).abs());
    ((m / (m + 2.0)) as f32).copysign(x)
}

#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "the platform's maths is the yardstick"
)]
mod tests {
    use super::*;

    /// How many f32s lie between a and b (0: the same; NaN and NaN are the same).
    fn ulps(a: f32, b: f32) -> u32 {
        if a.is_nan() && b.is_nan() {
            return 0;
        }
        if a == b {
            return if a.to_bits() == b.to_bits() { 0 } else { 1 };
        }
        let key = |v: f32| {
            let i = v.to_bits() as i32 as i64;
            if i < 0 { i32::MIN as i64 - i } else { i }
        };
        (key(a) - key(b)).unsigned_abs() as u32
    }

    /// Inputs from `lo` to `hi`: every f32 step through the range in `n` evenly spread values,
    /// plus the ends.
    fn spread(lo: f32, hi: f32, n: u32) -> Vec<f32> {
        (0..=n)
            .map(|i| lo + (hi - lo) * (i as f32 / n as f32))
            .collect()
    }

    /// Every finite positive f32 exponent, with mantissas across each.
    fn across_exponents() -> Vec<f32> {
        (1u32..0x7f80_0000)
            .step_by(0x0001_2345)
            .map(f32::from_bits)
            .collect()
    }

    /// The largest distance from the platform's function, computed in f64 and rounded once.
    fn worst(ours: impl Fn(f32) -> f32, theirs: impl Fn(f64) -> f64, inputs: &[f32]) -> (u32, f32) {
        inputs
            .iter()
            .map(|&x| (ulps(ours(x), theirs(x as f64) as f32), x))
            .max_by_key(|p| p.0)
            .unwrap()
    }

    #[test]
    fn within_one_last_place_of_the_platform() {
        let angles = spread(-10_000.0, 10_000.0, 200_000);
        let near = spread(-7.0, 7.0, 100_000);
        let unit = spread(-1.0, 1.0, 100_000);
        let positive = across_exponents();
        let wide = spread(-30.0, 30.0, 100_000);
        let cases: Vec<(&str, (u32, f32))> = vec![
            ("sin", worst(sin, f64::sin, &angles)),
            ("sin near", worst(sin, f64::sin, &near)),
            ("cos", worst(cos, f64::cos, &angles)),
            ("cos near", worst(cos, f64::cos, &near)),
            ("tan", worst(tan, f64::tan, &near)),
            ("exp", worst(exp, f64::exp, &spread(-103.0, 88.7, 200_000))),
            (
                "exp2",
                worst(exp2, f64::exp2, &spread(-149.0, 127.9, 200_000)),
            ),
            (
                "exp_m1",
                worst(exp_m1, f64::exp_m1, &spread(-2.0, 2.0, 100_000)),
            ),
            ("ln", worst(ln, f64::ln, &positive)),
            ("log2", worst(log2, f64::log2, &positive)),
            ("log10", worst(log10, f64::log10, &positive)),
            (
                "ln_1p",
                worst(ln_1p, f64::ln_1p, &spread(-0.99, 4.0, 100_000)),
            ),
            (
                "cbrt",
                worst(cbrt, f64::cbrt, &spread(-1000.0, 1000.0, 100_000)),
            ),
            ("atan", worst(atan, f64::atan, &wide)),
            ("asin", worst(asin, f64::asin, &unit)),
            ("acos", worst(acos, f64::acos, &unit)),
            ("sinh", worst(sinh, f64::sinh, &wide)),
            ("cosh", worst(cosh, f64::cosh, &wide)),
            ("tanh", worst(tanh, f64::tanh, &wide)),
        ];
        let off: Vec<_> = cases.iter().filter(|(_, (u, _))| *u > 1).collect();
        assert!(off.is_empty(), "more than one last place off: {off:?}");
    }

    #[test]
    fn two_argument_functions_within_one_last_place() {
        let grid = spread(-50.0, 50.0, 400);
        let mut worst_atan2 = (0, 0.0, 0.0);
        let mut worst_hypot = (0, 0.0, 0.0);
        for &y in &grid {
            for &x in &grid {
                let u = ulps(atan2(y, x), (y as f64).atan2(x as f64) as f32);
                if u > worst_atan2.0 {
                    worst_atan2 = (u, y, x);
                }
                let u = ulps(hypot(x, y), (x as f64).hypot(y as f64) as f32);
                if u > worst_hypot.0 {
                    worst_hypot = (u, x, y);
                }
            }
        }
        assert!(worst_atan2.0 <= 1, "atan2 {worst_atan2:?}");
        assert!(worst_hypot.0 <= 1, "hypot {worst_hypot:?}");
        let mut worst_pow = (0, 0.0, 0.0);
        for &x in &spread(0.01, 20.0, 400) {
            for &y in &spread(-12.0, 12.0, 400) {
                let u = ulps(pow(x, y), (x as f64).powf(y as f64) as f32);
                if u > worst_pow.0 {
                    worst_pow = (u, x, y);
                }
            }
        }
        assert!(worst_pow.0 <= 1, "pow {worst_pow:?}");
        for x in spread(-3.0, 3.0, 600) {
            for n in -12..=12 {
                let u = ulps(powi(x, n), (x as f64).powi(n) as f32);
                assert!(u <= 1, "powi({x}, {n}) is {u} last places off");
            }
        }
    }

    #[test]
    fn the_edges_are_the_platforms() {
        let specials = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.5,
            2.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            f32::MIN_POSITIVE,
            1e-40,
            f32::MAX,
        ];
        type One = (&'static str, fn(f32) -> f32, fn(f32) -> f32);
        let ones: [One; 17] = [
            ("sin", sin, f32::sin),
            ("cos", cos, f32::cos),
            ("tan", tan, f32::tan),
            ("exp", exp, f32::exp),
            ("exp2", exp2, f32::exp2),
            ("exp_m1", exp_m1, f32::exp_m1),
            ("ln", ln, f32::ln),
            ("log2", log2, f32::log2),
            ("log10", log10, f32::log10),
            ("ln_1p", ln_1p, f32::ln_1p),
            ("cbrt", cbrt, f32::cbrt),
            ("atan", atan, f32::atan),
            ("asin", asin, f32::asin),
            ("acos", acos, f32::acos),
            ("sinh", sinh, f32::sinh),
            ("cosh", cosh, f32::cosh),
            ("tanh", tanh, f32::tanh),
        ];
        for (name, ours, theirs) in ones {
            for x in specials {
                // sin, cos and tan of f32::MAX: both are some value in [−1, 1] (or any for tan)
                if x == f32::MAX && ["sin", "cos", "tan"].contains(&name) {
                    continue;
                }
                let (a, b) = (ours(x), theirs(x));
                assert!(
                    ulps(a, b) <= 1,
                    "{name}({x:e}): ours {a:e}, the platform's {b:e}"
                );
                if a == 0.0 || a.is_infinite() {
                    assert_eq!(a.to_bits(), b.to_bits(), "{name}({x:e}): the sign of {a}");
                }
            }
        }
        for y in specials {
            for x in specials {
                let pairs = [
                    ("atan2", atan2(y, x), y.atan2(x)),
                    ("pow", pow(y, x), y.powf(x)),
                    ("hypot", hypot(y, x), y.hypot(x)),
                ];
                for (name, a, b) in pairs {
                    if x == f32::MAX || y == f32::MAX {
                        continue;
                    }
                    assert!(
                        ulps(a, b) <= 1,
                        "{name}({y:e}, {x:e}): ours {a:e}, the platform's {b:e}"
                    );
                    if a == 0.0 || a.is_infinite() {
                        assert_eq!(
                            a.to_bits(),
                            b.to_bits(),
                            "{name}({y:e}, {x:e}): the sign of {a}"
                        );
                    }
                }
            }
        }
    }

    /// The bits themselves, hashed: the same on every machine, or the promise is broken. CI runs
    /// this test on Windows, Linux and macOS; here it also guards against a change that moves a bit.
    #[test]
    fn the_same_bits_everywhere() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: f32| {
            for b in v.to_bits().to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            }
        };
        for x in spread(-100.0, 100.0, 20_000) {
            eat(sin(x));
            eat(cos(x));
            eat(tan(x));
            eat(atan(x));
            eat(exp(x));
            eat(exp2(x));
            eat(exp_m1(x));
            eat(ln(x.abs()));
            eat(log2(x.abs()));
            eat(log10(x.abs()));
            eat(ln_1p(x.abs()));
            eat(cbrt(x));
            eat(asin(x / 100.0));
            eat(acos(x / 100.0));
            eat(sinh(x / 4.0));
            eat(cosh(x / 4.0));
            eat(tanh(x / 4.0));
            eat(atan2(x, 37.0 - x));
            eat(pow(x.abs(), x / 25.0));
            eat(powi(x / 50.0, (x as i32) % 9));
            eat(hypot(x, 3.0 * x - 7.0));
        }
        assert_eq!(h, 4_856_180_576_767_528_144, "the maths' bits");
    }
}

//! Fixed-point numbers for the physics: Q16.16 held in an `i64`, so the
//! product of two world values fits before the shift. Every operation is
//! integer arithmetic, so the same inputs give the same bits everywhere.

use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Fractional bits.
pub const FRAC_BITS: u32 = 16;

/// A Q16.16 fixed-point number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fx(pub i64);

impl Fx {
    pub const ZERO: Fx = Fx(0);
    pub const ONE: Fx = Fx(1 << FRAC_BITS);
    pub const HALF: Fx = Fx(1 << (FRAC_BITS - 1));

    pub const fn from_int(n: i64) -> Fx {
        Fx(n << FRAC_BITS)
    }

    /// `n / d` as a fixed-point number (`d` must not be zero).
    pub const fn ratio(n: i64, d: i64) -> Fx {
        Fx((n << FRAC_BITS) / d)
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    pub const fn abs(self) -> Fx {
        Fx(self.0.abs())
    }

    /// The whole part, rounded toward negative infinity.
    pub const fn floor_int(self) -> i64 {
        self.0 >> FRAC_BITS
    }

    pub fn sqrt(self) -> Fx {
        if self.0 <= 0 {
            return Fx::ZERO;
        }
        Fx(isqrt((self.0 as u64) << FRAC_BITS) as i64)
    }

    pub fn clamp_to(self, lo: Fx, hi: Fx) -> Fx {
        self.max(lo).min(hi)
    }

    pub fn lerp(a: Fx, b: Fx, t: Fx) -> Fx {
        a + (b - a) * t
    }
}

/// Integer square root, rounded down.
pub fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut bit: u64 = 1 << 62;
    while bit > n {
        bit >>= 2;
    }
    let mut num = n;
    let mut res: u64 = 0;
    while bit != 0 {
        if num >= res + bit {
            num -= res + bit;
            res = (res >> 1) + bit;
        } else {
            res >>= 1;
        }
        bit >>= 2;
    }
    res
}

impl Add for Fx {
    type Output = Fx;
    fn add(self, o: Fx) -> Fx {
        Fx(self.0 + o.0)
    }
}
impl Sub for Fx {
    type Output = Fx;
    fn sub(self, o: Fx) -> Fx {
        Fx(self.0 - o.0)
    }
}
impl AddAssign for Fx {
    fn add_assign(&mut self, o: Fx) {
        self.0 += o.0;
    }
}
impl SubAssign for Fx {
    fn sub_assign(&mut self, o: Fx) {
        self.0 -= o.0;
    }
}
impl Neg for Fx {
    type Output = Fx;
    fn neg(self) -> Fx {
        Fx(-self.0)
    }
}
impl Mul for Fx {
    type Output = Fx;
    fn mul(self, o: Fx) -> Fx {
        Fx((self.0 * o.0) >> FRAC_BITS)
    }
}
impl Mul<i64> for Fx {
    type Output = Fx;
    fn mul(self, o: i64) -> Fx {
        Fx(self.0 * o)
    }
}
/// Division by zero gives zero: the physics never wants a panic mid-siege.
impl Div for Fx {
    type Output = Fx;
    fn div(self, o: Fx) -> Fx {
        if o.0 == 0 {
            Fx::ZERO
        } else {
            Fx((self.0 << FRAC_BITS) / o.0)
        }
    }
}
impl Div<i64> for Fx {
    type Output = Fx;
    fn div(self, o: i64) -> Fx {
        if o == 0 {
            Fx::ZERO
        } else {
            Fx(self.0 / o)
        }
    }
}

/// A 2D vector of fixed-point numbers. `y` points up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vec2 {
    pub x: Fx,
    pub y: Fx,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 {
        x: Fx::ZERO,
        y: Fx::ZERO,
    };

    pub const fn new(x: Fx, y: Fx) -> Vec2 {
        Vec2 { x, y }
    }

    /// A vector from whole units.
    pub const fn ints(x: i64, y: i64) -> Vec2 {
        Vec2 {
            x: Fx::from_int(x),
            y: Fx::from_int(y),
        }
    }

    pub fn dot(self, o: Vec2) -> Fx {
        self.x * o.x + self.y * o.y
    }

    pub fn cross(self, o: Vec2) -> Fx {
        self.x * o.y - self.y * o.x
    }

    pub fn len2(self) -> Fx {
        self.dot(self)
    }

    pub fn len(self) -> Fx {
        self.len2().sqrt()
    }

    /// Rotated a quarter turn counterclockwise.
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }

    /// The unit vector, or zero for a zero vector.
    pub fn normalized(self) -> Vec2 {
        let l = self.len();
        if l == Fx::ZERO {
            Vec2::ZERO
        } else {
            Vec2::new(self.x / l, self.y / l)
        }
    }

    /// Rotated by the angle whose cosine and sine are `c` and `s`.
    pub fn rotate(self, c: Fx, s: Fx) -> Vec2 {
        Vec2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    pub fn lerp(a: Vec2, b: Vec2, t: Fx) -> Vec2 {
        a + (b - a) * t
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}
impl Mul<Fx> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: Fx) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}

/// Angles are in brads: a full turn is `TURN`.
pub const TURN: i64 = 1 << 16;
const QUARTER: i64 = TURN / 4;
/// Two pi in Q16.16.
const TWO_PI: i64 = 411_775;

/// Sine of an angle in brads, by a seventh-order series on the first quarter.
pub fn sin(angle: i64) -> Fx {
    let a = angle.rem_euclid(TURN);
    let q = a / QUARTER;
    let r = a % QUARTER;
    match q {
        0 => sin_quarter(r),
        1 => sin_quarter(QUARTER - r),
        2 => -sin_quarter(r),
        _ => -sin_quarter(QUARTER - r),
    }
}

pub fn cos(angle: i64) -> Fx {
    sin(angle + QUARTER)
}

/// Brads to radians, as a fixed-point number.
pub fn brads_to_radians(angle: i64) -> Fx {
    Fx(angle * TWO_PI / TURN)
}

fn sin_quarter(r: i64) -> Fx {
    let x = brads_to_radians(r);
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    let v = x - x3 / 6 + x5 / 120 - x7 / 5040;
    v.clamp_to(-Fx::ONE, Fx::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_sqrt_and_trig_hit_known_values() {
        assert_eq!(Fx::from_int(9).sqrt(), Fx::from_int(3));
        assert_eq!(sin(0), Fx::ZERO);
        assert!((sin(QUARTER) - Fx::ONE).abs() < Fx::ratio(1, 1000));
        // sin 30 degrees = 0.5, within 1/1000
        assert!((sin(TURN / 12) - Fx::HALF).abs() < Fx::ratio(1, 1000));
        assert!((cos(TURN / 2) + Fx::ONE).abs() < Fx::ratio(1, 1000));
    }
}

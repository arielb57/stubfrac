use std::cmp::Ordering;
use std::fmt;

use crate::Error;

/// An exact fraction with an `i64` numerator and a positive `i64` denominator,
/// always stored in lowest terms. Arithmetic is done in `i128` and fails with
/// [`Error::Overflow`] if the reduced result does not fit back into `i64`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rational {
    num: i64,
    den: i64,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Rational {
    pub const ZERO: Rational = Rational { num: 0, den: 1 };
    pub const ONE: Rational = Rational { num: 1, den: 1 };

    pub fn new(num: i64, den: i64) -> Result<Rational, Error> {
        Rational::from_i128(num as i128, den as i128)
    }

    fn from_i128(num: i128, den: i128) -> Result<Rational, Error> {
        if den == 0 {
            return Err(Error::ZeroDenominator);
        }
        let g = gcd(num, den);
        let sign = if den < 0 { -1 } else { 1 };
        let (num, den) = (sign * num / g, sign * den / g);
        match (i64::try_from(num), i64::try_from(den)) {
            (Ok(num), Ok(den)) => Ok(Rational { num, den }),
            _ => Err(Error::Overflow),
        }
    }

    pub fn integer(n: i64) -> Rational {
        Rational { num: n, den: 1 }
    }

    pub fn numer(self) -> i64 {
        self.num
    }

    pub fn denom(self) -> i64 {
        self.den
    }

    pub fn checked_add(self, other: Rational) -> Result<Rational, Error> {
        let g = gcd(self.den as i128, other.den as i128);
        let lcm = self.den as i128 / g * other.den as i128;
        Rational::from_i128(
            self.num as i128 * (lcm / self.den as i128)
                + other.num as i128 * (lcm / other.den as i128),
            lcm,
        )
    }

    pub fn checked_sub(self, other: Rational) -> Result<Rational, Error> {
        self.checked_add(Rational {
            num: -other.num,
            den: other.den,
        })
    }

    /// Decimal rendering rounded half away from zero, computed by exact long
    /// division so it never passes through a float.
    pub fn to_decimal(self, places: usize) -> String {
        let neg = self.num < 0;
        let num = (self.num as i128).abs();
        let den = self.den as i128;
        let scale = 10i128.pow(places as u32);
        let scaled = num * scale;
        let mut q = scaled / den;
        if (scaled % den) * 2 >= den {
            q += 1;
        }
        let int_part = q / scale;
        let frac_part = q % scale;
        let sign = if neg && q != 0 { "-" } else { "" };
        if places == 0 {
            format!("{sign}{int_part}")
        } else {
            format!("{sign}{int_part}.{frac_part:0places$}")
        }
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Rational) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Rational) -> Ordering {
        (self.num as i128 * other.den as i128).cmp(&(other.num as i128 * self.den as i128))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i64, d: i64) -> Rational {
        Rational::new(n, d).unwrap()
    }

    #[test]
    fn reduces_and_normalises_sign() {
        assert_eq!(r(6, -8), r(-3, 4));
        assert_eq!(r(-6, -8).numer(), 3);
        assert_eq!(r(0, 7), Rational::ZERO);
        assert_eq!(r(0, 7).denom(), 1);
        assert_eq!(Rational::new(1, 0), Err(Error::ZeroDenominator));
    }

    #[test]
    fn exact_arithmetic() {
        assert_eq!(
            r(61, 365).checked_add(r(121, 366)).unwrap(),
            r(66491, 133590)
        );
        assert_eq!(r(1, 2).checked_sub(r(1, 3)).unwrap(), r(1, 6));
        assert!(r(1, 3) < r(1, 2));
        assert!(r(-1, 2) < Rational::ZERO);
    }

    #[test]
    fn overflow_is_reported_not_wrapped() {
        let big = r(i64::MAX, 1);
        assert_eq!(big.checked_add(Rational::ONE), Err(Error::Overflow));
        // Consecutive integers are coprime, so the lcm is their product.
        let a = r(1, i64::MAX);
        let b = r(1, i64::MAX - 1);
        assert_eq!(a.checked_add(b), Err(Error::Overflow));
    }

    #[test]
    fn decimal_rendering_rounds_half_up() {
        assert_eq!(r(66491, 133590).to_decimal(5), "0.49772");
        assert_eq!(r(1, 8).to_decimal(2), "0.13");
        assert_eq!(r(-1, 8).to_decimal(2), "-0.13");
        assert_eq!(r(1, 1000).to_decimal(2), "0.00");
        assert_eq!(r(7, 2).to_decimal(0), "4");
        assert_eq!(r(337, 368).to_decimal(5), "0.91576");
    }
}

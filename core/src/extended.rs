//! Just enough of x87 80-bit `Extended` arithmetic (64-bit mantissa, round to nearest
//! even) to load data files like Free Pascal does: `StrToFloat` returns `Extended`, so
//! `-StrToFloat(s) * Scale / 1.1` rounds three times at 64 bits before the result is stored
//! in a `Single`. Doing it in f64 instead differs by one ulp for about 1.6% of the
//! animation coordinates.

/// A finite extended value: `mant * 2^exp`, `mant` normalized (top bit set) unless zero.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Ext {
    neg: bool,
    mant: u64,
    exp: i32,
}

/// Unsigned big integer (little-endian 32-bit limbs) for exact decimal conversion.
#[derive(Clone, Debug)]
struct Big(Vec<u32>);

impl Big {
    fn from_u128(mut v: u128) -> Big {
        let mut limbs = Vec::new();
        while v > 0 {
            limbs.push(v as u32);
            v >>= 32;
        }
        Big(limbs)
    }

    fn trim(mut self) -> Big {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
        self
    }

    fn is_zero(&self) -> bool {
        self.0.iter().all(|&l| l == 0)
    }

    fn bits(&self) -> i32 {
        match self.0.iter().rposition(|&l| l != 0) {
            Some(i) => i as i32 * 32 + (32 - self.0[i].leading_zeros() as i32),
            None => 0,
        }
    }

    fn mul_small(&self, m: u32) -> Big {
        let mut carry = 0u64;
        let mut out = Vec::with_capacity(self.0.len() + 1);
        for &l in &self.0 {
            let v = u64::from(l) * u64::from(m) + carry;
            out.push(v as u32);
            carry = v >> 32;
        }
        if carry > 0 {
            out.push(carry as u32);
        }
        Big(out)
    }

    fn pow10(n: u32) -> Big {
        (0..n).fold(Big::from_u128(1), |b, _| b.mul_small(10))
    }

    fn shl(&self, n: u32) -> Big {
        let (limbs, bits) = ((n / 32) as usize, n % 32);
        let mut out = vec![0u32; limbs];
        let mut carry = 0u32;
        for &l in &self.0 {
            if bits == 0 {
                out.push(l);
            } else {
                out.push((l << bits) | carry);
                carry = l >> (32 - bits);
            }
        }
        if carry > 0 {
            out.push(carry);
        }
        Big(out).trim()
    }

    fn bit(&self, i: i32) -> bool {
        let limb = (i / 32) as usize;
        limb < self.0.len() && (self.0[limb] >> (i % 32)) & 1 == 1
    }

    fn ge(&self, other: &Big) -> bool {
        let n = self.0.len().max(other.0.len());
        for i in (0..n).rev() {
            let (a, b) = (
                self.0.get(i).copied().unwrap_or(0),
                other.0.get(i).copied().unwrap_or(0),
            );
            if a != b {
                return a > b;
            }
        }
        true
    }

    fn sub(&self, other: &Big) -> Big {
        let mut borrow = 0i64;
        let mut out = Vec::with_capacity(self.0.len());
        for i in 0..self.0.len() {
            let mut v =
                i64::from(self.0[i]) - i64::from(other.0.get(i).copied().unwrap_or(0)) - borrow;
            borrow = 0;
            if v < 0 {
                v += 1 << 32;
                borrow = 1;
            }
            out.push(v as u32);
        }
        Big(out).trim()
    }

    fn to_u128(&self) -> Option<u128> {
        if self.bits() > 128 {
            return None;
        }
        Some(
            self.0
                .iter()
                .rev()
                .fold(0u128, |v, &l| (v << 32) | u128::from(l)),
        )
    }

    /// Long division, bit by bit: (quotient as u128, remainder is zero).
    fn div(&self, d: &Big) -> (u128, bool) {
        // remainders stay below the divisor: no allocations when it fits in 127 bits
        if d.bits() <= 127
            && let Some(d) = d.to_u128()
        {
            let (mut rem, mut q) = (0u128, 0u128);
            for i in (0..self.bits()).rev() {
                rem = (rem << 1) | u128::from(self.bit(i));
                q <<= 1;
                if rem >= d {
                    rem -= d;
                    q |= 1;
                }
            }
            return (q, rem == 0);
        }

        let mut rem = Big(Vec::new());
        let mut q = 0u128;
        for i in (0..self.bits()).rev() {
            rem = rem.shl(1);
            if self.bit(i) {
                if rem.0.is_empty() {
                    rem.0.push(1);
                } else {
                    rem.0[0] |= 1;
                }
            }
            q <<= 1;
            if rem.ge(d) {
                rem = rem.sub(d);
                q |= 1;
            }
        }
        (q, rem.is_zero())
    }
}

impl Ext {
    pub const ZERO: Ext = Ext {
        neg: false,
        mant: 0,
        exp: 0,
    };

    /// Rounds `q * 2^exp` (plus a sticky bit for a nonzero remainder) to 64 bits.
    fn round(neg: bool, q: u128, exp: i32, exact: bool) -> Ext {
        if q == 0 {
            return Ext::ZERO;
        }
        let bits = 128 - q.leading_zeros() as i32;
        if bits <= 64 {
            let shift = 64 - bits;
            return Ext {
                neg,
                mant: (q << shift) as u64,
                exp: exp - shift,
            };
        }
        let drop = bits - 64;
        let mut mant = q >> drop;
        let rest = q & ((1u128 << drop) - 1);
        let half = 1u128 << (drop - 1);
        if rest > half || (rest == half && (!exact || mant & 1 == 1)) {
            mant += 1;
        }
        let mut exp = exp + drop;
        if mant >> 64 != 0 {
            mant >>= 1;
            exp += 1;
        }
        Ext {
            neg,
            mant: mant as u64,
            exp,
        }
    }

    /// `StrToFloat`: a decimal number, correctly rounded to extended precision.
    pub fn parse(s: &str) -> Option<Ext> {
        let s = s.trim();
        let (neg, s) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s.strip_prefix('+').unwrap_or(s)),
        };
        let (number, exp10) = match s.find(['e', 'E']) {
            Some(i) => (&s[..i], s[i + 1..].parse::<i32>().ok()?),
            None => (s, 0),
        };
        let (int, frac) = number.split_once('.').unwrap_or((number, ""));
        if int.is_empty() && frac.is_empty() {
            return None;
        }
        let mut digits: u128 = 0;
        for c in int.chars().chain(frac.chars()) {
            digits = digits.checked_mul(10)? + u128::from(c.to_digit(10)?);
        }
        if digits == 0 {
            return Some(Ext::ZERO);
        }
        let exp10 = exp10 - frac.len() as i32;

        // fast path: digits / 10^k = digits / 5^k * 2^-k with everything in 128 bits
        if (-26..0).contains(&exp10) && digits < 1 << 64 {
            let k = -exp10;
            let den = 5u128.pow(k as u32);
            let num_bits = 128 - digits.leading_zeros() as i32;
            let den_bits = 128 - den.leading_zeros() as i32;
            let shift = 66 + den_bits - num_bits;
            if (0..128 - num_bits).contains(&shift) {
                let num = digits << shift;
                return Some(Ext::round(
                    neg,
                    num / den,
                    -shift - k,
                    num.is_multiple_of(den),
                ));
            }
        }

        let (num, den) = if exp10 >= 0 {
            (
                Big::from_u128(digits).mul_pow10(exp10 as u32),
                Big::from_u128(1),
            )
        } else {
            (Big::from_u128(digits), Big::pow10((-exp10) as u32))
        };

        // scale so the quotient has 66..=67 bits, then round with the remainder as sticky
        let shift = 66 + den.bits() - num.bits();
        let (q, exact) = if shift >= 0 {
            num.shl(shift as u32).div(&den)
        } else {
            num.div(&den.shl((-shift) as u32))
        };
        Some(Ext::round(neg, q, -shift, exact))
    }

    pub fn from_f32(v: f32) -> Ext {
        if v == 0.0 {
            return Ext::ZERO;
        }
        let bits = v.to_bits();
        let neg = bits >> 31 == 1;
        let e = ((bits >> 23) & 0xff) as i32;
        let (m, e) = if e == 0 {
            (bits & 0x7f_ffff, -149)
        } else {
            ((bits & 0x7f_ffff) | 0x80_0000, e - 150)
        };
        Ext::round(neg, u128::from(m), e, true)
    }

    pub fn from_i32(v: i32) -> Ext {
        Ext::round(v < 0, u128::from(v.unsigned_abs()), 0, true)
    }

    /// Stores the value in a `Single` (round to nearest even).
    pub fn to_f32(self) -> f32 {
        if self.mant == 0 {
            return 0.0;
        }
        let mut mant = self.mant >> 40;
        let rest = self.mant & ((1 << 40) - 1);
        let half = 1u64 << 39;
        if rest > half || (rest == half && mant & 1 == 1) {
            mant += 1;
        }
        // exact: at most 25 bits, and well inside the f64 exponent range
        let v = (mant as f64) * 2f64.powi(self.exp + 40);
        let v = v as f32;
        if self.neg { -v } else { v }
    }
}

impl std::ops::Neg for Ext {
    type Output = Ext;

    fn neg(self) -> Ext {
        Ext {
            neg: !self.neg,
            ..self
        }
    }
}

impl std::ops::Mul for Ext {
    type Output = Ext;

    fn mul(self, other: Ext) -> Ext {
        if self.mant == 0 || other.mant == 0 {
            return Ext::ZERO;
        }
        let q = u128::from(self.mant) * u128::from(other.mant);
        Ext::round(self.neg != other.neg, q, self.exp + other.exp, true)
    }
}

impl std::ops::Div for Ext {
    type Output = Ext;

    fn div(self, other: Ext) -> Ext {
        assert!(other.mant != 0, "extended division by zero");
        if self.mant == 0 {
            return Ext::ZERO;
        }
        let num = u128::from(self.mant) << 64;
        let den = u128::from(other.mant);
        let (q, r) = (num / den, num % den);
        Ext::round(self.neg != other.neg, q, self.exp - other.exp - 64, r == 0)
    }
}

impl Big {
    fn mul_pow10(&self, n: u32) -> Big {
        (0..n).fold(self.clone(), |b, _| b.mul_small(10))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_free_pascal_rounding() {
        // `-3 * StrToFloat(s)` stored in a Single, from anims/wipe.poa: f64 gets these one
        // ulp wrong; reference bits printed by a program compiled with fpc 3.2.2
        let z = |s: &str| {
            (Ext::from_i32(-3) * Ext::parse(s).unwrap())
                .to_f32()
                .to_bits()
        };
        assert_eq!(z("1.32294762134552"), 0xC07E0185);
        assert_eq!(z("4.9411301612854"), 0xC16D2C9B);
        assert_eq!(z("6.08528995513916"), 0xC1920C05);
        assert_ne!(
            (-3.0f64 * 1.322_947_621_345_52) as f32,
            f32::from_bits(0xC07E0185)
        );

        assert_eq!(Ext::parse("0.5").unwrap().to_f32(), 0.5);
        assert_eq!(Ext::parse("-2.5E-3").unwrap().to_f32(), -0.0025);
        assert_eq!(Ext::parse("12E2").unwrap().to_f32(), 1200.0);
        assert_eq!(Ext::from_f32(1.1).to_f32(), 1.1);
        assert_eq!((Ext::from_i32(3) * Ext::from_i32(7)).to_f32(), 21.0);
        assert_eq!((Ext::from_i32(1) / Ext::from_i32(4)).to_f32(), 0.25);
    }
}

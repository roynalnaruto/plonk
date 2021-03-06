//! This module contains an implementation of a polynomial in coefficient form
//! Where each coefficient is represented using a position in the underlying vector.
use super::{EvaluationDomain, Evaluations};
use crate::util;
use bls12_381::Scalar;
use rand::Rng;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};

use std::ops::{Add, AddAssign, Deref, DerefMut, Mul, Neg, Sub, SubAssign};

#[derive(Debug, Eq, PartialEq, Clone)]
/// Polynomial represents a polynomial in coeffiient form.
pub struct Polynomial {
    /// The coefficient of `x^i` is stored at location `i` in `self.coeffs`.
    pub coeffs: Vec<Scalar>,
}

impl Deref for Polynomial {
    type Target = [Scalar];

    fn deref(&self) -> &[Scalar] {
        &self.coeffs
    }
}

impl DerefMut for Polynomial {
    fn deref_mut(&mut self) -> &mut [Scalar] {
        &mut self.coeffs
    }
}

#[cfg(feature = "serde")]
use serde::{
    self, de::Visitor, ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer,
};

#[cfg(feature = "serde")]
impl Serialize for Polynomial {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_seq(Some(self.len()))?;
        for coeff in &self.coeffs {
            tup.serialize_element(&coeff)?;
        }
        tup.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Polynomial {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PolynomialVisitor;

        impl<'de> Visitor<'de> for PolynomialVisitor {
            type Value = Polynomial;

            fn expecting(&self, formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                formatter.write_str("a polynomial with valid scalars")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Polynomial, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut scalar_vec = Vec::new();
                // Visit each element in the inner array and push it onto
                // the existing vector.
                while let Some(elem) = seq.next_element()? {
                    scalar_vec.push(elem)
                }
                Ok(Polynomial::from_coefficients_vec(scalar_vec))
            }
        }

        deserializer.deserialize_seq(PolynomialVisitor)
    }
}

impl Polynomial {
    /// Returns the zero polynomial.
    pub fn zero() -> Self {
        Self { coeffs: Vec::new() }
    }

    /// Checks if the given polynomial is zero.
    pub fn is_zero(&self) -> bool {
        self.coeffs.is_empty() || self.coeffs.par_iter().all(|coeff| coeff == &Scalar::zero())
    }

    /// Constructs a new polynomial from a list of coefficients.
    pub fn from_coefficients_slice(coeffs: &[Scalar]) -> Self {
        Self::from_coefficients_vec(coeffs.to_vec())
    }

    /// Constructs a new polynomial from a list of coefficients.
    pub fn from_coefficients_vec(coeffs: Vec<Scalar>) -> Self {
        let mut result = Self { coeffs };
        // While there are zeros at the end of the coefficient vector, pop them off.
        result.truncate_leading_zeros();
        // Check that either the coefficients vec is empty or that the last coeff is
        // non-zero.
        assert!(result
            .coeffs
            .last()
            .map_or(true, |coeff| coeff != &Scalar::zero()));

        result
    }

    /// Returns the degree of the polynomial.
    pub fn degree(&self) -> usize {
        if self.is_zero() {
            return 0;
        }
        assert!(self
            .coeffs
            .last()
            .map_or(false, |coeff| coeff != &Scalar::zero()));
        self.coeffs.len() - 1
    }

    fn truncate_leading_zeros(&mut self) {
        while self.coeffs.last().map_or(false, |c| c == &Scalar::zero()) {
            self.coeffs.pop();
        }
    }
    /// Evaluates `self` at the given `point` in the field.
    pub fn evaluate(&self, point: &Scalar) -> Scalar {
        if self.is_zero() {
            return Scalar::zero();
        }

        // Compute powers of points
        let powers = util::powers_of(point, self.len());

        let p_evals: Vec<_> = self
            .par_iter()
            .zip(powers.into_par_iter())
            .map(|(c, p)| p * c)
            .collect();
        let mut sum = Scalar::zero();
        for eval in p_evals.into_iter() {
            sum += &eval;
        }
        sum
    }

    /// Outputs a polynomial of degree `d` where each coefficient is sampled
    /// uniformly at random from the field `F`.
    pub fn rand<R: Rng>(d: usize, mut rng: &mut R) -> Self {
        let mut random_coeffs = Vec::with_capacity(d + 1);
        for _ in 0..=d {
            random_coeffs.push(util::random_scalar(&mut rng));
        }
        Self::from_coefficients_vec(random_coeffs)
    }
}

use std::iter::Sum;

impl Sum for Polynomial {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        let sum: Polynomial = iter.fold(Polynomial::zero(), |mut res, val| {
            res = &res + &val;
            res
        });
        sum
    }
}

impl<'a, 'b> Add<&'a Polynomial> for &'b Polynomial {
    type Output = Polynomial;

    fn add(self, other: &'a Polynomial) -> Polynomial {
        let mut result = if self.is_zero() {
            other.clone()
        } else if other.is_zero() {
            self.clone()
        } else if self.degree() >= other.degree() {
            let mut result = self.clone();
            for (a, b) in result.coeffs.iter_mut().zip(&other.coeffs) {
                *a += b
            }
            result
        } else {
            let mut result = other.clone();
            for (a, b) in result.coeffs.iter_mut().zip(&self.coeffs) {
                *a += b
            }
            result
        };
        result.truncate_leading_zeros();
        result
    }
}

#[allow(dead_code)]
// Addition method tht uses iterators to add polynomials
// Benchmark this against the original method
fn iter_add(poly_a: &Polynomial, poly_b: &Polynomial) -> Polynomial {
    if poly_a.len() == 0 {
        return poly_b.clone();
    }
    if poly_b.len() == 0 {
        return poly_a.clone();
    }

    let max_len = std::cmp::max(poly_a.len(), poly_b.len());
    let min_len = std::cmp::min(poly_a.len(), poly_b.len());
    let mut data = Vec::with_capacity(max_len);
    let (mut poly_a_iter, mut poly_b_iter) = (poly_a.iter(), poly_b.iter());

    let partial_addition = poly_a_iter
        .by_ref()
        .zip(poly_b_iter.by_ref())
        .map(|(&a, &b)| a + b)
        .take(min_len);

    data.extend(partial_addition);
    data.extend(poly_a_iter);
    data.extend(poly_b_iter);

    assert_eq!(data.len(), std::cmp::max(poly_a.len(), poly_b.len()));

    let mut result = Polynomial::from_coefficients_vec(data);
    result.truncate_leading_zeros();
    result
}

impl<'a, 'b> AddAssign<&'a Polynomial> for Polynomial {
    fn add_assign(&mut self, other: &'a Polynomial) {
        if self.is_zero() {
            self.coeffs.truncate(0);
            self.coeffs.extend_from_slice(&other.coeffs);
        } else if other.is_zero() {
        } else if self.degree() >= other.degree() {
            for (a, b) in self.coeffs.iter_mut().zip(&other.coeffs) {
                *a += b
            }
        } else {
            // Add the necessary number of zero coefficients.
            self.coeffs.resize(other.coeffs.len(), Scalar::zero());
            for (a, b) in self.coeffs.iter_mut().zip(&other.coeffs) {
                *a += b
            }
            self.truncate_leading_zeros();
        }
    }
}

impl<'a, 'b> AddAssign<(Scalar, &'a Polynomial)> for Polynomial {
    fn add_assign(&mut self, (f, other): (Scalar, &'a Polynomial)) {
        if self.is_zero() {
            self.coeffs.truncate(0);
            self.coeffs.extend_from_slice(&other.coeffs);
            self.coeffs.iter_mut().for_each(|c| *c *= &f);
        } else if other.is_zero() {
        } else if self.degree() >= other.degree() {
            for (a, b) in self.coeffs.iter_mut().zip(&other.coeffs) {
                *a += &(f * b);
            }
        } else {
            // Add the necessary number of zero coefficients.
            self.coeffs.resize(other.coeffs.len(), Scalar::zero());
            for (a, b) in self.coeffs.iter_mut().zip(&other.coeffs) {
                *a += &(f * b);
            }
            self.truncate_leading_zeros();
        }
    }
}

impl Neg for Polynomial {
    type Output = Polynomial;

    #[inline]
    fn neg(mut self) -> Polynomial {
        for coeff in &mut self.coeffs {
            *coeff = -*coeff;
        }
        self
    }
}

impl<'a, 'b> Sub<&'a Polynomial> for &'b Polynomial {
    type Output = Polynomial;

    #[inline]
    fn sub(self, other: &'a Polynomial) -> Polynomial {
        let mut result = if self.is_zero() {
            let mut result = other.clone();
            for coeff in &mut result.coeffs {
                *coeff = -(*coeff);
            }
            result
        } else if other.is_zero() {
            self.clone()
        } else if self.degree() >= other.degree() {
            let mut result = self.clone();
            for (a, b) in result.coeffs.iter_mut().zip(&other.coeffs) {
                *a -= b
            }
            result
        } else {
            let mut result = self.clone();
            result.coeffs.resize(other.coeffs.len(), Scalar::zero());
            for (a, b) in result.coeffs.iter_mut().zip(&other.coeffs) {
                *a -= b;
            }
            result
        };
        result.truncate_leading_zeros();
        result
    }
}

impl<'a, 'b> SubAssign<&'a Polynomial> for Polynomial {
    #[inline]
    fn sub_assign(&mut self, other: &'a Polynomial) {
        if self.is_zero() {
            self.coeffs.resize(other.coeffs.len(), Scalar::zero());
            for (i, coeff) in other.coeffs.iter().enumerate() {
                self.coeffs[i] -= coeff;
            }
        } else if other.is_zero() {
        } else if self.degree() >= other.degree() {
            for (a, b) in self.coeffs.iter_mut().zip(&other.coeffs) {
                *a -= b
            }
        } else {
            // Add the necessary number of zero coefficients.
            self.coeffs.resize(other.coeffs.len(), Scalar::zero());
            for (a, b) in self.coeffs.iter_mut().zip(&other.coeffs) {
                *a -= b
            }
            // If the leading coefficient ends up being zero, pop it off.
            self.truncate_leading_zeros();
        }
    }
}

impl Polynomial {
    #[allow(dead_code)]
    #[inline]
    fn leading_coefficient(&self) -> Option<&Scalar> {
        self.last()
    }

    #[allow(dead_code)]
    #[inline]
    fn iter_with_index(&self) -> Vec<(usize, Scalar)> {
        self.iter().cloned().enumerate().collect()
    }

    /// Divides `self` by x-z using Ruffinis method
    pub fn ruffini(&self, z: Scalar) -> Polynomial {
        let mut quotient: Vec<Scalar> = Vec::with_capacity(self.degree());
        let mut k = Scalar::zero();

        // Reverse the results and use Ruffini's method to compute the quotient
        // The coefficients must be reversed as Ruffini's method
        // starts with the leading coefficient, while Polynomials
        // are stored in increasing order i.e. the leading coefficient is the last element
        for coeff in self.coeffs.iter().rev() {
            let t = coeff + k;
            quotient.push(t);
            k = z * t;
        }

        // Pop off the last element, it is the remainder term
        // For PLONK, we only care about perfect factors
        quotient.pop();

        // Reverse the results for storage in the Polynomial struct
        quotient.reverse();
        Polynomial::from_coefficients_vec(quotient)
    }
}

/// Performs O(nlogn) multiplication of polynomials if F is smooth.
impl<'a, 'b> Mul<&'a Polynomial> for &'b Polynomial {
    type Output = Polynomial;

    #[inline]
    fn mul(self, other: &'a Polynomial) -> Polynomial {
        if self.is_zero() || other.is_zero() {
            Polynomial::zero()
        } else {
            let domain = EvaluationDomain::new(self.coeffs.len() + other.coeffs.len())
                .expect("field is not smooth enough to construct domain");
            let mut self_evals = Evaluations::from_vec_and_domain(domain.fft(&self.coeffs), domain);
            let other_evals = Evaluations::from_vec_and_domain(domain.fft(&other.coeffs), domain);
            self_evals *= &other_evals;
            self_evals.interpolate()
        }
    }
}
/// Convenience Trait to multiply a scalar and polynomial
impl<'a, 'b> Mul<&'a Scalar> for &'b Polynomial {
    type Output = Polynomial;

    #[inline]
    fn mul(self, constant: &'a Scalar) -> Polynomial {
        if self.is_zero() || (constant == &Scalar::zero()) {
            return Polynomial::zero();
        }
        let scaled_coeffs: Vec<_> = self
            .coeffs
            .par_iter()
            .map(|coeff| coeff * constant)
            .collect();
        Polynomial::from_coefficients_vec(scaled_coeffs)
    }
}
/// Convenience Trait to sub a scalar and polynomial
impl<'a, 'b> Add<&'a Scalar> for &'b Polynomial {
    type Output = Polynomial;

    #[inline]
    fn add(self, constant: &'a Scalar) -> Polynomial {
        if self.is_zero() {
            return Polynomial::from_coefficients_vec(vec![*constant]);
        }
        let mut result = self.clone();
        if constant == &Scalar::zero() {
            return result;
        }

        result[0] += constant;
        result
    }
}
impl<'a, 'b> Sub<&'a Scalar> for &'b Polynomial {
    type Output = Polynomial;

    #[inline]
    fn sub(self, constant: &'a Scalar) -> Polynomial {
        let negated_constant = -constant;
        self + &negated_constant
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_ruffini() {
        // X^2 + 4X + 4
        let quadratic = Polynomial::from_coefficients_vec(vec![
            Scalar::from(4),
            Scalar::from(4),
            Scalar::one(),
        ]);
        // Divides X^2 + 4X + 4 by X+2
        let quotient = quadratic.ruffini(-Scalar::from(2));
        // X+2
        let expected_quotient =
            Polynomial::from_coefficients_vec(vec![Scalar::from(2), Scalar::one()]);
        assert_eq!(quotient, expected_quotient);
    }
    #[test]
    fn test_ruffini_zero() {
        // Tests the two situations where zero can be added to Ruffini:
        // (1) Zero polynomial in the divided
        // (2) Zero as the constant term for the polynomial you are dividing by
        // In the first case, we should get zero as the quotient
        // In the second case, this is the same as dividing by X
        // (1)
        //
        // Zero polynomial
        let zero = Polynomial::zero();
        // Quotient is invariant under any argument we pass
        let quotient = zero.ruffini(-Scalar::from(2));
        assert_eq!(quotient, Polynomial::zero());
        // (2)
        //
        // X^2 + X
        let p =
            Polynomial::from_coefficients_vec(vec![Scalar::zero(), Scalar::one(), Scalar::one()]);
        // Divides X^2 + X by X
        let quotient = p.ruffini(Scalar::zero());
        // X + 1
        let expected_quotient =
            Polynomial::from_coefficients_vec(vec![Scalar::one(), Scalar::one()]);
        assert_eq!(quotient, expected_quotient);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn poly_serialisation_roundtrip() {
        use bincode;
        let poly = Polynomial::from_coefficients_slice(&[
            Scalar::one(),
            Scalar::one(),
            Scalar::one(),
            Scalar::one(),
            Scalar::one(),
            Scalar::one(),
        ]);
        let ser = bincode::serialize(&poly).unwrap();
        let deser: Polynomial = bincode::deserialize(&ser).unwrap();

        assert!(&poly.coeffs[..] == &deser.coeffs[..]);
    }
}

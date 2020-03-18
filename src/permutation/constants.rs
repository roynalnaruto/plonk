use bls12_381::Scalar;

pub const K1: Scalar = Scalar::from_raw([7, 0, 0, 0]);
pub const K2: Scalar = Scalar::from_raw([13, 0, 0, 0]);
pub const K3: Scalar = Scalar::from_raw([17, 0, 0, 0]);

mod test {
    use super::*;

    fn legendre_symbol(scalar: &Scalar) -> bool {
        let min_one_half = [
            9223372034707292160u64,
            12240451741123816959u64,
            1845609449319885826u64,
            4176758429732224676u64,
        ];

        let min_one = -Scalar::one();
        let one = Scalar::one();
        let zero = Scalar::zero();
        scalar.pow(&min_one_half).eq(&min_one) ^ true
    }

    #[test]
    fn legendre_symbol_test() {
        let a = Scalar::from(7u64);
        assert!(!legendre_symbol(&a));
        let a = Scalar::from(6u64);
        assert!(legendre_symbol(&a));
    }
}

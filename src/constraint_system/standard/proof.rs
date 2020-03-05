/// For the proof, there are values,
/// chosen by the prover, which are committed
/// to for the given polynomials.
///
/// Then the verifier computes the evaluations
/// of the values for the given polynomials.
use super::linearisation_poly::ProofEvaluations;
use super::PreProcessedCircuit;
use crate::commitment_scheme::kzg10::{Commitment, VerifierKey};
use crate::fft::{EvaluationDomain, Polynomial};
use crate::permutation::constants::{K1, K2};
use crate::transcript::TranscriptProtocol;
use crate::util::{multiscalar_mul, sum_points};
use bls12_381::{pairing, G1Affine, G1Projective, Scalar};
pub struct Proof {
    // Commitment to the witness polynomial for the left wires
    pub a_comm: Commitment,
    // Commitment to the witness polynomial for the right wires
    pub b_comm: Commitment,
    // Commitment to the witness polynomial for the output wires
    pub c_comm: Commitment,

    // Commitment to the permutation polynomial
    pub z_comm: Commitment,

    // Commitment to the quotient polynomial
    pub t_lo_comm: Commitment,
    pub t_mid_comm: Commitment,
    pub t_hi_comm: Commitment,

    // Commitment to the opening polynomial
    pub w_z_comm: Commitment,
    // Commitment to the shifted opening polynomial
    pub w_zw_comm: Commitment,

    pub evaluations: ProofEvaluations,
}

impl Proof {
    pub fn empty() -> Proof {
        Proof {
            a_comm: Commitment::empty(),
            b_comm: Commitment::empty(),
            c_comm: Commitment::empty(),

            z_comm: Commitment::empty(),

            t_lo_comm: Commitment::empty(),
            t_mid_comm: Commitment::empty(),
            t_hi_comm: Commitment::empty(),

            w_z_comm: Commitment::empty(),
            w_zw_comm: Commitment::empty(),
            evaluations: ProofEvaluations {
                a_eval: Scalar::zero(),
                b_eval: Scalar::zero(),
                c_eval: Scalar::zero(),

                left_sigma_eval: Scalar::zero(),
                right_sigma_eval: Scalar::zero(),

                lin_poly_eval: Scalar::zero(),

                perm_eval: Scalar::zero(),
            },
        }
    }

    // Includes the commitments to the witness polynomials for left
    // right and output wires in the proof
    pub fn set_witness_poly_commitments(
        &mut self,
        a_comm: &Commitment,
        b_comm: &Commitment,
        c_comm: &Commitment,
    ) -> () {
        self.a_comm = *a_comm;
        self.b_comm = *b_comm;
        self.c_comm = *c_comm;
    }

    pub fn verify(
        &self,
        preprocessed_circuit: &PreProcessedCircuit,
        transcript: &mut dyn TranscriptProtocol,
        verifier_key: &VerifierKey,
        pub_inputs: &Vec<Scalar>,
    ) -> bool {
        let domain = EvaluationDomain::new(preprocessed_circuit.n).unwrap();

        // XXX: Check if components are valid

        // Add witness polynomials to transcript
        transcript.append_commitment(b"w_l", &self.a_comm);
        transcript.append_commitment(b"w_r", &self.b_comm);
        transcript.append_commitment(b"w_o", &self.c_comm);

        // Compute beta and gamma
        let beta = transcript.challenge_scalar(b"beta");
        transcript.append_scalar(b"beta", &beta);
        let gamma = transcript.challenge_scalar(b"gamma");
        // Add commitment to permutation polynomial to transcript
        transcript.append_commitment(b"z", &self.z_comm);
        // Compute quotient challenge
        let alpha = transcript.challenge_scalar(b"alpha");
        // Add commitment to quotient polynomial to transcript
        transcript.append_commitment(b"t_lo", &self.t_lo_comm);
        transcript.append_commitment(b"t_mid", &self.t_mid_comm);
        transcript.append_commitment(b"t_hi", &self.t_hi_comm);
        // Compute evaluation challenge
        let z_challenge = transcript.challenge_scalar(b"z");
        // Compute zero polynomial evaluated at `z_challenge`
        let z_h_eval = domain.evaluate_vanishing_polynomial(z_challenge);

        // Compute first lagrange polynomial evaluated at `z_challenge`
        let l1_eval = domain.evaluate_all_lagrange_coefficients(z_challenge)[0];

        // Compute the public input polynomial evaluated at `z_challenge`
        let pi_poly = Polynomial::from_coefficients_vec(domain.ifft(&pub_inputs));
        let pi_eval = pi_poly.evaluate(&z_challenge);
        // Compute quotient polynomial evaluated at `z_challenge`
        let t_eval = self.compute_quotient_evaluation(
            pi_eval,
            alpha,
            beta,
            gamma,
            l1_eval,
            z_h_eval,
            self.evaluations.perm_eval,
        );

        // Add evaluations to transcript
        transcript.append_scalar(b"a_eval", &self.evaluations.a_eval);
        transcript.append_scalar(b"b_eval", &self.evaluations.b_eval);
        transcript.append_scalar(b"c_eval", &self.evaluations.c_eval);
        transcript.append_scalar(b"left_sig_eval", &self.evaluations.left_sigma_eval);
        transcript.append_scalar(b"right_sig_eval", &self.evaluations.right_sigma_eval);
        transcript.append_scalar(b"perm_eval", &self.evaluations.perm_eval);
        transcript.append_scalar(b"t_eval", &t_eval);
        transcript.append_scalar(b"r_eval", &self.evaluations.lin_poly_eval);

        // Compute opening challenge
        let v = transcript.challenge_scalar(b"v");
        // Add commitment to openings to transcript
        transcript.append_commitment(b"w_z", &self.w_z_comm);
        transcript.append_commitment(b"w_z_w", &self.w_zw_comm);

        // Compute multi-point separation challenge
        let u = transcript.challenge_scalar(b"u");

        // Compute Partial Opening commitment
        let d_comm = self.compute_partial_opening_commitment(
            alpha,
            beta,
            gamma,
            z_challenge,
            u,
            v,
            l1_eval,
            &preprocessed_circuit,
        );

        // Compute batch opening commitment
        let f_comm = self.compute_batch_opening_commitment(
            z_challenge,
            v,
            G1Affine::from(d_comm),
            &preprocessed_circuit,
        );

        // Compute batch evaluation commitment
        let e_comm = self.compute_batch_evaluation_commitment(u, v, t_eval, &verifier_key);

        // Validate

        let lhs = pairing(
            &G1Affine::from(self.w_z_comm.0 + &self.w_zw_comm.0 * &u),
            &verifier_key.beta_h,
        );

        let inner = {
            let k_0 = self.w_z_comm.0 * z_challenge;

            let u_z_root = u * &z_challenge * &domain.group_gen;
            let k_1 = self.w_zw_comm.0 * u_z_root;

            k_0 + &k_1 + &f_comm - &e_comm
        };

        let rhs = pairing(&G1Affine::from(inner), &verifier_key.h);

        lhs == rhs
    }

    fn compute_quotient_evaluation(
        &self,
        pi_eval: Scalar,
        alpha: Scalar,
        beta: Scalar,
        gamma: Scalar,
        l1_eval: Scalar,
        z_h_eval: Scalar,
        z_hat_eval: Scalar,
    ) -> Scalar {
        let alpha_sq = alpha.square();
        let alpha_cu = alpha_sq * &alpha;

        // r + PI(z) * alpha
        let a = self.evaluations.lin_poly_eval + &(pi_eval * &alpha);

        // a + beta * sigma_1 + gamma
        let beta_sig1 = beta * &self.evaluations.left_sigma_eval;
        let b_0 = self.evaluations.a_eval + &beta_sig1 + &gamma;

        // b+ beta * sigma_2 + gamma
        let beta_sig2 = beta * &self.evaluations.right_sigma_eval;
        let b_1 = self.evaluations.b_eval + &beta_sig2 + &gamma;

        // ((c + gamma) * z_hat) * alpha^2
        let b_2 = (self.evaluations.c_eval + &gamma) * &z_hat_eval * &alpha_sq;

        let b = b_0 * &b_1 * &b_2;

        // l_1(z) * alpha^3
        let c = l1_eval * &alpha_cu;

        let t_eval = (a - &b - &c) * &z_h_eval.invert().unwrap();

        t_eval
    }

    fn compute_partial_opening_commitment(
        &self,
        alpha: Scalar,
        beta: Scalar,
        gamma: Scalar,
        z_challenge: Scalar,
        u: Scalar,
        v: Scalar,
        l1_eval: Scalar,
        preprocessed_circuit: &PreProcessedCircuit,
    ) -> G1Projective {
        let mut scalars: Vec<_> = Vec::with_capacity(6);
        let mut points: Vec<G1Affine> = Vec::with_capacity(6);

        scalars.push(self.evaluations.a_eval * &self.evaluations.b_eval * &alpha * &v);
        points.push(preprocessed_circuit.qm_comm().0);

        scalars.push(self.evaluations.a_eval * &alpha * &v);
        points.push(preprocessed_circuit.ql_comm().0);

        scalars.push(self.evaluations.b_eval * &alpha * &v);
        points.push(preprocessed_circuit.qr_comm().0);

        scalars.push(self.evaluations.c_eval * &alpha * &v);
        points.push(preprocessed_circuit.qo_comm().0);

        scalars.push(alpha * &v);
        points.push(preprocessed_circuit.qc_comm().0);

        // (a_eval + beta * z + gamma)(b_eval + beta * z * k1 + gamma)(c_eval + beta * k2* z + gamma) * alpha^2 * v
        let x = {
            let beta_z = beta * &z_challenge;
            let q_0 = self.evaluations.a_eval + &beta_z + &gamma;

            let beta_k1_z = beta * &K1 * &z_challenge;
            let q_1 = self.evaluations.b_eval + &beta_k1_z + &gamma;

            let beta_k2_z = beta * &K2 * &z_challenge;
            let q_2 = (self.evaluations.c_eval + &beta_k2_z + &gamma) * &alpha * &alpha * &v;

            q_0 * &q_1 * &q_2
        };

        // l1(z) * alpha^3 * v
        let r = l1_eval * &alpha.pow(&[3, 0, 0, 0]) * &v;
        // v^7* u
        let s = v.pow(&[7, 0, 0, 0]) * &u;

        scalars.push(x + &r + &s);
        points.push(self.z_comm.0);

        // (a_eval + beta * sigma_1_eval + gamma)(b_eval + beta * sigma_2_eval + gamma)(c_eval + beta * sigma_3_eval + gamma) * alpha^2 * v
        let y = {
            let beta_sigma_1 = beta * &self.evaluations.left_sigma_eval;
            let q_0 = self.evaluations.a_eval + &beta_sigma_1 + &gamma;

            let beta_sigma_2 = beta * &self.evaluations.right_sigma_eval;
            let q_1 = self.evaluations.b_eval + &beta_sigma_2 + &gamma;

            let q_2 = beta * &self.evaluations.perm_eval * &alpha * &alpha * &v;

            q_0 * &q_1 * &q_2
        };
        scalars.push(-y);
        points.push(preprocessed_circuit.out_sigma_comm().0);

        let points = multiscalar_mul(&scalars, &points);
        sum_points(&points)
    }
    fn compute_batch_opening_commitment(
        &self,
        z_challenge: Scalar,
        v: Scalar,
        d_comm: G1Affine,
        preprocessed_circuit: &PreProcessedCircuit,
    ) -> G1Projective {
        let mut scalars: Vec<_> = Vec::with_capacity(6);
        let mut points: Vec<G1Affine> = Vec::with_capacity(6);
        let n = preprocessed_circuit.n;

        let z_n = z_challenge.pow(&[n as u64, 0, 0, 0]);
        let z_two_n = z_challenge.pow(&[(2 * n) as u64, 0, 0, 0]);

        scalars.push(Scalar::one());
        points.push(self.t_lo_comm.0);

        scalars.push(z_n);
        points.push(self.t_mid_comm.0);

        scalars.push(z_two_n);
        points.push(self.t_hi_comm.0);

        scalars.push(Scalar::one());
        points.push(d_comm);

        scalars.push(v.pow(&[2, 0, 0, 0]));
        points.push(self.a_comm.0);

        scalars.push(v.pow(&[3, 0, 0, 0]));
        points.push(self.b_comm.0);

        scalars.push(v.pow(&[4, 0, 0, 0]));
        points.push(self.c_comm.0);

        scalars.push(v.pow(&[5, 0, 0, 0]));
        points.push(preprocessed_circuit.left_sigma_comm().0);

        scalars.push(v.pow(&[6, 0, 0, 0]));
        points.push(preprocessed_circuit.right_sigma_comm().0);

        let points = multiscalar_mul(&scalars, &points);
        sum_points(&points)
    }
    fn compute_batch_evaluation_commitment(
        &self,
        u: Scalar,
        v: Scalar,
        t_eval: Scalar,
        vk: &VerifierKey,
    ) -> G1Projective {
        let x = vec![
            (Scalar::one(), t_eval),
            (v, self.evaluations.lin_poly_eval),
            (v.pow(&[2, 0, 0, 0]), self.evaluations.a_eval),
            (v.pow(&[3, 0, 0, 0]), self.evaluations.b_eval),
            (v.pow(&[4, 0, 0, 0]), self.evaluations.c_eval),
            (v.pow(&[5, 0, 0, 0]), self.evaluations.left_sigma_eval),
            (v.pow(&[6, 0, 0, 0]), self.evaluations.right_sigma_eval),
            (v.pow(&[7, 0, 0, 0]), u * &self.evaluations.perm_eval),
        ];

        let mut result = Scalar::zero();
        for (i, j) in x.iter() {
            result += &(*i * j);
        }

        vk.g * result
    }
}

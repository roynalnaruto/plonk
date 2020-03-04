pub mod composer;
pub(crate) mod linearisation_poly; // XXX: change visibility to `mod linearisation_poly` we keep it like this for now, so that opening_poly won't complain
mod preprocessed_circuit;
mod proof;
mod quotient_poly;

use crate::commitment_scheme::kzg10::ProverKey;
use crate::fft::EvaluationDomain;
use crate::transcript::TranscriptProtocol;

pub use composer::StandardComposer;
pub use preprocessed_circuit::PreProcessedCircuit;

/// Implementation of the standard PLONK proof system

pub trait Composer {
    // `circuit_size` is the number of gates in the circuit
    fn circuit_size(&self) -> usize;
    // Preprocessing produces a preprocessed circuit
    fn preprocess(
        &mut self,
        commit_key: &ProverKey,
        transcript: &mut dyn TranscriptProtocol,
        domain: &EvaluationDomain,
    ) -> PreProcessedCircuit;
    fn prove(
        &mut self,
        commit_key: &ProverKey,
        preprocessed_circuit: &PreProcessedCircuit,
        transcript: &mut dyn TranscriptProtocol,
    ) -> proof::Proof;
}
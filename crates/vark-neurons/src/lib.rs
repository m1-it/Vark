pub mod network;
pub mod synapses;
pub use network::{LifNetwork, LifNeuron};
pub use synapses::CsrSynapses;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fires_and_recur() {
        let mut n = LifNetwork::new(2, 1.0, 1.0, 0);
        n.synapses.connect(1, 0, 2.0);
        let a = n.step(&[2.0, 0.0], 1.0);
        assert!(a[0]);
        let b = n.step(&[0.0, 0.0], 1.0);
        assert!(b[1]);
    }
}

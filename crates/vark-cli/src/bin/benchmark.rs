use std::time::Instant;
use vark_neurons::LifNetwork;
fn main() {
    for count in [10_000usize, 100_000, 1_000_000] {
        let start = Instant::now();
        let mut net = LifNetwork::new(count, 1.0, 20.0, 2);
        let input = vec![0.2; count];
        let spikes = net.simulate(&input, 1.0, 20);
        let elapsed = start.elapsed();
        println!(
            "neurons={} steps=20 spikes={} elapsed_ms={} bytes_state~{}",
            count,
            spikes.iter().filter(|x| **x).count(),
            elapsed.as_millis(),
            count * 32
        );
    }
}

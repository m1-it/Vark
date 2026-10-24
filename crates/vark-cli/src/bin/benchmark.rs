use std::{
    mem::size_of,
    time::{Duration, Instant},
};
use vark_neurons::LifNetwork;

const FAN_IN: usize = 10;
const WARMUP_STEPS: usize = 20;
const STEPS_PER_RUN: usize = 100;
const RUNS: usize = 5;

fn allocated_buffer_bytes(network: &LifNetwork) -> usize {
    let synapses = &network.synapses;
    network.neurons.capacity() * size_of::<vark_neurons::LifNeuron>()
        + network.last_spikes.capacity() * size_of::<bool>()
        + synapses.row_ptr.capacity() * size_of::<usize>()
        + synapses.col_idx.capacity() * size_of::<usize>()
        + synapses.weights.capacity() * size_of::<f32>()
        + synapses.out_row_ptr.capacity() * size_of::<usize>()
        + synapses.out_col_idx.capacity() * size_of::<usize>()
        + synapses.out_weights.capacity() * size_of::<f32>()
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn main() {
    for neuron_count in [256usize, 1_024, 4_096] {
        let mut network = LifNetwork::new(neuron_count, 1.0, 20.0, 2);
        network.connect_sparse_seeded(42, FAN_IN);
        let input = vec![2.0; neuron_count];
        for _ in 0..WARMUP_STEPS {
            network.step(&input, 1.0);
        }

        let mut samples = Vec::with_capacity(RUNS);
        let mut emitted_spikes = 0usize;
        for _ in 0..RUNS {
            let started = Instant::now();
            for _ in 0..STEPS_PER_RUN {
                emitted_spikes += network
                    .step(&input, 1.0)
                    .into_iter()
                    .filter(|spike| *spike)
                    .count();
            }
            samples.push(started.elapsed());
        }
        let elapsed = median(samples);
        let measured_steps = STEPS_PER_RUN * RUNS;
        println!(
            "neurons={neuron_count} fan_in={FAN_IN} csr_edges={} steps_per_run={STEPS_PER_RUN} runs={RUNS} median_ms={} median_us_per_step={} spikes={} owned_buffer_bytes={}",
            network.synapses.weights.len(),
            elapsed.as_millis(),
            elapsed.as_micros() / measured_steps as u128,
            emitted_spikes,
            allocated_buffer_bytes(&network),
        );
    }
}

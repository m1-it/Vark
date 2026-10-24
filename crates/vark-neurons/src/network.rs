use super::CsrSynapses;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifNeuron {
    pub voltage: f32,
    pub rest: f32,
    pub threshold: f32,
    pub tau: f32,
    pub refractory: u32,
    pub remaining_refractory: u32,
}
impl LifNeuron {
    pub fn new(threshold: f32, tau: f32, refractory: u32) -> Self {
        Self {
            voltage: 0.0,
            rest: 0.0,
            threshold,
            tau,
            refractory,
            remaining_refractory: 0,
        }
    }
    pub fn step(&mut self, input: f32, dt: f32) -> bool {
        if self.remaining_refractory > 0 {
            self.remaining_refractory -= 1;
            return false;
        }
        self.voltage += dt / self.tau * (-(self.voltage - self.rest) + input);
        if self.voltage >= self.threshold {
            self.voltage = self.rest;
            self.remaining_refractory = self.refractory;
            true
        } else {
            false
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifNetwork {
    pub neurons: Vec<LifNeuron>,
    pub synapses: CsrSynapses,
    pub last_spikes: Vec<bool>,
    pub timestep: u64,
    #[serde(default)]
    pub concept_assemblies: HashMap<u64, Vec<usize>>,
}
impl LifNetwork {
    pub fn new(n: usize, threshold: f32, tau: f32, refractory: u32) -> Self {
        Self {
            neurons: (0..n)
                .map(|_| LifNeuron::new(threshold, tau, refractory))
                .collect(),
            synapses: CsrSynapses::new(n),
            last_spikes: vec![false; n],
            timestep: 0,
            concept_assemblies: HashMap::new(),
        }
    }
    pub fn connect_sparse_seeded(&mut self, seed: u64, degree: usize) {
        let n = self.neurons.len();
        if n < 2 {
            return;
        }
        let mut state = seed | 1;
        let requested = degree.clamp(10, 50).min(n - 1);
        let mut edges = Vec::with_capacity(n * requested);
        for target in 0..n {
            let mut selected = Vec::with_capacity(requested);
            for _ in 0..requested {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let mut source = (state as usize) % n;
                if source == target {
                    source = (source + 1) % n;
                }
                while selected.contains(&source) {
                    source = (source + 1) % n;
                    if source == target {
                        source = (source + 1) % n;
                    }
                }
                selected.push(source);
                let weight = 0.15 + ((state >> 32) as f32 / u32::MAX as f32) * 0.25;
                edges.push((target, source, weight));
            }
        }
        self.synapses.batch_connect(&edges);
    }
    pub fn assembly_indices(&self, concept: u64, width: usize) -> Vec<usize> {
        let n = self.neurons.len();
        if n == 0 {
            return vec![];
        }
        let start = (concept as usize) % n;
        (0..width.max(1).min(n))
            .map(|i| (start + i * 31) % n)
            .collect()
    }
    pub fn assembly_for_concept(&mut self, concept: u64, width: usize) -> Vec<usize> {
        if let Some(existing) = self.concept_assemblies.get(&concept) {
            return existing.clone();
        }
        let n = self.neurons.len();
        if n == 0 {
            return vec![];
        }
        let width = width.max(1).min(n);
        let start = (self.concept_assemblies.len() * width) % n;
        let assembly = (0..width).map(|i| (start + i) % n).collect::<Vec<_>>();
        self.concept_assemblies.insert(concept, assembly.clone());
        assembly
    }
    pub fn step(&mut self, external: &[f32], dt: f32) -> Vec<bool> {
        self.synapses.propagate(&self.last_spikes);
        let spikes = (0..self.neurons.len())
            .map(|i| {
                self.neurons[i].step(
                    external.get(i).copied().unwrap_or(0.0) + self.synapses.take_current(i),
                    dt,
                )
            })
            .collect::<Vec<_>>();
        self.last_spikes = spikes.clone();
        self.timestep += 1;
        spikes
    }
    pub fn step_parallel(&mut self, external: &[f32], dt: f32) -> Vec<bool> {
        self.synapses.propagate(&self.last_spikes);
        let currents = (0..self.neurons.len())
            .map(|i| external.get(i).copied().unwrap_or(0.0) + self.synapses.take_current(i))
            .collect::<Vec<_>>();
        let spikes = self
            .neurons
            .par_iter_mut()
            .enumerate()
            .map(|(i, n)| n.step(currents[i], dt))
            .collect::<Vec<_>>();
        self.last_spikes = spikes.clone();
        self.timestep += 1;
        spikes
    }
    pub fn simulate(&mut self, external: &[f32], dt: f32, steps: usize) -> Vec<bool> {
        let mut out = vec![false; self.neurons.len()];
        for _ in 0..steps {
            out = self.step(external, dt)
        }
        out
    }
    pub fn simulate_trace(&mut self, external: &[f32], dt: f32, steps: usize) -> Vec<Vec<bool>> {
        (0..steps).map(|_| self.step(external, dt)).collect()
    }
    pub fn save_atomic(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = path.as_ref();
        if let Some(d) = p.parent() {
            fs::create_dir_all(d)?
        }
        let tmp = p.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec(self)?)?;
        fs::OpenOptions::new().read(true).open(&tmp)?.sync_all()?;
        fs::rename(tmp, p)?;
        Ok(())
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut n: Self = serde_json::from_slice(&fs::read(path)?)?;
        n.synapses.pending = vec![0.0; n.neurons.len()];
        n.synapses.rebuild_outgoing();
        Ok(n)
    }
}

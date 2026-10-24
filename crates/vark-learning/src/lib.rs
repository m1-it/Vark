use vark_neurons::LifNetwork;
pub mod consolidation;
pub mod training;
#[derive(Debug, Clone)]
pub struct StdpConfig {
    pub learning_rate: f32,
    pub tau_pre: f32,
    pub tau_post: f32,
    pub min_weight: f32,
    pub max_weight: f32,
}
impl Default for StdpConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            tau_pre: 20.0,
            tau_post: 20.0,
            min_weight: -1.0,
            max_weight: 1.0,
        }
    }
}
#[derive(Debug, Clone)]
pub struct StdpState {
    pub pre_trace: Vec<f32>,
    pub post_trace: Vec<f32>,
}
impl StdpState {
    pub fn new(n: usize) -> Self {
        Self {
            pre_trace: vec![0.0; n],
            post_trace: vec![0.0; n],
        }
    }
    pub fn apply(
        &mut self,
        network: &mut LifNetwork,
        pre: &[bool],
        post: &[bool],
        cfg: &StdpConfig,
        dt: f32,
    ) {
        let ep = (-dt / cfg.tau_pre).exp();
        let eo = (-dt / cfg.tau_post).exp();
        for x in &mut self.pre_trace {
            *x *= ep
        }
        for x in &mut self.post_trace {
            *x *= eo
        }
        for (i, s) in pre.iter().enumerate() {
            if *s {
                self.pre_trace[i] += 1.0
            }
        }
        for (i, s) in post.iter().enumerate() {
            if *s {
                self.post_trace[i] += 1.0
            }
        }
        for target in 0..network.synapses.neuron_count() {
            let a = network.synapses.row_ptr[target];
            let b = network.synapses.row_ptr[target + 1];
            for k in a..b {
                let source = network.synapses.col_idx[k];
                let dw = cfg.learning_rate
                    * (if post.get(target).copied().unwrap_or(false) {
                        self.pre_trace[source]
                    } else {
                        0.0
                    } - if pre.get(source).copied().unwrap_or(false) {
                        self.post_trace[target]
                    } else {
                        0.0
                    });
                network.synapses.weights[k] =
                    (network.synapses.weights[k] + dw).clamp(cfg.min_weight, cfg.max_weight);
            }
        }
    }
    pub fn apply_sequence(
        &mut self,
        network: &mut LifNetwork,
        pre_sequence: &[Vec<bool>],
        post_sequence: &[Vec<bool>],
        cfg: &StdpConfig,
        dt: f32,
    ) {
        for (pre, post) in pre_sequence.iter().zip(post_sequence) {
            self.apply(network, pre, post, cfg, dt);
        }
    }
    pub fn reward(&self, network: &mut LifNetwork, reward: f32) {
        for weight in &mut network.synapses.weights {
            *weight = (*weight + reward).clamp(-1.0, 1.0);
        }
    }
}
pub fn apply_stdp(network: &mut LifNetwork, pre: &[bool], post: &[bool], cfg: &StdpConfig) {
    let mut s = StdpState::new(network.neurons.len());
    s.apply(network, pre, post, cfg, 1.0)
}
pub fn homeostasis(network: &mut LifNetwork, target_rate: f32, observed: &[f32], rate: f32) {
    for (n, o) in network.neurons.iter_mut().zip(observed) {
        n.threshold = (n.threshold + (o - target_rate) * rate).clamp(0.05, 10.0)
    }
}

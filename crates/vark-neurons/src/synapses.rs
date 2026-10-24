use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrSynapses {
    pub row_ptr: Vec<usize>,
    pub col_idx: Vec<usize>,
    pub weights: Vec<f32>,
    pub out_row_ptr: Vec<usize>,
    pub out_col_idx: Vec<usize>,
    pub out_weights: Vec<f32>,
    #[serde(default = "default_max_fan_in")]
    pub max_fan_in: usize,
    #[serde(skip)]
    pub(crate) pending: Vec<f32>,
}
fn default_max_fan_in() -> usize {
    50
}
impl CsrSynapses {
    pub fn new(n: usize) -> Self {
        Self {
            row_ptr: vec![0; n + 1],
            col_idx: vec![],
            weights: vec![],
            out_row_ptr: vec![0; n + 1],
            out_col_idx: vec![],
            out_weights: vec![],
            pending: vec![0.0; n],
            max_fan_in: default_max_fan_in(),
        }
    }
    pub fn batch_connect(&mut self, edges: &[(usize, usize, f32)]) {
        let n = self.neuron_count();
        let mut rows = vec![Vec::<(usize, f32)>::new(); n];
        for &(target, source, weight) in edges {
            if target < n
                && source < n
                && rows[target].iter().all(|(s, _)| *s != source)
                && rows[target].len() < self.max_fan_in
            {
                rows[target].push((source, weight));
            }
        }
        self.row_ptr = vec![0; n + 1];
        for (index, row) in rows.iter_mut().enumerate() {
            row.sort_unstable_by_key(|(source, _)| *source);
            self.row_ptr[index + 1] = self.row_ptr[index] + row.len();
        }
        self.col_idx = rows
            .iter()
            .flat_map(|r| r.iter().map(|(s, _)| *s))
            .collect();
        self.weights = rows
            .iter()
            .flat_map(|r| r.iter().map(|(_, w)| *w))
            .collect();
        self.rebuild_outgoing();
    }
    pub fn connect(&mut self, target: usize, source: usize, weight: f32) {
        if target >= self.neuron_count()
            || source >= self.neuron_count()
            || self.row_ptr[target + 1] - self.row_ptr[target] >= self.max_fan_in
        {
            return;
        }
        let at = self.row_ptr[target + 1];
        self.col_idx.insert(at, source);
        self.weights.insert(at, weight);
        for x in self.row_ptr.iter_mut().skip(target + 1) {
            *x += 1
        }
        self.rebuild_outgoing()
    }
    pub fn grow_coactive(&mut self, active: &[bool], amount: f32) {
        let nodes = active
            .iter()
            .enumerate()
            .filter_map(|(i, on)| on.then_some(i))
            .collect::<Vec<_>>();
        for &target in &nodes {
            for &source in &nodes {
                if target == source {
                    continue;
                }
                if let Some(weight) = self.weight_mut(target, source) {
                    *weight = (*weight + amount).min(1.0);
                    continue;
                }
                let a = self.row_ptr[target];
                let b = self.row_ptr[target + 1];
                if b - a < self.max_fan_in {
                    self.connect(target, source, amount);
                } else if let Some((weakest, _)) = (a..b)
                    .map(|k| (k, self.weights[k]))
                    .min_by(|x, y| x.1.total_cmp(&y.1))
                {
                    self.col_idx[weakest] = source;
                    self.weights[weakest] = amount;
                }
            }
        }
        self.rebuild_outgoing();
    }
    pub fn rebuild_outgoing(&mut self) {
        let n = self.neuron_count();
        let mut edges = vec![Vec::<(usize, f32)>::new(); n];
        for t in 0..n {
            for k in self.row_ptr[t]..self.row_ptr[t + 1] {
                edges[self.col_idx[k]].push((t, self.weights[k]))
            }
        }
        self.out_row_ptr = vec![0; n + 1];
        for (index, outgoing) in edges.iter().enumerate() {
            self.out_row_ptr[index + 1] = self.out_row_ptr[index] + outgoing.len()
        }
        self.out_col_idx = edges.iter().flat_map(|e| e.iter().map(|x| x.0)).collect();
        self.out_weights = edges.iter().flat_map(|e| e.iter().map(|x| x.1)).collect()
    }
    pub fn propagate(&mut self, spikes: &[bool]) {
        self.pending.fill(0.0);
        for source in spikes
            .iter()
            .enumerate()
            .filter(|(_, v)| **v)
            .map(|(i, _)| i)
        {
            for k in self.out_row_ptr[source]..self.out_row_ptr[source + 1] {
                self.pending[self.out_col_idx[k]] += self.out_weights[k]
            }
        }
    }
    pub fn take_current(&mut self, target: usize) -> f32 {
        self.pending
            .get_mut(target)
            .map(|x| {
                let v = *x;
                *x = 0.0;
                v
            })
            .unwrap_or(0.0)
    }
    pub fn neuron_count(&self) -> usize {
        self.row_ptr.len().saturating_sub(1)
    }
    pub fn weight_mut(&mut self, target: usize, source: usize) -> Option<&mut f32> {
        let a = self.row_ptr[target];
        let b = self.row_ptr[target + 1];
        (a..b)
            .find(|k| self.col_idx[*k] == source)
            .map(|k| &mut self.weights[k])
    }
}

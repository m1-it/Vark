use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayItem {
    pub input: Vec<f32>,
    pub target: Vec<f32>,
    pub reward: f32,
    pub salience: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBuffer {
    pub capacity: usize,
    pub items: VecDeque<ReplayItem>,
}
impl ReplayBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            items: VecDeque::new(),
        }
    }
    pub fn push(&mut self, item: ReplayItem) {
        if item.reward.is_finite() && item.salience > 0.0 {
            if self.items.len() >= self.capacity {
                self.items.pop_front();
            }
            self.items.push_back(item)
        }
    }
    pub fn sample(&self, n: usize) -> Vec<ReplayItem> {
        self.items.iter().rev().take(n).cloned().collect()
    }
    pub fn consolidate(&self) -> f32 {
        self.items
            .iter()
            .map(|x| x.reward * x.salience)
            .sum::<f32>()
            / (self.items.len().max(1) as f32)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwcState {
    pub importance: Vec<f32>,
    pub reference: Vec<f32>,
    pub lambda: f32,
}
impl EwcState {
    pub fn penalty(&self, weights: &[f32]) -> f32 {
        weights
            .iter()
            .zip(&self.reference)
            .zip(&self.importance)
            .map(|((w, r), f)| self.lambda * f * (w - r).powi(2))
            .sum()
    }
    pub fn update(&mut self, weights: &[f32], gradients: &[f32], rate: f32) {
        for i in 0..weights.len().min(self.importance.len()) {
            self.importance[i] = (self.importance[i] + rate * gradients[i].abs()).min(1e6);
            self.reference[i] = weights[i]
        }
    }
}

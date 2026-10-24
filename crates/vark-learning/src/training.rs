use super::consolidation::{EwcState, ReplayBuffer, ReplayItem};
#[derive(Debug, Clone)]
pub struct Learner {
    pub weights: Vec<f32>,
    pub replay: ReplayBuffer,
    pub ewc: EwcState,
    pub learning_rate: f32,
    pub step: u64,
}
impl Learner {
    pub fn new(size: usize, capacity: usize, lambda: f32) -> Self {
        Self {
            weights: vec![0.0; size],
            replay: ReplayBuffer::new(capacity),
            ewc: EwcState {
                importance: vec![0.0; size],
                reference: vec![0.0; size],
                lambda,
            },
            learning_rate: 0.01,
            step: 0,
        }
    }
    pub fn predict(&self, input: &[f32]) -> f32 {
        self.weights.iter().zip(input).map(|(w, x)| w * x).sum()
    }
    pub fn train_step(&mut self, input: &[f32], target: f32, reward: f32, salience: f32) {
        let pred = self.predict(input);
        let err = pred - target;
        let penalty_grad = self
            .weights
            .iter()
            .zip(&self.ewc.reference)
            .zip(&self.ewc.importance)
            .map(|((w, r), f)| 2.0 * self.ewc.lambda * f * (w - r))
            .collect::<Vec<_>>();
        let gradients = input
            .iter()
            .enumerate()
            .map(|(i, x)| 2.0 * err * x + penalty_grad.get(i).copied().unwrap_or(0.0))
            .collect::<Vec<_>>();
        for (weight, gradient) in self.weights.iter_mut().zip(&gradients) {
            *weight -= self.learning_rate * gradient;
        }
        self.replay.push(ReplayItem {
            input: input.to_vec(),
            target: vec![target],
            reward,
            salience,
        });
        self.step += 1
    }
    pub fn sleep_consolidate(&mut self, epochs: usize) {
        for _ in 0..epochs.max(1) {
            for item in self.replay.sample(self.replay.capacity) {
                let target = item.target.first().copied().unwrap_or(0.0);
                self.train_step_without_replay(&item.input, target);
            }
        }
        let grads = self
            .weights
            .iter()
            .zip(&self.ewc.reference)
            .map(|(w, r)| (w - r).abs())
            .collect::<Vec<_>>();
        self.ewc.update(&self.weights, &grads, 0.1)
    }
    fn train_step_without_replay(&mut self, input: &[f32], target: f32) {
        let err = self.predict(input) - target;
        for (weight, value) in self.weights.iter_mut().zip(input) {
            *weight -= self.learning_rate * 2.0 * err * value;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn learns_and_consolidates() {
        let mut l = Learner::new(2, 8, 0.1);
        for _ in 0..100 {
            l.train_step(&[1.0, 0.0], 1.0, 1.0, 1.0)
        }
        let before = l.predict(&[1.0, 0.0]);
        assert!(before > 0.5);
        l.sleep_consolidate(2);
        assert!(l.ewc.importance.iter().any(|x| *x > 0.0));
    }
}

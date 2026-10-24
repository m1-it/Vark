use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use thiserror::Error;
#[derive(Debug, Error)]
pub enum UnigramError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnigramModel {
    pub pieces: HashMap<String, f64>,
    pub unk: String,
}
impl UnigramModel {
    pub fn train(texts: &[String], target_vocab: usize, epochs: usize) -> Self {
        let mut counts = HashMap::new();
        for text in texts {
            for word in text.to_lowercase().split_whitespace() {
                let chars = word.chars().collect::<Vec<_>>();
                for n in 1..=4 {
                    for i in 0..chars.len() {
                        if i + n <= chars.len() {
                            *counts
                                .entry(chars[i..i + n].iter().collect::<String>())
                                .or_insert(0usize) += 1
                        }
                    }
                }
            }
        }
        let total = counts.values().sum::<usize>().max(1) as f64;
        let mut pieces = counts
            .into_iter()
            .map(|(p, c)| (p, (c as f64 / total).ln()))
            .collect::<HashMap<_, _>>();
        for _ in 0..epochs.max(1) {
            let mut usage: HashMap<String, f64> = HashMap::new();
            for text in texts {
                for word in text.to_lowercase().split_whitespace() {
                    for p in Self::viterbi_with(&pieces, word) {
                        *usage.entry(p).or_default() += 1.0
                    }
                }
            }
            let z = usage.values().sum::<f64>().max(1.0);
            for (p, c) in usage {
                pieces.insert(p, (c / z).ln());
            }
            if pieces.len() > target_vocab {
                let mut ranked = pieces
                    .iter()
                    .map(|(p, s)| (p.clone(), *s))
                    .collect::<Vec<_>>();
                ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
                ranked.truncate(target_vocab);
                pieces = ranked.into_iter().collect();
            }
        }
        Self {
            pieces,
            unk: "<unk>".into(),
        }
    }
    fn viterbi_with(pieces: &HashMap<String, f64>, word: &str) -> Vec<String> {
        let chars = word.chars().collect::<Vec<_>>();
        let mut best = vec![(f64::NEG_INFINITY, Vec::<String>::new()); chars.len() + 1];
        best[0] = (0.0, vec![]);
        for i in 0..chars.len() {
            if !best[i].0.is_finite() {
                continue;
            }
            for j in i + 1..=chars.len() {
                let p = chars[i..j].iter().collect::<String>();
                if let Some(score) = pieces.get(&p) {
                    let candidate = best[i].0 + *score;
                    if candidate > best[j].0 {
                        let mut seq = best[i].1.clone();
                        seq.push(p);
                        best[j] = (candidate, seq)
                    }
                }
            }
            if !best[i + 1].0.is_finite() {
                let p = chars[i].to_string();
                let mut seq = best[i].1.clone();
                seq.push(p.clone());
                best[i + 1] = (best[i].0 - 20.0, seq)
            }
        }
        best[chars.len()].1.clone()
    }
    pub fn encode(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .flat_map(|w| {
                let p = Self::viterbi_with(&self.pieces, w);
                if p.is_empty() {
                    vec![self.unk.clone()]
                } else {
                    p
                }
            })
            .collect()
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), UnigramError> {
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, UnigramError> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segments() {
        let m = UnigramModel::train(&["bahasa bahasa sistem".into()], 32, 2);
        assert!(!m.encode("bahasa").is_empty());
    }
}

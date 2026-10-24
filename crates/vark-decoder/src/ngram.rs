use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NgramDecoder {
    pub order: usize,
    pub counts: HashMap<String, HashMap<String, u64>>,
    pub totals: HashMap<String, u64>,
    pub vocabulary: Vec<String>,
    pub discount: f64,
}
#[derive(Debug, Clone)]
struct Beam {
    words: Vec<String>,
    score: f64,
}
impl NgramDecoder {
    pub fn train(texts: &[String], order: usize) -> Self {
        let order = order.max(1);
        let mut counts = HashMap::new();
        let mut totals = HashMap::new();
        let mut vocab: HashMap<String, u64> = HashMap::new();
        for text in texts {
            let words = std::iter::once("<s>".to_string())
                .chain(text.split_whitespace().map(|x| x.to_lowercase()))
                .chain(std::iter::once("</s>".into()))
                .collect::<Vec<_>>();
            for i in 0..words.len() {
                let start = i.saturating_sub(order - 1);
                let ctx = words[start..i].join(" ");
                *counts
                    .entry(ctx.clone())
                    .or_insert_with(HashMap::new)
                    .entry(words[i].clone())
                    .or_default() += 1;
                *totals.entry(ctx).or_default() += 1;
                *vocab.entry(words[i].clone()).or_default() += 1
            }
        }
        let mut vocabulary = vocab.keys().cloned().collect::<Vec<_>>();
        vocabulary.sort();
        Self {
            order,
            counts,
            totals,
            vocabulary,
            discount: 0.75,
        }
    }
    fn probability(&self, ctx: &str, next: &str) -> f64 {
        let ctx_words = ctx.split_whitespace().collect::<Vec<_>>();
        let mut c = ctx_words.to_vec();
        let mut backoff = 0.4;
        loop {
            let key = c.join(" ");
            if let Some(row) = self.counts.get(&key) {
                let total = *self.totals.get(&key).unwrap_or(&1) as f64;
                let count = *row.get(next).unwrap_or(&0) as f64;
                let unique = row.len() as f64;
                let p = ((count - self.discount).max(0.0) / total)
                    + (self.discount * unique / total)
                        * ((self.vocabulary.len().max(1)) as f64).recip();
                return p.max(1e-9) * backoff;
            }
            if c.is_empty() {
                return 1.0 / (self.vocabulary.len().max(1) as f64);
            }
            c.remove(0);
            backoff *= 0.4
        }
    }
    pub fn generate(&self, prompt: &str, max_tokens: usize) -> String {
        self.generate_beam(prompt, max_tokens, 4)
            .into_iter()
            .next()
            .unwrap_or_else(|| prompt.into())
    }
    pub fn generate_beam(&self, prompt: &str, max_tokens: usize, width: usize) -> Vec<String> {
        let seed = prompt
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>();
        let mut beams = vec![Beam {
            words: seed,
            score: 0.0,
        }];
        for _ in 0..max_tokens {
            let mut next = Vec::new();
            for b in &beams {
                let start = b.words.len().saturating_sub(self.order - 1);
                let ctx = b.words[start..].join(" ");
                let choices = self
                    .counts
                    .get(&ctx)
                    .map(|r| r.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_else(|| self.vocabulary.iter().take(64).cloned().collect());
                for w in choices {
                    if w == "<s>" {
                        continue;
                    }
                    let mut words = b.words.clone();
                    words.push(w.clone());
                    next.push(Beam {
                        score: b.score + self.probability(&ctx, &w).ln(),
                        words,
                    })
                }
            }
            next.sort_by(|a, b| b.score.total_cmp(&a.score));
            next.truncate(width.max(1));
            beams = next;
            if beams
                .iter()
                .all(|b| b.words.last().map(|x| x == "</s>").unwrap_or(false))
            {
                break;
            }
        }
        beams
            .into_iter()
            .map(|b| {
                b.words
                    .into_iter()
                    .filter(|w| w != "</s>" && w != "<s>")
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }
    pub fn perplexity(&self, text: &str) -> f64 {
        let words = text
            .split_whitespace()
            .map(|x| x.to_lowercase())
            .collect::<Vec<_>>();
        if words.is_empty() {
            return 0.0;
        }
        let mut loss = 0.0;
        for i in 0..words.len() {
            let s = i.saturating_sub(self.order - 1);
            let ctx = words[s..i].join(" ");
            loss -= self.probability(&ctx, &words[i]).ln()
        }
        (loss / words.len() as f64).exp()
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn learns_and_scores() {
        let d = NgramDecoder::train(&["rust bahasa sistem".into(), "rust bahasa aman".into()], 3);
        assert!(d.generate("rust", 2).contains("bahasa"));
        assert!(d.perplexity("rust bahasa") > 0.0);
    }
}

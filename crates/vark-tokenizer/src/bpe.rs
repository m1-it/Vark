use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader},
    path::Path,
};
use thiserror::Error;
#[derive(Debug, Error)]
pub enum BpeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpeModel {
    pub vocab: Vec<String>,
    pub merges: Vec<(String, String)>,
}
impl BpeModel {
    pub fn train(texts: &[String], vocab_limit: usize) -> Self {
        let mut vocab = HashMap::<String, usize>::new();
        for t in texts {
            for w in t.to_lowercase().split_whitespace() {
                for c in w.chars() {
                    *vocab.entry(c.to_string()).or_default() += 1
                }
            }
        }
        let mut symbols = vocab.keys().cloned().collect::<Vec<_>>();
        symbols.sort();
        let mut merges = Vec::new();
        while symbols.len() < vocab_limit {
            let mut pairs = HashMap::<(String, String), usize>::new();
            for t in texts {
                let chars = t
                    .to_lowercase()
                    .split_whitespace()
                    .flat_map(|w| w.chars().map(|c| c.to_string()))
                    .collect::<Vec<_>>();
                for p in chars.windows(2) {
                    *pairs.entry((p[0].clone(), p[1].clone())).or_default() += 1
                }
            }
            if let Some((pair, _)) = pairs.into_iter().max_by_key(|(_, n)| *n) {
                let merged = format!("{}{}", pair.0, pair.1);
                if symbols.contains(&merged) {
                    break;
                }
                symbols.push(merged);
                merges.push(pair)
            } else {
                break;
            }
        }
        Self {
            vocab: symbols,
            merges,
        }
    }
    pub fn encode(&self, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for word in text.to_lowercase().split_whitespace() {
            let mut parts = word.chars().map(|c| c.to_string()).collect::<Vec<_>>();
            for (pair_a, pair_b) in &self.merges {
                let mut next = Vec::new();
                let mut i = 0;
                while i < parts.len() {
                    if i + 1 < parts.len() && parts[i] == *pair_a && parts[i + 1] == *pair_b {
                        next.push(format!("{}{}", pair_a, pair_b));
                        i += 2
                    } else {
                        next.push(parts[i].clone());
                        i += 1
                    }
                }
                parts = next
            }
            out.extend(parts);
            out.push(" ".into())
        }
        out
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), BpeError> {
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, BpeError> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusLine {
    pub language: String,
    pub text: String,
}
pub fn load_balanced(
    id_path: impl AsRef<Path>,
    en_path: impl AsRef<Path>,
    max_lines: usize,
) -> Result<Vec<CorpusLine>, BpeError> {
    let read = |p: &Path, lang: &str| -> Result<Vec<CorpusLine>, BpeError> {
        Ok(BufReader::new(fs::File::open(p)?)
            .lines()
            .take(max_lines)
            .filter_map(|l| l.ok())
            .filter(|s| !s.trim().is_empty())
            .map(|text| CorpusLine {
                language: lang.into(),
                text,
            })
            .collect())
    };
    let id = read(id_path.as_ref(), "id")?;
    let en = read(en_path.as_ref(), "en")?;
    let n = ((id.len().min(en.len() * 2) as f32) * 0.65 / 0.35).ceil() as usize;
    let take_id = id.len().min(n).min(max_lines);
    let take_en = en.len().min(max_lines - take_id);
    let mut out = id.into_iter().take(take_id).collect::<Vec<_>>();
    out.extend(en.into_iter().take(take_en));
    Ok(out)
}

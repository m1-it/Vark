use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
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
        let mut words = texts
            .iter()
            .flat_map(|text| {
                text.to_lowercase()
                    .split_whitespace()
                    .map(|word| word.chars().map(|ch| ch.to_string()).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut symbols = words
            .iter()
            .flatten()
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        symbols.sort();
        let mut symbol_set = symbols.iter().cloned().collect::<HashSet<_>>();
        let mut merges = Vec::new();
        while symbols.len() < vocab_limit {
            let mut pairs = BTreeMap::<(String, String), usize>::new();
            for word in &words {
                for pair in word.windows(2) {
                    *pairs.entry((pair[0].clone(), pair[1].clone())).or_default() += 1;
                }
            }
            let Some((pair, _)) = pairs
                .into_iter()
                .filter(|((left, right), _)| !symbol_set.contains(&format!("{left}{right}")))
                .max_by(|(left_pair, left_count), (right_pair, right_count)| {
                    left_count
                        .cmp(right_count)
                        .then_with(|| right_pair.cmp(left_pair))
                })
            else {
                break;
            };
            let merged = format!("{}{}", pair.0, pair.1);
            let mut merge_count = 0;
            for word in &mut words {
                let mut next = Vec::with_capacity(word.len());
                let mut index = 0;
                while index < word.len() {
                    if index + 1 < word.len() && word[index] == pair.0 && word[index + 1] == pair.1
                    {
                        next.push(merged.clone());
                        index += 2;
                        merge_count += 1;
                    } else {
                        next.push(word[index].clone());
                        index += 1;
                    }
                }
                *word = next;
            }
            if merge_count == 0 {
                break;
            }
            symbol_set.insert(merged.clone());
            symbols.push(merged);
            merges.push(pair);
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

#[cfg(test)]
mod bpe_training_tests {
    use super::*;

    #[test]
    fn training_applies_multiple_merges_to_each_word() {
        let model = BpeModel::train(&["abab abab".into()], 4);
        assert_eq!(
            model.merges,
            vec![("a".into(), "b".into()), ("ab".into(), "ab".into())]
        );
        assert_eq!(
            model.encode("abab"),
            vec!["abab".to_owned(), " ".to_owned()]
        );
    }

    #[test]
    fn training_does_not_merge_across_word_boundaries() {
        let model = BpeModel::train(&["a b".into()], 8);
        assert!(model.merges.is_empty());
        assert_eq!(
            model.encode("a b"),
            vec![
                "a".to_owned(),
                " ".to_owned(),
                "b".to_owned(),
                " ".to_owned()
            ]
        );
    }
}

use serde::{Deserialize, Serialize};
use vark_hashing::feature_index;
pub mod bpe;
pub mod unigram;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub text: String,
    pub id: u32,
}
#[derive(Debug, Clone)]
pub struct Tokenizer {
    pub vocab_buckets: usize,
}
impl Tokenizer {
    pub fn new(vocab_buckets: usize) -> Self {
        assert!(vocab_buckets > 0);
        Self { vocab_buckets }
    }
    pub fn tokenize(&self, text: &str) -> Vec<Token> {
        text.split_whitespace()
            .map(|raw| {
                let clean = raw
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
                    .to_lowercase();
                Token {
                    text: clean.clone(),
                    id: feature_index(&clean, self.vocab_buckets) as u32,
                }
            })
            .filter(|t| !t.text.is_empty())
            .collect()
    }
    pub fn encode(&self, text: &str) -> Vec<u32> {
        self.tokenize(text).into_iter().map(|t| t.id).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokenizes() {
        assert_eq!(Tokenizer::new(100).encode("Halo dunia").len(), 2);
    }
}

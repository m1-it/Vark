use vark_tokenizer::Tokenizer;
pub mod ngram;
pub use ngram::NgramDecoder;
pub struct Decoder {
    tokenizer: Tokenizer,
}
impl Decoder {
    pub fn new(vocab: usize) -> Self {
        Self {
            tokenizer: Tokenizer::new(vocab),
        }
    }
    pub fn decode_ids(&self, ids: &[u32]) -> String {
        ids.iter()
            .map(|id| format!("<{}>", id))
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn encode_and_format(&self, text: &str) -> String {
        self.decode_ids(&self.tokenizer.encode(text))
    }
}

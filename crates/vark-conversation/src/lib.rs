mod dialogue;
mod engine;
mod language;
mod types;
pub use engine::Conversation;
pub use types::{Answer, ConversationError, Sentence, Turn};
#[cfg(test)]
mod tests {
    use super::*;
    use vark_memory::EpisodicMemory;
    #[test]
    fn grounded_abstain_and_recover() {
        let p = std::env::temp_dir().join("vark_dialogue2.jsonl");
        let _ = std::fs::remove_file(&p);
        let mut c = Conversation::new(EpisodicMemory::open(&p).unwrap(), 64);
        c.ingest("Rust adalah bahasa sistem.", "dokumen", 1.0)
            .unwrap();
        assert!(c.respond("Apa Rust bahasa sistem?").unwrap().grounded);
        assert!(!c.respond("Siapa presiden Mars?").unwrap().grounded);
        let d = Conversation::new(EpisodicMemory::open(&p).unwrap(), 64);
        assert!(!d.memory.load_records().unwrap().is_empty());
    }
}

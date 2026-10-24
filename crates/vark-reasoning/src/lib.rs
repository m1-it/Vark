use thiserror::Error;
use vark_memory::{EpisodicMemory, MemoryError, MemoryRecord};
#[derive(Debug, Error)]
pub enum ReasoningError {
    #[error(transparent)]
    Memory(#[from] MemoryError),
}
pub struct Planner<'a> {
    pub memory: &'a EpisodicMemory,
}
impl<'a> Planner<'a> {
    pub fn new(memory: &'a EpisodicMemory) -> Self {
        Self { memory }
    }
    pub fn plan(&self, question: &str) -> Result<Vec<String>, ReasoningError> {
        let hits = self.memory.search(question, 16)?;
        let mut steps = vec![format!("Analisis pertanyaan: {}", question)];
        if hits.is_empty() {
            steps.push("Tidak ada memori dekat; susun jawaban dari input baru.".into())
        } else {
            steps.push(format!("Bandingkan {} memori relevan.", hits.len()));
        }
        steps.push("Verifikasi konsistensi sebelum keluaran.".into());
        Ok(steps)
    }
    pub fn retrieve(&self, q: &str) -> Result<Vec<MemoryRecord>, ReasoningError> {
        Ok(self.memory.search(q, 16)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_handles_empty_memory_conservatively() {
        let path =
            std::env::temp_dir().join(format!("vark-reasoning-empty-{}.jsonl", std::process::id()));
        let memory = EpisodicMemory::open(&path).unwrap();
        let planner = Planner::new(&memory);
        let plan = planner.plan("Siapa presiden Mars?").unwrap();
        assert!(plan
            .iter()
            .any(|step| step.contains("Tidak ada memori dekat")));
        assert!(plan
            .iter()
            .any(|step| step.contains("Verifikasi konsistensi")));
        assert!(planner.retrieve("Siapa presiden Mars?").unwrap().is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn planner_and_retriever_use_persisted_records() {
        let path =
            std::env::temp_dir().join(format!("vark-reasoning-fact-{}.jsonl", std::process::id()));
        let mut memory = EpisodicMemory::open(&path).unwrap();
        memory
            .append("Rust adalah bahasa pemrograman sistem", 1.0)
            .unwrap();
        let planner = Planner::new(&memory);
        let plan = planner
            .plan("Rust adalah bahasa pemrograman sistem")
            .unwrap();
        assert!(plan
            .iter()
            .any(|step| step.contains("Bandingkan 1 memori relevan")));
        assert_eq!(
            planner
                .retrieve("Rust adalah bahasa pemrograman sistem")
                .unwrap()
                .len(),
            1
        );
        std::fs::remove_file(path).unwrap();
    }
}

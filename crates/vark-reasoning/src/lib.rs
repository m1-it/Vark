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

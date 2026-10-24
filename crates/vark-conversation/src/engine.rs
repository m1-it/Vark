use crate::{
    language::{content_tokens, key_for, normalize, split_sentences},
    types::{ConversationError, FactStatus, Sentence, Turn},
};
use std::collections::HashMap;
use vark_config::VarkConfig;
use vark_decoder::NgramDecoder;
use vark_learning::StdpState;
use vark_llm::UsageStats;
use vark_llm::{OpenAiCompatible, TeacherStudentRouter};
use vark_memory::{
    semantic::{extract_relations, ContradictionChecker, KnowledgeGraph},
    ConversationLog, EpisodicMemory, MemoryKind,
};
use vark_neurons::LifNetwork;
use vark_tokenizer::Tokenizer;
use vark_tokenizer::{bpe::BpeModel, unigram::UnigramModel};
pub struct Conversation {
    pub memory: EpisodicMemory,
    pub(crate) tokenizer: Tokenizer,
    pub(crate) network: LifNetwork,
    pub(crate) stdp: StdpState,
    pub(crate) turns: Vec<Turn>,
    pub(crate) facts: HashMap<String, Sentence>,
    pub graph: KnowledgeGraph,
    pub(crate) max_turns: usize,
    pub(crate) simulation_steps: usize,
    pub(crate) llm: Option<OpenAiCompatible>,
    pub(crate) router: Option<TeacherStudentRouter>,
    pub(crate) user_id: String,
    pub(crate) teacher_confidence_threshold: f32,
    pub(crate) conversation_log: Option<ConversationLog>,
    pub(crate) last_student_draft: Option<String>,
    pub(crate) last_response_role: String,
    pub(crate) bpe: BpeModel,
    pub(crate) unigram: UnigramModel,
    pub(crate) decoder: NgramDecoder,
    pub(crate) checkpoint_dir: String,
}
impl Conversation {
    pub fn set_user_id(&mut self, user_id: impl Into<String>) {
        self.user_id = user_id.into();
    }
    pub fn routing_stats(&self) -> Option<UsageStats> {
        self.router.as_ref().map(|router| router.stats())
    }
    pub(crate) fn update_last_fact(
        &mut self,
        status: FactStatus,
    ) -> Result<usize, ConversationError> {
        let texts = self
            .turns
            .last()
            .map(|t| {
                t.evidence
                    .iter()
                    .map(|s| s.text.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut changed = 0;
        for sentence in self
            .facts
            .values_mut()
            .filter(|s| texts.iter().any(|t| t == &s.text))
        {
            sentence.status = status.clone();
            if status == FactStatus::Confirmed {
                sentence.confirmations += 1;
            }
            if matches!(status, FactStatus::Refuted | FactStatus::Retracted) {
                let relations = extract_relations(&sentence.text, &sentence.source);
                self.graph
                    .relations
                    .retain(|stored| !relations.iter().any(|r| r == stored));
            }
            self.memory.append_kind(
                &sentence.text,
                sentence.confidence,
                MemoryKind::Fact,
                match status {
                    FactStatus::Confirmed => "confirm",
                    FactStatus::Refuted => "rebut",
                    FactStatus::Retracted => "retract",
                    FactStatus::Tentative => "tentative",
                    FactStatus::Claimed => "claim",
                },
            )?;
            changed += 1;
        }
        if changed > 0 {
            if let Some(log) = &mut self.conversation_log {
                log.append(
                    &self.user_id,
                    "feedback",
                    &format!("status={:?}", status),
                    Some(&format!("{:?}", status)),
                )
                .map_err(ConversationError::Memory)?;
            }
            self.stdp.reward(
                &mut self.network,
                if status == FactStatus::Confirmed {
                    0.02
                } else if status == FactStatus::Refuted {
                    -0.02
                } else {
                    0.0
                },
            );
        }
        Ok(changed)
    }
    pub fn new(memory: EpisodicMemory, vocab: usize) -> Self {
        let mut facts = HashMap::new();
        let mut graph = KnowledgeGraph::new();
        if let Ok(records) = memory.load_records() {
            for r in records
                .into_iter()
                .filter(|r| r.kind == MemoryKind::Fact && !r.text.starts_with("Pengguna:"))
            {
                for raw in r.text.split(['.', '!', '?', '\n']) {
                    let n = raw.trim().to_lowercase();
                    let tok = Tokenizer::new(vocab)
                        .tokenize(&n)
                        .into_iter()
                        .map(|x| x.text)
                        .filter(|x| x.len() > 2)
                        .collect::<Vec<_>>();
                    if !tok.is_empty() {
                        facts.insert(
                            key_for(&tok),
                            Sentence {
                                text: raw.trim().into(),
                                tokens: tok,
                                source: if r.source.is_empty() {
                                    "memori dipulihkan".into()
                                } else {
                                    r.source.clone()
                                },
                                confidence: r.salience,
                                status: match r.source.as_str() {
                                    "confirm" => FactStatus::Confirmed,
                                    "koreksi pengguna" => FactStatus::Confirmed,
                                    "rebut" => FactStatus::Refuted,
                                    "retract" => FactStatus::Retracted,
                                    "tentative" | "guru tentative" => FactStatus::Tentative,
                                    _ => FactStatus::Claimed,
                                },
                                confirmations: if r.source == "confirm" { 1 } else { 0 },
                                supersedes: None,
                                id: r.fact_id,
                                parent_id: r.parent_id,
                                node_kind: r.node_kind.clone(),
                            },
                        );
                        for relation in extract_relations(raw, &r.source) {
                            if matches!(r.source.as_str(), "rebut" | "retract") {
                                graph.relations.retain(|stored| stored != &relation);
                            } else if r.source == "koreksi pengguna" {
                                graph.replace_conflicts(relation);
                            } else if !(ContradictionChecker {
                                graph: graph.clone(),
                            })
                            .check(&relation)
                            {
                                graph.insert(relation);
                            }
                        }
                    }
                }
            }
        }
        Self {
            memory,
            tokenizer: Tokenizer::new(vocab),
            network: LifNetwork::new(vocab, 1.0, 20.0, 2),
            stdp: StdpState::new(vocab),
            turns: vec![],
            facts,
            graph,
            max_turns: 32,
            simulation_steps: 20,
            llm: None,
            router: None,
            user_id: "local-user".into(),
            teacher_confidence_threshold: 0.65,
            conversation_log: None,
            last_student_draft: None,
            last_response_role: "murid".into(),
            bpe: BpeModel::train(&[], vocab),
            unigram: UnigramModel::train(&[], vocab, 1),
            decoder: NgramDecoder::train(&[], 3),
            checkpoint_dir: "checkpoints".into(),
        }
    }
    pub fn from_config(memory: EpisodicMemory, config: &VarkConfig) -> Self {
        let mut conversation = Self::new(memory, config.vocab_size);
        conversation.network = LifNetwork::new(
            config.neuron_count,
            config.threshold,
            config.tau,
            config.refractory_steps,
        );
        conversation.checkpoint_dir = config.checkpoint_dir.clone();
        let checkpoint =
            std::path::Path::new(&config.checkpoint_dir).join("conversation-network.json");
        conversation.network = LifNetwork::load(&checkpoint)
            .ok()
            .filter(|n| n.neurons.len() == config.neuron_count)
            .unwrap_or_else(|| {
                let mut network = LifNetwork::new(
                    config.neuron_count,
                    config.threshold,
                    config.tau,
                    config.refractory_steps,
                );
                network.connect_sparse_seeded(config.seed, 10);
                network
            });
        conversation.stdp = StdpState::new(config.neuron_count);
        conversation.simulation_steps = config.simulation_steps;
        conversation.llm = OpenAiCompatible::from_config(config);
        conversation.router = Some(TeacherStudentRouter::from_config(config));
        conversation.user_id = config.user_id.clone();
        conversation.teacher_confidence_threshold = config.teacher_confidence_threshold;
        conversation.conversation_log =
            ConversationLog::open(format!("{}.conversation.jsonl", config.memory_db)).ok();
        if let Ok(records) = conversation.memory.load_records() {
            let texts = records.iter().map(|r| r.text.clone()).collect::<Vec<_>>();
            if !texts.is_empty() {
                conversation.bpe = BpeModel::train(&texts, config.vocab_size);
                conversation.unigram = UnigramModel::train(&texts, config.vocab_size, 2);
                conversation.decoder = NgramDecoder::train(&texts, 3);
            }
        }
        conversation
    }
    pub fn with_max_turns(mut self, n: usize) -> Self {
        self.max_turns = n.max(1);
        self
    }
    pub fn ingest(
        &mut self,
        text: &str,
        source: impl Into<String>,
        confidence: f32,
    ) -> Result<usize, ConversationError> {
        let source = source.into();
        let mut count = 0;
        for raw in split_sentences(text) {
            let n = normalize(&raw);
            let tokens = content_tokens(&self.tokenizer, &n);
            if tokens.is_empty() {
                continue;
            }
            let parent_id = self.facts.get(&key_for(&tokens)).and_then(|old| old.id);
            let mut s = Sentence {
                text: raw.trim().into(),
                tokens: tokens.clone(),
                source: source.clone(),
                confidence: confidence.clamp(0.0, 1.0),
                status: if source == "koreksi pengguna" {
                    FactStatus::Confirmed
                } else if source == "guru tentative" {
                    FactStatus::Tentative
                } else {
                    FactStatus::Claimed
                },
                confirmations: if source == "koreksi pengguna" { 1 } else { 0 },
                supersedes: None,
                id: None,
                parent_id,
                node_kind: Some(if source == "koreksi pengguna" {
                    "koreksi".into()
                } else {
                    "klaim".into()
                }),
            };
            for relation in extract_relations(&raw, &source) {
                if source == "koreksi pengguna" {
                    self.graph.replace_conflicts(relation);
                } else {
                    let checker = ContradictionChecker {
                        graph: self.graph.clone(),
                    };
                    if !checker.check(&relation) {
                        self.graph.insert(relation);
                    }
                }
            }
            let event = self.memory.append_fact(
                raw.trim(),
                confidence,
                &source,
                parent_id,
                s.node_kind.as_deref().unwrap_or("klaim"),
            )?;
            s.id = event.fact_id;
            self.facts.insert(key_for(&tokens), s);
            count += 1
        }
        Ok(count)
    }
}

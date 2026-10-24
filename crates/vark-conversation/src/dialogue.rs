use super::Conversation;
use crate::{
    language::{content_tokens, is_question, normalize},
    types::{Answer, ConversationError, FactStatus, Sentence, Turn},
};
use std::collections::{HashMap, HashSet};
use vark_learning::StdpConfig;
use vark_llm::{LanguageModel, LlmRequest};
use vark_memory::semantic::{extract_relations, KnowledgeGraph};
use vark_memory::MemoryKind;
const UNKNOWN:&str="Saya belum memiliki bukti yang cukup untuk menjawabnya dengan andal. Mohon berikan sumber, konteks, atau koreksi yang dapat saya pelajari.";
impl Conversation {
    fn activate_input(&mut self, input: &str) -> usize {
        let pieces = self
            .bpe
            .encode(input)
            .into_iter()
            .chain(self.unigram.encode(input))
            .collect::<Vec<_>>();
        let mut external = vec![0.0; self.network.neurons.len()];
        for piece in pieces {
            for slot in self.network.assembly_for_concept(concept_hash(&piece), 4) {
                external[slot] += 0.35;
            }
        }
        let pre = external.iter().map(|v| *v > 0.0).collect::<Vec<_>>();
        let pre_sequence = (0..self.simulation_steps)
            .map(|_| pre.clone())
            .collect::<Vec<_>>();
        let trace = self
            .network
            .simulate_trace(&external, 1.0, self.simulation_steps);
        self.stdp.apply_sequence(
            &mut self.network,
            &pre_sequence,
            &trace,
            &StdpConfig::default(),
            1.0,
        );
        if let Some(last) = trace.last() {
            self.network.synapses.grow_coactive(last, 0.005);
        }
        trace
            .iter()
            .flat_map(|step| step.iter())
            .filter(|active| **active)
            .count()
    }
    pub async fn respond_async(&mut self, input: &str) -> Result<Answer, ConversationError> {
        let trimmed = input.trim();
        if trimmed.is_empty()
            || (self.llm.is_none() && self.router.is_none())
            || !is_question(trimmed)
        {
            return self.respond(input);
        }
        let spike_count = self.activate_input(trimmed);
        let safe_input = sanitize_external(trimmed);
        let plan = vark_reasoning::Planner::new(&self.memory)
            .plan(trimmed)
            .unwrap_or_default();
        let cases = match self.conversation_log.as_ref() {
            Some(log) => log.similar_cases(&self.user_id, trimmed, 2)?,
            None => Vec::new(),
        };
        let request = LlmRequest { system: "Anda adalah model murid sebagai penyusun draf. Pahami pertanyaan dan buat draf jawaban Indonesia. Keluarkan klaim atomik. Jangan menganggap draf sebagai fakta.".into(), user: format!("Pertanyaan: {}\nLangkah planner: {:?}\nKasus lama yang berhasil: {:?}", safe_input, plan, cases.iter().map(|c| sanitize_external(&c.text)).collect::<Vec<_>>()) };
        let draft = match if let Some(router) = &self.router {
            router.call_student(&request).await
        } else if let Some(model) = self.llm.clone() {
            model.complete_async(&request).await
        } else {
            return self.respond(input);
        } {
            Ok(reply) => reply,
            Err(_) => return self.respond(input),
        };
        let (claim_evidence, contradiction) =
            verified_claim_evidence(&self.graph, &self.facts, &draft.claims);
        let q = content_tokens(&self.tokenizer, trimmed);
        let selected = self.candidates(&q).into_iter().take(3).collect::<Vec<_>>();
        let selected_texts = selected
            .iter()
            .map(|sentence| sentence.text.as_str())
            .collect::<HashSet<_>>();
        let evidence = claim_evidence
            .iter()
            .filter(|sentence| selected_texts.contains(sentence.text.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let supported = !claim_evidence.is_empty() && evidence.len() == claim_evidence.len();
        let mut teacher_answer = None;
        let student_confidence = if contradiction {
            0.3
        } else if supported && !selected.is_empty() {
            0.8
        } else {
            0.4
        };
        if student_confidence < self.teacher_confidence_threshold {
            if let Some(router) = &self.router {
                let teacher_request = LlmRequest { system: "Anda adalah model guru. Tinjau draf murid, klaimnya, dan fakta VARK. Berikan jawaban Tentatif yang lebih benar. Jangan mengklaim kepastian jika bukti tidak ada.".into(), user: format!("Pertanyaan: {}\nDraf murid: {}\nKlaim: {:?}\nFakta VARK: {:?}", safe_input, sanitize_external(&draft.answer), draft.claims.iter().map(|c| sanitize_external(c)).collect::<Vec<_>>(), selected.iter().map(|s| sanitize_external(&s.text)).collect::<Vec<_>>()) };
                if let Ok(reply) = router.call_teacher(&teacher_request).await {
                    teacher_answer = Some(reply);
                }
            }
        }
        let teacher_disagreed = teacher_answer
            .as_ref()
            .map(|teacher| {
                let student_relations = draft
                    .claims
                    .iter()
                    .flat_map(|c| extract_relations(c, "student"))
                    .map(|r| (r.subject, r.predicate, r.object, r.polarity))
                    .collect::<std::collections::HashSet<_>>();
                let teacher_relations = teacher
                    .claims
                    .iter()
                    .flat_map(|c| extract_relations(c, "teacher"))
                    .map(|r| (r.subject, r.predicate, r.object, r.polarity))
                    .collect::<std::collections::HashSet<_>>();
                if !student_relations.is_empty() && !teacher_relations.is_empty() {
                    student_relations != teacher_relations
                } else {
                    normalize(&teacher.answer) != normalize(&draft.answer)
                }
            })
            .unwrap_or(false);
        if let Some(teacher) = &teacher_answer {
            for claim in &teacher.claims {
                self.ingest(claim, "guru tentative", 0.45)?;
            }
        }
        self.last_student_draft = Some(draft.answer.clone());
        self.last_response_role = if teacher_answer.is_some() {
            "guru".into()
        } else {
            "murid".into()
        };
        let (text, evidence, grounded, reasoning) = if contradiction {
            (
                UNKNOWN.to_owned(),
                vec![],
                false,
                vec!["Klaim draf berkonflik dengan graph; VARK abstain.".into()],
            )
        } else if supported {
            (
                evidence
                    .iter()
                    .map(|sentence| sentence.text.clone())
                    .collect::<Vec<_>>()
                    .join(" "),
                evidence,
                true,
                vec![if teacher_disagreed {
                    "Draf diverifikasi terhadap graph; fakta tersimpan menjadi jawaban.".into()
                } else {
                    "Setiap klaim draf cocok dengan graph; jawaban memakai fakta tersimpan.".into()
                }],
            )
        } else {
            (
                UNKNOWN.to_owned(),
                vec![],
                false,
                vec![if teacher_disagreed {
                    "Jawaban guru/murid belum terverifikasi terhadap graph; VARK abstain.".into()
                } else {
                    "Klaim kosong atau tidak didukung; VARK abstain.".into()
                }],
            )
        };
        let answer = answer(
            &text,
            if grounded { 0.85 } else { 0.35 },
            grounded,
            evidence.clone(),
            reasoning,
            spike_count,
        );
        self.push_turn(trimmed, &answer.text, evidence)?;
        Ok(answer)
    }
    pub fn respond(&mut self, input: &str) -> Result<Answer, ConversationError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(answer(
                "Silakan tulis pertanyaan atau pernyataan yang ingin dibahas.",
                0.0,
                false,
                vec![],
                vec!["Input kosong".into()],
                0,
            ));
        }
        let bpe_pieces = self.bpe.encode(trimmed);
        let unigram_pieces = self.unigram.encode(trimmed);
        let perplexity = self.decoder.perplexity(trimmed);
        let mut external = vec![0.0; self.network.neurons.len()];
        for piece in bpe_pieces.iter().chain(unigram_pieces.iter()) {
            for slot in self.network.assembly_for_concept(concept_hash(piece), 4) {
                external[slot] += 0.35;
            }
        }
        let pre = external.iter().map(|v| *v > 0.0).collect::<Vec<_>>();
        let pre_sequence = (0..self.simulation_steps)
            .map(|_| pre.clone())
            .collect::<Vec<_>>();
        let trace = self
            .network
            .simulate_trace(&external, 1.0, self.simulation_steps);
        let spike_count = trace
            .iter()
            .flat_map(|step| step.iter())
            .filter(|x| **x)
            .count();
        self.stdp.apply_sequence(
            &mut self.network,
            &pre_sequence,
            &trace,
            &StdpConfig::default(),
            1.0,
        );
        if let Some(last) = trace.last() {
            self.network.synapses.grow_coactive(last, 0.005);
        }
        if let Some(c) = trimmed
            .strip_prefix("koreksi:")
            .or_else(|| trimmed.strip_prefix("belajar:"))
        {
            let n = self.ingest(c.trim(), "koreksi pengguna", 1.0)?;
            let text = if n > 0 {
                "Baik, koreksi telah dipelajari dan disimpan."
            } else {
                UNKNOWN
            };
            return Ok(answer(
                text,
                if n > 0 { 1.0 } else { 0.0 },
                n > 0,
                vec![],
                vec!["Koreksi eksplisit diproses".into()],
                spike_count,
            ));
        }
        match normalize(trimmed).as_str() {
            "benar" | "betul" => {
                let n = self.update_last_fact(FactStatus::Confirmed)?;
                return Ok(answer(
                    if n > 0 {
                        "Baik, fakta terakhir dikonfirmasi dan jalur asosiasinya diperkuat."
                    } else {
                        UNKNOWN
                    },
                    if n > 0 { 1.0 } else { 0.0 },
                    n > 0,
                    vec![],
                    vec!["Konfirmasi pengguna direkam".into()],
                    spike_count,
                ));
            }
            "salah" => {
                let n = self.update_last_fact(FactStatus::Refuted)?;
                return Ok(answer(
                    if n > 0 {
                        "Baik, fakta terakhir ditandai dibantah."
                    } else {
                        UNKNOWN
                    },
                    if n > 0 { 1.0 } else { 0.0 },
                    n > 0,
                    vec![],
                    vec!["Bantahan pengguna direkam".into()],
                    spike_count,
                ));
            }
            "lupakan itu" => {
                let n = self.update_last_fact(FactStatus::Retracted)?;
                return Ok(answer(
                    if n > 0 {
                        "Baik, fakta terakhir ditarik dari pengetahuan aktif."
                    } else {
                        UNKNOWN
                    },
                    if n > 0 { 1.0 } else { 0.0 },
                    n > 0,
                    vec![],
                    vec!["Retraksi pengguna direkam".into()],
                    spike_count,
                ));
            }
            _ => {}
        }
        let normalized = normalize(trimmed);
        if let Some(correct) = normalized.strip_prefix("salah yang benar ") {
            self.update_last_fact(FactStatus::Refuted)?;
            let n = self.ingest(correct, "koreksi pengguna", 1.0)?;
            return Ok(answer(
                "Baik, jawaban sebelumnya dibantah dan fakta penggantinya disimpan.",
                if n > 0 { 1.0 } else { 0.0 },
                n > 0,
                vec![],
                vec!["Koreksi bercabang direkam".into()],
                spike_count,
            ));
        }
        let q = content_tokens(&self.tokenizer, trimmed);
        if !is_question(trimmed) {
            let n = self.ingest(trimmed, "pernyataan pengguna", 0.9)?;
            let text = if n > 0 {
                "Saya memahami pernyataan itu dan menyimpannya sebagai konteks percakapan."
            } else {
                UNKNOWN
            };
            self.push_turn(trimmed, text, vec![])?;
            return Ok(answer(
                text,
                if n > 0 { 0.9 } else { 0.0 },
                n > 0,
                vec![],
                vec![format!(
                    "Kalimat dianalisis sebagai pernyataan; perplexity n-gram {:.3}",
                    perplexity
                )],
                spike_count,
            ));
        }
        let selected = self.candidates(&q).into_iter().take(3).collect::<Vec<_>>();
        if selected.is_empty() {
            let a = answer(
                UNKNOWN,
                0.0,
                false,
                vec![],
                vec!["Bukti tidak cukup; abstain untuk mencegah halusinasi".into()],
                spike_count,
            );
            self.push_turn(trimmed, &a.text, vec![])?;
            return Ok(a);
        }
        let confidence =
            (selected.iter().map(|s| s.confidence).sum::<f32>() / selected.len() as f32).min(0.95);
        if let Some(model) = self.llm.clone() {
            let evidence = selected
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let request = LlmRequest { system: "Anda adalah VARK. Gunakan hanya bukti berikut. Jangan menambah fakta. Jika bukti tidak cukup, jawab Saya tidak tahu.".into(), user: format!("Pertanyaan: {}\nBukti:\n{}", trimmed, evidence) };
            if let Ok(reply) = model.complete(&request) {
                let evidence_words = evidence
                    .split_whitespace()
                    .map(normalize)
                    .collect::<Vec<_>>();
                let claims_grounded = reply.claims.iter().all(|claim| {
                    let words = normalize(claim)
                        .split_whitespace()
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                    words
                        .iter()
                        .filter(|w| evidence_words.iter().any(|e| e == *w))
                        .count()
                        >= words.len().min(2)
                });
                if claims_grounded
                    && !reply.answer.trim().is_empty()
                    && reply.answer.trim() != "Saya tidak tahu"
                {
                    let a = answer(
                        &reply.answer,
                        confidence,
                        true,
                        selected.clone(),
                        vec!["Jawaban LLM dibatasi oleh bukti dan klaim divalidasi".into()],
                        spike_count,
                    );
                    self.push_turn(trimmed, &a.text, selected)?;
                    return Ok(a);
                }
            }
        }
        let text = selected
            .iter()
            .map(|s| s.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let a = answer(
            &text,
            confidence,
            true,
            selected.clone(),
            vec![
                "Tokenisasi dan simulasi LIF 20 timestep".into(),
                format!(
                    "{} bukti dipilih; perplexity n-gram {:.3}",
                    selected.len(),
                    perplexity
                ),
                "STDP diterapkan setelah respons".into(),
            ],
            spike_count,
        );
        self.push_turn(trimmed, &a.text, selected)?;
        Ok(a)
    }
    fn candidates(&self, q: &[String]) -> Vec<Sentence> {
        let set: HashSet<&String> = q.iter().collect();
        let mut v = self
            .facts
            .values()
            .cloned()
            .map(|s| {
                let o = s.tokens.iter().filter(|t| set.contains(t)).count() as f32;
                let boost = match s.status {
                    FactStatus::Confirmed => 1.25,
                    FactStatus::Tentative => 0.4,
                    FactStatus::Claimed => 0.75,
                    FactStatus::Refuted | FactStatus::Retracted => 0.0,
                };
                (o / (q.len().max(1) as f32) * s.confidence * boost, s)
            })
            .filter(|(x, _)| *x > 0.0)
            .collect::<Vec<_>>();
        v.sort_by(|a, b| b.0.total_cmp(&a.0));
        v.into_iter().map(|(_, s)| s).collect()
    }
    fn push_turn(
        &mut self,
        user: &str,
        assistant: &str,
        evidence: Vec<Sentence>,
    ) -> Result<(), ConversationError> {
        let trace = serde_json::json!({"question": user, "facts_used": evidence.iter().map(|s| &s.text).collect::<Vec<_>>(), "conclusion": assistant, "feedback": null});
        self.turns.push(Turn {
            user: user.into(),
            assistant: assistant.into(),
            evidence,
        });
        if self.turns.len() > self.max_turns {
            self.turns.remove(0);
        }
        self.memory.append_kind(
            &format!("Pengguna: {} Jawaban: {}", user, assistant),
            0.8,
            MemoryKind::Dialogue,
            "dialog",
        )?;
        if let Some(log) = &mut self.conversation_log {
            log.append(&self.user_id, "user", user, None)
                .map_err(ConversationError::Memory)?;
            if let Some(draft) = self.last_student_draft.take() {
                log.append(&self.user_id, "murid", &draft, None)
                    .map_err(ConversationError::Memory)?;
            }
            let feedback = (self.last_response_role == "guru").then_some("Tentatif");
            log.append(&self.user_id, &self.last_response_role, assistant, feedback)
                .map_err(ConversationError::Memory)?;
            log.append(&self.user_id, "reasoning", &trace.to_string(), None)
                .map_err(ConversationError::Memory)?;
        }
        let checkpoint =
            std::path::Path::new(&self.checkpoint_dir).join("conversation-network.json");
        self.network
            .save_atomic(checkpoint)
            .map_err(|e| ConversationError::Checkpoint(e.to_string()))?;
        Ok(())
    }
}
fn verify_claims(graph: &KnowledgeGraph, claims: &[String]) -> (bool, bool) {
    if claims.is_empty() {
        return (false, false);
    }
    let mut contradiction = false;
    for claim in claims {
        let relations = extract_relations(claim, "draft");
        if relations.is_empty() {
            return (false, contradiction);
        }
        for relation in relations {
            if graph
                .query(
                    Some(&relation.subject),
                    Some(&relation.predicate),
                    Some(&relation.object),
                    Some(relation.polarity),
                )
                .is_empty()
            {
                contradiction |= !graph.conflicts_for(&relation).is_empty();
                return (false, contradiction);
            }
        }
    }
    (true, contradiction)
}

fn verified_claim_evidence(
    graph: &KnowledgeGraph,
    facts: &HashMap<String, Sentence>,
    claims: &[String],
) -> (Vec<Sentence>, bool) {
    let (claims_supported, contradiction) = verify_claims(graph, claims);
    if !claims_supported {
        return (Vec::new(), contradiction);
    }
    let mut evidence = Vec::new();
    for claim in claims {
        for relation in extract_relations(claim, "draft") {
            let source = facts.values().find(|sentence| {
                !matches!(sentence.status, FactStatus::Refuted | FactStatus::Retracted)
                    && extract_relations(&sentence.text, &sentence.source)
                        .iter()
                        .any(|stored| {
                            stored.subject == relation.subject
                                && stored.predicate == relation.predicate
                                && stored.object == relation.object
                                && stored.polarity == relation.polarity
                        })
            });
            let Some(source) = source else {
                return (Vec::new(), false);
            };
            if !evidence
                .iter()
                .any(|saved: &Sentence| saved.text == source.text)
            {
                evidence.push(source.clone());
            }
        }
    }
    (evidence, false)
}

fn answer(t: &str, c: f32, g: bool, e: Vec<Sentence>, r: Vec<String>, s: usize) -> Answer {
    Answer {
        text: t.into(),
        confidence: c,
        grounded: g,
        evidence: e,
        reasoning: r,
        spike_count: s,
    }
}
fn concept_hash(text: &str) -> u64 {
    text.bytes().fold(1469598103934665603u64, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(1099511628211)
    })
}
fn sanitize_external(input: &str) -> String {
    vark_memory::redact_sensitive_text(input)
}

#[cfg(test)]
mod claim_verification_tests {
    use super::*;

    #[test]
    fn empty_unparseable_and_unsupported_claims_do_not_pass() {
        let mut graph = KnowledgeGraph::new();
        for relation in extract_relations("Rust adalah bahasa pemrograman", "fakta") {
            graph.insert(relation);
        }

        assert_eq!(verify_claims(&graph, &[]), (false, false));
        assert_eq!(
            verify_claims(&graph, &["jawaban tanpa relasi".into()]),
            (false, false)
        );
        assert_eq!(
            verify_claims(&graph, &["Python adalah bahasa pemrograman".into()]),
            (false, false)
        );
    }

    #[test]
    fn every_claim_must_match_an_existing_relation() {
        let mut graph = KnowledgeGraph::new();
        for relation in extract_relations("Rust adalah bahasa pemrograman", "fakta") {
            graph.insert(relation);
        }
        assert_eq!(
            verify_claims(&graph, &["Rust adalah bahasa pemrograman".into()]),
            (true, false)
        );
    }

    #[test]
    fn grounded_evidence_must_come_from_a_matching_active_fact() {
        let mut graph = KnowledgeGraph::new();
        let text = "Rust adalah bahasa pemrograman";
        for relation in extract_relations(text, "koreksi pengguna") {
            graph.insert(relation);
        }
        let sentence = Sentence {
            text: text.into(),
            tokens: vec!["rust".into(), "bahasa".into(), "pemrograman".into()],
            source: "koreksi pengguna".into(),
            confidence: 1.0,
            status: FactStatus::Confirmed,
            confirmations: 1,
            supersedes: None,
            id: Some(1),
            parent_id: None,
            node_kind: Some("koreksi".into()),
        };
        let facts = HashMap::from([("rust".into(), sentence)]);
        let (evidence, contradiction) =
            verified_claim_evidence(&graph, &facts, &["Rust adalah bahasa pemrograman".into()]);
        assert!(!contradiction);
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].text, text);
        assert!(verified_claim_evidence(
            &graph,
            &HashMap::new(),
            &["Rust adalah bahasa pemrograman".into()]
        )
        .0
        .is_empty());
    }
}

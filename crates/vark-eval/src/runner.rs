use crate::{
    dataset::{DatasetBundle, EvalCase},
    metrics::{percentile, ratio, EvalReport},
};
use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use vark_api::Engine;
use vark_config::VarkConfig;
use vark_decoder::NgramDecoder;
use vark_memory::semantic::extract_relations;

struct Outcome {
    answer: vark_conversation::Answer,
    latency_ms: f64,
}

pub fn evaluate() -> Result<EvalReport> {
    let bundle = DatasetBundle::load()?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("vark-eval-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&root)?;

    let mut outcomes: BTreeMap<String, Vec<(EvalCase, Outcome)>> = BTreeMap::new();
    let result = (|| {
        for (category, cases) in &bundle.datasets {
            let mut evaluated = Vec::with_capacity(cases.len());
            for case in cases {
                let outcome = run_case(&root, category, case)?;
                evaluated.push((case.clone(), outcome));
            }
            outcomes.insert(category.clone(), evaluated);
        }
        build_report(&bundle, &outcomes)
    })();
    let cleanup_result = fs::remove_dir_all(&root).context("menghapus data sementara evaluator");
    match (result, cleanup_result) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn run_case(root: &Path, category: &str, case: &EvalCase) -> Result<Outcome> {
    let case_root = root.join(category).join(&case.id);
    let config = VarkConfig {
        neuron_count: 256,
        input_size: 64,
        vocab_size: 512,
        simulation_steps: 8,
        memory_db: case_root
            .join("memory.jsonl")
            .to_string_lossy()
            .into_owned(),
        checkpoint_dir: case_root.join("checkpoints").to_string_lossy().into_owned(),
        ..VarkConfig::default()
    };
    let mut engine = Engine::new(config)?;
    for fact in &case.facts {
        let text = if case.should_abstain {
            fact.clone()
        } else {
            format!("koreksi: {fact}")
        };
        engine
            .chat(&text)
            .with_context(|| format!("ingest fakta {}", case.id))?;
    }
    let query = match &case.instruction {
        Some(instruction) => format!("{instruction}\n{}", case.question),
        None => case.question.clone(),
    };
    let started = Instant::now();
    let answer = engine
        .chat(&query)
        .with_context(|| format!("inferensi kasus {}", case.id))?;
    Ok(Outcome {
        answer,
        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
    })
}

fn build_report(
    bundle: &DatasetBundle,
    outcomes: &BTreeMap<String, Vec<(EvalCase, Outcome)>>,
) -> Result<EvalReport> {
    let qa = &outcomes["qa_factual"];
    let (qa_correct, qa_answerable) = accuracy(qa);
    let mut true_abstain = 0;
    let mut false_abstain = 0;
    let mut missed_abstain = 0;
    for (case, outcome) in qa {
        let predicted_abstain = abstained(&outcome.answer);
        match (case.answerable, predicted_abstain) {
            (false, true) => true_abstain += 1,
            (true, true) => false_abstain += 1,
            (false, false) => missed_abstain += 1,
            (true, false) => {}
        }
    }
    let mut reasoning_accuracy = BTreeMap::new();
    for category in [
        "reasoning_transitive",
        "reasoning_compare",
        "reasoning_rules",
        "long_sentence",
    ] {
        let (correct, total) = accuracy(&outcomes[category]);
        reasoning_accuracy.insert(category.to_owned(), ratio(correct, total));
    }
    let contradiction = &outcomes["contradiction"];
    let contradiction_abstained = contradiction
        .iter()
        .filter(|(_, outcome)| abstained(&outcome.answer))
        .count();
    let generation = &outcomes["generation"];
    let (matched_facts, all_facts) =
        generation
            .iter()
            .fold((0, 0), |(matched, total), (case, outcome)| {
                let answer = normalize(&outcome.answer.text);
                let count = case.required_facts.len();
                let found = case
                    .required_facts
                    .iter()
                    .filter(|fact| answer.contains(&normalize(fact)))
                    .count();
                (matched + found, total + count)
            });
    let generation_unsupported = generation
        .iter()
        .filter(|(case, outcome)| {
            !abstained(&outcome.answer) && !supported_by_sources(case, &outcome.answer.text)
        })
        .count();
    let scored_outputs = outcomes
        .iter()
        .filter(|(category, _)| category.as_str() != "contradiction")
        .flat_map(|(_, rows)| rows.iter())
        .filter(|(_, outcome)| !abstained(&outcome.answer))
        .collect::<Vec<_>>();
    let unsupported_outputs = scored_outputs
        .iter()
        .filter(|(case, outcome)| !supported_by_sources(case, &outcome.answer.text))
        .count();
    let mut latencies = outcomes
        .values()
        .flat_map(|rows| rows.iter().map(|(_, outcome)| outcome.latency_ms))
        .collect::<Vec<_>>();
    let p50 = percentile(&mut latencies, 0.50);
    let p95 = percentile(&mut latencies, 0.95);
    let counts = outcomes
        .iter()
        .map(|(name, rows)| (name.clone(), rows.len()))
        .collect::<BTreeMap<_, _>>();

    let training_texts = bundle.datasets["qa_factual"]
        .iter()
        .filter(|case| case.answerable)
        .flat_map(|case| case.facts.iter().cloned())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let decoder = NgramDecoder::train(&training_texts, 3);
    let heldout = bundle.datasets["long_sentence"]
        .iter()
        .flat_map(|case| case.facts.iter());
    let (perplexity_sum, perplexity_count) = heldout.fold((0.0, 0usize), |(sum, count), text| {
        (sum + decoder.perplexity(text), count + 1)
    });
    let perplexity = if perplexity_count == 0 {
        0.0
    } else {
        perplexity_sum / perplexity_count as f64
    };

    Ok(EvalReport {
        schema_version: 1,
        dataset_version: bundle.version.clone(),
        dataset_hashes: bundle.hashes.clone(),
        case_counts: counts,
        cases: outcomes.values().map(Vec::len).sum(),
        answer_accuracy: ratio(qa_correct, qa_answerable),
        abstain_precision: ratio(true_abstain, true_abstain + false_abstain),
        abstain_recall: ratio(true_abstain, true_abstain + missed_abstain),
        hallucination_rate: ratio(unsupported_outputs, scored_outputs.len()),
        reasoning_accuracy,
        contradiction_detection_rate: ratio(contradiction_abstained, contradiction.len()),
        generation_fact_recall: ratio(matched_facts, all_facts),
        generation_unsupported_rate: ratio(generation_unsupported, generation.len()),
        perplexity,
        latency_p50_ms: p50,
        latency_p95_ms: p95,
    })
}

fn accuracy(cases: &[(EvalCase, Outcome)]) -> (usize, usize) {
    let scored = cases
        .iter()
        .filter(|(case, _)| case.answerable)
        .collect::<Vec<_>>();
    let correct = scored
        .iter()
        .filter(|(case, outcome)| {
            !abstained(&outcome.answer)
                && case
                    .expected_answer_contains
                    .iter()
                    .all(|expected| normalize(&outcome.answer.text).contains(&normalize(expected)))
        })
        .count();
    (correct, scored.len())
}

fn abstained(answer: &vark_conversation::Answer) -> bool {
    !answer.grounded
        || answer.text.to_lowercase().contains("belum memiliki bukti")
        || answer.text.to_lowercase().contains("saya tidak tahu")
}

fn supported_by_sources(case: &EvalCase, answer: &str) -> bool {
    let claims = extract_relations(answer, "evaluation");
    if claims.is_empty() {
        return false;
    }
    let sources = case
        .facts
        .iter()
        .flat_map(|fact| extract_relations(fact, "source"))
        .collect::<Vec<_>>();
    claims.iter().all(|claim| {
        sources.iter().any(|source| {
            claim.subject == source.subject
                && claim.predicate == source.predicate
                && claim.object == source.object
                && claim.polarity == source.polarity
        })
    })
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character.is_whitespace() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

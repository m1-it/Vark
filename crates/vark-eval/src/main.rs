use anyhow::Result;
use serde::Serialize;
use std::{fs, path::PathBuf};
use vark_api::Engine;
use vark_config::VarkConfig;

#[derive(Debug, Serialize)]
struct EvalReport {
    cases: usize,
    corrected_accuracy: f32,
    consistency: f32,
    teacher_calls: u64,
    total_llm_calls: u64,
    teacher_call_rate: f32,
    estimated_cost: f64,
}

fn main() -> Result<()> {
    let root = std::env::temp_dir().join(format!("vark-eval-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)?;
    let mut config = VarkConfig::default();
    config.memory_db = root.join("memory.jsonl").to_string_lossy().into();
    config.checkpoint_dir = root.join("checkpoints").to_string_lossy().into();
    let mut engine = Engine::new(config.clone())?;
    let cases = [(
        "Rust adalah bahasa pemrograman sistem.",
        "Apa Rust?",
        "Rust adalah bahasa pemrograman sistem.",
    )];
    let mut correct = 0;
    let mut consistent = 0;
    for (fact, question, expected) in cases {
        let _ = engine.chat(fact)?;
        let first = engine.chat(question)?.text;
        let second = engine.chat(question)?.text;
        let clean = |s: &str| s.to_ascii_lowercase().replace(['.', ',', '!', '?'], "");
        if clean(&first).contains(&clean(expected)) {
            correct += 1;
        }
        if first == second {
            consistent += 1;
        }
    }
    let stats = engine.routing_stats();
    let calls = stats
        .as_ref()
        .map(|s| s.student_calls + s.teacher_calls)
        .unwrap_or(0);
    let teachers = stats.as_ref().map(|s| s.teacher_calls).unwrap_or(0);
    let report = EvalReport {
        cases: cases.len(),
        corrected_accuracy: correct as f32 / cases.len() as f32,
        consistency: consistent as f32 / cases.len() as f32,
        teacher_calls: teachers,
        total_llm_calls: calls,
        teacher_call_rate: if calls == 0 {
            0.0
        } else {
            teachers as f32 / calls as f32
        },
        estimated_cost: stats.map(|s| s.estimated_cost).unwrap_or(0.0),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    let output = PathBuf::from("data/evaluation-latest.json");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    let _ = fs::remove_dir_all(root);
    Ok(())
}

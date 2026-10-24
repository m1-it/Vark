use anyhow::Result;
use std::{
    env,
    io::{self, BufRead, Write},
};
use vark_api::Engine;
use vark_config::VarkConfig;
#[tokio::main]
async fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("train") {
        let first = args.get(1).map(String::as_str);
        let (config_path, subject) = match first {
            Some(path) if std::path::Path::new(path).exists() => {
                (Some(path), args.get(2).map(String::as_str))
            }
            Some(subject) => (None, Some(subject)),
            None => (None, None),
        };
        return train(config_path, subject).await;
    }
    if args.first().map(String::as_str) == Some("--chat") {
        return chat(args.get(1).map(String::as_str)).await;
    }
    let mut cfg = VarkConfig::default();
    let input = match args.as_slice() {
        [] => String::new(),
        [text] => text.clone(),
        [path, text @ ..] => {
            cfg = VarkConfig::load(path)?;
            text.join(" ")
        }
    };
    if input.is_empty() {
        println!("Gunakan: cargo run -p vark-cli -- \"teks\" atau --chat [config.json]");
        return Ok(());
    }
    let mut e = Engine::new(cfg)?;
    println!("{}", serde_json::to_string_pretty(&e.infer(&input)?)?);
    Ok(())
}
async fn train(config_path: Option<&str>, subject: Option<&str>) -> Result<()> {
    let cfg = match config_path {
        Some(path) => VarkConfig::load(path)?,
        None => VarkConfig::default(),
    };
    let subject = subject.unwrap_or("konsep utama");
    let curriculum = [
        format!("Apa itu {}?", subject),
        format!("Apa fungsi {}?", subject),
        format!("Mengapa {} penting?", subject),
        format!("Apa beda {} dengan konsep terkait?", subject),
        format!("Berikan contoh penggunaan {}.", subject),
    ];
    let max_questions = std::env::var("VARK_TRAIN_MAX_QUESTIONS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(curriculum.len())
        .min(curriculum.len());
    let mut engine = Engine::new(cfg)?;
    println!("Sesi training dimulai untuk: {}", subject);
    for question in curriculum.iter().take(max_questions) {
        let answer = engine.chat_async(question).await?;
        println!(
            "TRAIN {}\nMURID: {}\nconfidence={:.2} grounded={}",
            question, answer.text, answer.confidence, answer.grounded
        );
    }
    if let Some(stats) = engine.routing_stats() {
        println!(
            "STATISTIK: student_calls={} teacher_calls={} tokens={} cost={:.6}",
            stats.student_calls,
            stats.teacher_calls,
            stats.student_tokens + stats.teacher_tokens,
            stats.estimated_cost
        );
    }
    Ok(())
}
async fn chat(path: Option<&str>) -> Result<()> {
    let cfg = match path {
        Some(p) => VarkConfig::load(p)?,
        None => VarkConfig::default(),
    };
    let mut e = Engine::new(cfg)?;
    println!("VARK chat aktif. Ketik 'keluar' untuk berhenti.");
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.trim().eq_ignore_ascii_case("keluar") {
            break;
        }
        let a = e.chat_async(&line).await?;
        println!(
            "VARK: {}\n[keyakinan: {:.2}, grounded: {}, spikes: {}]",
            a.text, a.confidence, a.grounded, a.spike_count
        );
        io::stdout().flush()?;
    }
    Ok(())
}

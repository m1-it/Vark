mod dataset;
mod metrics;
mod runner;

use anyhow::{bail, Context, Result};
use metrics::{enforce_regression, EvalReport};
use std::{env, fs, path::PathBuf};

fn main() -> Result<()> {
    let mut json_only = false;
    let mut check_baseline = false;
    let mut save_report = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--json" => json_only = true,
            "--check-baseline" => check_baseline = true,
            "--save-report" => {
                save_report = Some(PathBuf::from(
                    args.next().context("--save-report memerlukan path")?,
                ));
            }
            unknown => bail!("argumen evaluator tidak dikenal: {unknown}"),
        }
    }

    let report = runner::evaluate()?;
    if check_baseline {
        let baseline_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("baselines/baseline_v0.json");
        let baseline: EvalReport = serde_json::from_slice(
            &fs::read(&baseline_path)
                .with_context(|| format!("membaca {}", baseline_path.display()))?,
        )?;
        enforce_regression(&baseline, &report)?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = save_report {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("{json}\n"))?;
    }
    println!("{json}");
    if !json_only {
        println!("\n{}", report.table());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_engine_evaluation_does_not_regress_from_baseline() {
        let baseline_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("baselines/baseline_v0.json");
        let baseline: EvalReport =
            serde_json::from_slice(&fs::read(baseline_path).unwrap()).unwrap();
        let current = runner::evaluate().unwrap();
        enforce_regression(&baseline, &current).unwrap();
    }
}

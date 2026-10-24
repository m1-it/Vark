use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub schema_version: u32,
    pub dataset_version: String,
    pub dataset_hashes: BTreeMap<String, String>,
    pub case_counts: BTreeMap<String, usize>,
    pub cases: usize,
    pub answer_accuracy: f64,
    pub abstain_precision: f64,
    pub abstain_recall: f64,
    pub hallucination_rate: f64,
    pub reasoning_accuracy: BTreeMap<String, f64>,
    pub contradiction_detection_rate: f64,
    pub generation_fact_recall: f64,
    pub generation_unsupported_rate: f64,
    pub perplexity: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
}

impl EvalReport {
    pub fn table(&self) -> String {
        let mut rows = vec![
            ("answer_accuracy", self.answer_accuracy),
            ("abstain_precision", self.abstain_precision),
            ("abstain_recall", self.abstain_recall),
            ("hallucination_rate", self.hallucination_rate),
        ];
        rows.extend(
            self.reasoning_accuracy
                .iter()
                .map(|(name, value)| (name.as_str(), *value)),
        );
        rows.extend([
            (
                "contradiction_detection_rate",
                self.contradiction_detection_rate,
            ),
            ("generation_fact_recall", self.generation_fact_recall),
            (
                "generation_unsupported_rate",
                self.generation_unsupported_rate,
            ),
            ("perplexity", self.perplexity),
            ("latency_p50_ms", self.latency_p50_ms),
            ("latency_p95_ms", self.latency_p95_ms),
        ]);
        let mut output = format!("Metric                         Value\n{}\n", "-".repeat(48));
        for (name, value) in rows {
            output.push_str(&format!("{name:<30} {value:.6}\n"));
        }
        output.push_str(&format!("{:<30} {}\n", "cases", self.cases));
        output
    }
}

pub fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

pub fn percentile(values: &mut [f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let rank = ((percentile.clamp(0.0, 1.0) * values.len() as f64).ceil() as usize).max(1);
    values[rank.saturating_sub(1).min(values.len() - 1)]
}

pub fn enforce_regression(previous: &EvalReport, current: &EvalReport) -> Result<()> {
    if previous.schema_version != current.schema_version
        || previous.dataset_version != current.dataset_version
        || previous.dataset_hashes != current.dataset_hashes
        || previous.case_counts != current.case_counts
    {
        bail!("baseline dan hasil baru memakai schema atau dataset yang berbeda");
    }
    let maximum_drop = 0.02;
    let mut regressions = Vec::new();
    check_metric(
        "answer_accuracy",
        previous.answer_accuracy,
        current.answer_accuracy,
        maximum_drop,
        &mut regressions,
    );
    check_metric(
        "abstain_precision",
        previous.abstain_precision,
        current.abstain_precision,
        maximum_drop,
        &mut regressions,
    );
    check_metric(
        "abstain_recall",
        previous.abstain_recall,
        current.abstain_recall,
        maximum_drop,
        &mut regressions,
    );
    check_metric(
        "contradiction_detection_rate",
        previous.contradiction_detection_rate,
        current.contradiction_detection_rate,
        maximum_drop,
        &mut regressions,
    );
    check_metric(
        "generation_fact_recall",
        previous.generation_fact_recall,
        current.generation_fact_recall,
        maximum_drop,
        &mut regressions,
    );
    for (category, old_value) in &previous.reasoning_accuracy {
        if let Some(new_value) = current.reasoning_accuracy.get(category) {
            check_metric(
                category,
                *old_value,
                *new_value,
                maximum_drop,
                &mut regressions,
            );
        }
    }
    if current.hallucination_rate > previous.hallucination_rate {
        regressions.push(format!(
            "hallucination_rate naik dari {:.6} menjadi {:.6}",
            previous.hallucination_rate, current.hallucination_rate
        ));
    }
    if current.generation_unsupported_rate > previous.generation_unsupported_rate {
        regressions.push(format!(
            "generation_unsupported_rate naik dari {:.6} menjadi {:.6}",
            previous.generation_unsupported_rate, current.generation_unsupported_rate
        ));
    }
    if regressions.is_empty() {
        Ok(())
    } else {
        bail!("gerbang regresi evaluasi gagal: {}", regressions.join("; "))
    }
}

fn check_metric(
    name: &str,
    previous: f64,
    current: f64,
    allowed_drop: f64,
    regressions: &mut Vec<String>,
) {
    if current + allowed_drop < previous {
        regressions.push(format!(
            "{name} turun lebih dari {:.2} poin: {:.6} -> {:.6}",
            allowed_drop * 100.0,
            previous,
            current
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_uses_nearest_rank_for_p50_and_p95() {
        let mut values = [5.0, 1.0, 4.0, 2.0, 3.0];
        assert_eq!(percentile(&mut values, 0.50), 3.0);
        assert_eq!(percentile(&mut values, 0.95), 5.0);
    }

    #[test]
    fn ratio_handles_empty_denominator_without_nan() {
        assert_eq!(ratio(1, 0), 0.0);
        assert_eq!(ratio(1, 2), 0.5);
    }
}

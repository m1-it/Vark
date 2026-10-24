use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvalCase {
    pub id: String,
    pub facts: Vec<String>,
    pub question: String,
    #[serde(default)]
    pub expected_answer_contains: Vec<String>,
    #[serde(default = "default_true")]
    pub answerable: bool,
    #[serde(default)]
    pub should_abstain: bool,
    #[serde(default)]
    pub instruction: Option<String>,
    #[serde(default)]
    pub required_facts: Vec<String>,
    #[serde(default)]
    pub word_count: Option<usize>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct Manifest {
    dataset_version: String,
    record_counts: BTreeMap<String, usize>,
    sha256: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct DatasetBundle {
    pub version: String,
    pub hashes: BTreeMap<String, String>,
    pub datasets: BTreeMap<String, Vec<EvalCase>>,
}

const DATASETS: &[(&str, usize)] = &[
    ("qa_factual", 100),
    ("reasoning_transitive", 40),
    ("reasoning_compare", 30),
    ("reasoning_rules", 30),
    ("contradiction", 20),
    ("long_sentence", 30),
    ("generation", 30),
];

impl DatasetBundle {
    pub fn load() -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let manifest: Manifest = serde_json::from_slice(
            &fs::read(root.join("manifest.json")).context("membaca manifest dataset")?,
        )
        .context("mem-parsing manifest dataset")?;
        let mut datasets = BTreeMap::new();
        let mut hashes = BTreeMap::new();
        for (name, minimum) in DATASETS {
            let filename = format!("{name}.jsonl");
            let bytes =
                fs::read(root.join(&filename)).with_context(|| format!("membaca {filename}"))?;
            let expected_hash = manifest
                .sha256
                .get(&filename)
                .with_context(|| format!("checksum {filename} tidak tercantum"))?;
            if vark_hashing::sha256_hex(&bytes) != *expected_hash {
                bail!("checksum dataset {filename} tidak cocok dengan manifest");
            }
            hashes.insert((*name).to_owned(), expected_hash.clone());
            let mut cases = Vec::new();
            for (index, line) in std::str::from_utf8(&bytes)?.lines().enumerate() {
                let case: EvalCase = serde_json::from_str(line)
                    .with_context(|| format!("{filename}:{}", index + 1))?;
                validate_case(&case).with_context(|| format!("{filename}:{}", index + 1))?;
                cases.push(case);
            }
            let declared_count = manifest
                .record_counts
                .get(*name)
                .with_context(|| format!("jumlah {name} tidak tercantum"))?;
            if cases.len() < *minimum || cases.len() != *declared_count {
                bail!("jumlah record {name} tidak memenuhi manifest/minimum");
            }
            datasets.insert((*name).to_owned(), cases);
        }
        let qa = &datasets["qa_factual"];
        let unanswerable = qa.iter().filter(|case| !case.answerable).count();
        if unanswerable * 100 < qa.len() * 30 {
            bail!("qa_factual harus memiliki sekurangnya 30% unanswerable");
        }
        if datasets["long_sentence"].iter().any(|case| {
            !case
                .facts
                .iter()
                .any(|fact| (40..=150).contains(&fact.split_whitespace().count()))
        }) {
            bail!("setiap long_sentence harus berisi fakta 40–150 kata");
        }
        Ok(Self {
            version: manifest.dataset_version,
            hashes,
            datasets,
        })
    }
}

fn validate_case(case: &EvalCase) -> Result<()> {
    if case.id.trim().is_empty() || case.question.trim().is_empty() || case.facts.is_empty() {
        bail!("id, question, dan facts wajib ada");
    }
    if case.facts.iter().any(|fact| fact.trim().is_empty()) {
        bail!("facts tidak boleh kosong");
    }
    if case.answerable && case.expected_answer_contains.is_empty() && case.required_facts.is_empty()
    {
        bail!("kasus answerable wajib memiliki target");
    }
    if case.should_abstain && case.answerable {
        bail!("kasus yang meminta abstain tidak boleh answerable");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn versioned_corpus_meets_minimums_and_has_unique_ids() {
        let bundle = DatasetBundle::load().unwrap();
        assert_eq!(bundle.version, "vark-eval-2026-10-24-v1");
        assert_eq!(bundle.datasets.len(), DATASETS.len());
        for (name, minimum) in DATASETS {
            let cases = &bundle.datasets[*name];
            assert!(cases.len() >= *minimum);
            let ids = cases.iter().map(|case| &case.id).collect::<HashSet<_>>();
            assert_eq!(ids.len(), cases.len());
        }
    }

    #[test]
    fn factual_set_has_required_unanswerable_share_and_long_text_meets_bounds() {
        let bundle = DatasetBundle::load().unwrap();
        let qa = &bundle.datasets["qa_factual"];
        assert!(qa.iter().filter(|case| !case.answerable).count() * 100 >= qa.len() * 30);
        assert!(bundle.datasets["long_sentence"].iter().all(|case| {
            case.facts
                .iter()
                .any(|fact| (40..=150).contains(&fact.split_whitespace().count()))
        }));
    }

    #[test]
    fn malformed_or_unscored_cases_are_rejected() {
        assert!(serde_json::from_str::<EvalCase>("not-json").is_err());
        let case = EvalCase {
            id: "case".into(),
            facts: vec!["Ada fakta.".into()],
            question: "Apa?".into(),
            expected_answer_contains: Vec::new(),
            answerable: true,
            should_abstain: false,
            instruction: None,
            required_facts: Vec::new(),
            word_count: None,
        };
        assert!(validate_case(&case).is_err());
    }
}

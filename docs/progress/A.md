# Tahap A — Baseline dan fondasi evaluasi

**Status:** selesai untuk fondasi pengukuran. Tahap ini menambah evaluasi dan CI; tidak mengubah algoritme produk, sehingga tidak mengklaim kemampuan VARK meningkat.

## 1. Ringkasan perubahan

- `crates/vark-eval/data/`: snapshot lokal ber-hash tujuh kategori, total 283 kasus: 103 QA faktual (31 unanswerable), 40 transitive, 30 perbandingan, 30 aturan, 20 kontradiksi, 30 long-sentence, dan 30 generation/komposisi.
- `crates/vark-eval/src/dataset.rs`: schema JSONL, validasi jumlah minimum, rasio unanswerable, rentang panjang teks, dan checksum SHA-256.
- `crates/vark-eval/src/runner.rs`: setiap kasus menjalankan `vark_api::Engine` dengan penyimpanan terisolasi; mengukur jawaban, grounding terhadap relasi sumber, dan latency. Decoder n-gram VARK dilatih hanya pada fakta QA lalu diuji pada teks long-sentence terpisah.
- `crates/vark-eval/src/metrics.rs`: report terstruktur, precision/recall abstain, akurasi per kategori, hallucination/unsupported rate, recall fakta, perplexity, latency p50/p95, dan pemeriksaan regresi.
- `crates/vark-eval/src/main.rs`: output JSON atau JSON plus tabel, opsi `--check-baseline` dan `--save-report`.
- `crates/vark-eval/baselines/baseline_v0.json` dan `after_A.json`: pengukuran engine pada snapshot yang sama; metrik kualitas identik karena Tahap A hanya mengubah sistem pengukuran.
- `scripts/eval.sh`, `.github/workflows/ci.yml`: satu perintah evaluasi dengan gate, serta CI untuk fmt, seluruh test, Clippy `-D warnings`, dan evaluator.
- `README.md` dan `docs/DECISIONS.md`: metodologi, hasil, batas interpretasi, dan alasan desain.

Keputusan desain terperinci: [DECISIONS.md](../DECISIONS.md).

## 2. Korpus dan validasi

| Dataset | Kasus | Ketentuan |
|---|---:|---|
| `qa_factual` | 103 | 31 unanswerable (30,10%), sisanya memiliki target jawaban eksplisit |
| `reasoning_transitive` | 40 | dua premis per kasus |
| `reasoning_compare` | 30 | fakta bernilai numerik dan unit sebanding |
| `reasoning_rules` | 30 | aturan bersyarat dan premis pemicu |
| `contradiction` | 20 | pasangan klaim konflik, target abstain |
| `long_sentence` | 30 | 30 konteks unik berisi 40–150 kata, tiap kasus memiliki pertanyaan target |
| `generation` | 30 | dua fakta sumber atau lebih; instruksi meminta satu kalimat panjang dengan target recall eksplisit |

Setiap JSONL terkunci dalam `manifest.json` dengan checksum SHA-256. Dataset dimuat dari file tersimpan; tidak ada pembuatan kasus saat runtime dan tidak ada layanan eksternal.

## 3. Output nyata sebelum vs sesudah

### Evaluator lama, sebelum Tahap A

Perintah: `cargo run -p vark-eval -- --json`.

```json
{
  "cases": 4,
  "corrected_accuracy": 1.0,
  "consistency": 1.0,
  "consistency_method": "same question after two independent Engine restarts",
  "teacher_calls": 0,
  "total_llm_calls": 0,
  "teacher_call_rate": 0.0,
  "estimated_cost": 0.0
}
```

Angka 100% itu hanya empat kasus dasar yang dapat dijawab; tidak bisa dibandingkan langsung dengan metrik dataset baru yang lebih luas.

### Evaluator baru, setelah Tahap A

Perintah: `cargo run -p vark-eval -- --json --check-baseline`. Hasil aktual tersimpan pada `crates/vark-eval/baselines/after_A.json`.

| Metrik | baseline v0 | sesudah A | Catatan |
|---|---:|---:|---|
| Kasus | 283 | 283 | Dataset/version sama |
| QA faktual accuracy | 0.2500 | 0.2500 | 18 dari 72 kasus answerable |
| Abstain precision | 1.0000 | 1.0000 | Ketika abstain, jawaban memang unanswerable pada set QA ini |
| Abstain recall | 0.1935 | 0.1935 | Sistem masih melewatkan sebagian besar pertanyaan yang tak dapat dijawab |
| Hallucination rate | 0.8482 | 0.8482 | Tinggi; didefinisikan konservatif dari relasi klaim terhadap relasi fakta sumber |
| Transitive accuracy | 0.0000 | 0.0000 | Belum menyimpulkan rantai dua premis pada kasus ini |
| Comparison accuracy | 0.0000 | 0.0000 | Belum membandingkan pasangan numerik pada kasus ini |
| Rule accuracy | 0.8000 | 0.8000 | 24 dari 30 |
| Long-sentence accuracy | 0.4000 | 0.4000 | 12 dari 30 |
| Contradiction detection | 0.0000 | 0.0000 | Belum abstain untuk konflik yang disediakan |
| Generation fact recall | 0.0000 | 0.0000 | Tidak menghasilkan seluruh target fakta |
| Generation unsupported rate | 1.0000 | 1.0000 | Semua keluaran non-abstain pada set ini tidak lolos verifier metrik sumber |
| Decoder perplexity, held-out | 101608.7584 | 101608.7584 | Skor n-gram sangat tinggi; decoder tidak generalisasi pada prosa ini |
| Inferensi p50 | 18.75 ms | 18.71 ms | Hanya waktu `Engine::chat`, tidak termasuk startup/ingest |
| Inferensi p95 | 102.47 ms | 100.98 ms | Dapat berubah antar-run; tidak menjadi gate kualitas |

Nilai `hallucination_rate`/`generation_unsupported_rate` adalah indikator yang konservatif, menggunakan parser relasi sederhana yang sama dengan komponen graph VARK; nilai tersebut bukan audit manusia per klaim. Nilai generation recall nol dan reasoning nol adalah kegagalan terukur, bukan keberhasilan Tahap A.

### Test dan lint

Output `cargo fmt --all -- --check && cargo test --workspace`:

```text
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
(test per crate: API 5, config 2, conversation 4 + restart integration 1, decoder 1,
eval 6, hashing 1, learning 2, LLM 2, memory 11, neurons 1, reasoning 2, tokenizer 4)
```

Total 42 test unit/integration lulus; doc-test tiap crate juga lulus (0 test). Formatter check lulus. Output `cargo clippy --workspace --all-targets -- -D warnings`:

```text
Checking vark-eval v0.1.0 (...)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.70s
```

Tidak ada warning atau error pada kedua pemeriksaan tersebut. Test evaluator mengulang seluruh 283 kasus dan membandingkannya dengan baseline yang dikomit.

## 4. Ablasi

Tidak dilakukan pada Tahap A. Tidak ada perubahan komponen inference yang perlu diablasi; menjalankan ablation sekarang akan mengubah ruang lingkup menjadi eksperimen model sebelum baseline stabil.

## 5. Masalah, risiko, dan hal yang belum berhasil

- Akurasi QA baseline hanya 25%; transitive/comparison/contradiction/generation masing-masing 0%. VARK belum memenuhi sasaran kemampuan yang dijanjikan roadmap.
- Abstain recall masih 19,35%; grounding/verifier perlu diperbaiki sebelum memperluas generasi.
- Parser klaim semantik saat ini sederhana; hallucination rate adalah sinyal konservatif dan perlu kalibrasi/validasi pada dataset manusia sebelum dijadikan klaim eksternal.
- Korpus ini authored, fokus bahasa Indonesia, domain faktual umum dan teknis. Belum ada sumber sitasi eksternal untuk setiap item, paraphrase adversarial, variasi bahasa luas, atau penilaian manusia.
- Perplexity dari n-gram hanya mengukur model decoder sederhana dan tidak menunjukkan kualitas jawaban.
- Tahap H kurikulum/matriks lupa, learning yang terukur, checkpoint versioning/fuzz, API metrics/sleep/open-questions, serta arsitektur penuh masih belum dikerjakan.

## 6. Langkah berikutnya

1. Tahap B: bahasa dasar, normalisasi klaim, pemecahan kalimat, dan property/fuzz tests.
2. Tahap G1/G2: abstain lebih konservatif, verifikasi berbasis bukti yang benar, relasi transitive dan perbandingan numerik.
3. Jalankan korpus ini setelah setiap perubahan; perbarui baseline hanya pada keputusan eksplisit yang disertai laporan metrik dan alasan perubahan dataset.


## Lampiran — output validasi mentah

Perintah validasi final: `cargo fmt --all`, `cargo fmt --all -- --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --release --workspace --locked`, `cargo run -p vark-eval --locked`, dan `scripts/eval.sh`. Berikut output test aktual:

```text
   Compiling vark-eval v0.1.0 (/home/ubuntu/vark/crates/vark-eval)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.97s
     Running unittests src/lib.rs (target/debug/deps/vark_api-3ae74c66f0b20b65)

running 5 tests
test auth::tests::token_comparison_rejects_different_lengths_and_contents ... ok
test auth::tests::auth_requires_nonempty_credentials_and_matches_identity ... ok
test server::tests::user_paths_are_stable_and_collision_resistant ... ok
test server::tests::bearer_auth_is_required_and_bound_to_user_id ... ok
test server::tests::engine_limit_and_request_rate_limit_are_enforced ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests src/bin/server.rs (target/debug/deps/server-207b21e918c06904)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/bin/benchmark.rs (target/debug/deps/benchmark-43c9318c6a77d455)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/vark_cli-00256cb75e89fa65)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_config-27653d5a17232649)
running 2 tests
test tests::legacy_json_receives_explicit_runtime_defaults ... ok
test tests::save_and_load_preserve_service_configuration ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_conversation-1120436906ffe6be)
running 4 tests
test dialogue::claim_verification_tests::every_claim_must_match_an_existing_relation ... ok
test dialogue::claim_verification_tests::empty_unparseable_and_unsupported_claims_do_not_pass ... ok
test dialogue::claim_verification_tests::grounded_evidence_must_come_from_a_matching_active_fact ... ok
test tests::grounded_abstain_and_recover ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/restart.rs (target/debug/deps/restart-2ae15ff557d34bae)
running 1 test
test facts_survive_restart_and_network_checkpoint ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests src/lib.rs (target/debug/deps/vark_decoder-dd92e62ff69f86a9)
running 1 test
test ngram::tests::learns_and_scores ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/vark_eval-7afd95be54b8eed2)
running 6 tests
test metrics::tests::percentile_uses_nearest_rank_for_p50_and_p95 ... ok
test metrics::tests::ratio_handles_empty_denominator_without_nan ... ok
test dataset::tests::malformed_or_unscored_cases_are_rejected ... ok
test dataset::tests::versioned_corpus_meets_minimums_and_has_unique_ids ... ok
test dataset::tests::factual_set_has_required_unanswerable_share_and_long_text_meets_bounds ... ok
test tests::full_engine_evaluation_does_not_regress_from_baseline ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.82s

     Running unittests src/lib.rs (target/debug/deps/vark_hashing-04bec8fb4e0fac59)
running 1 test
test tests::stable_hash ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_learning-67f1741f5aab9e60)
running 2 tests
test training::tests::learns_and_consolidates ... ok
test synapse_learning_tests::learned_weights_drive_propagation_after_checkpoint_reload ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_llm-6d22fd8f22a77ab7)
running 2 tests
test tests::structured_response_parser_requires_choice_and_complete_reply ... ok
test tests::cache_key_is_stable_role_scoped_and_does_not_expose_prompt ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_memory-8627898004687715)
running 11 tests
test security::tests::redaction_covers_separate_and_inline_credentials ... ok
test conversation_log::tests::corrupted_log_is_rejected_during_open ... ok
test security::tests::authenticated_encryption_hides_and_authenticates_plaintext ... ok
test security::tests::rejects_short_storage_keys ... ok
test tests::corruption_is_reported_and_never_dropped_by_a_later_append ... ok
test conversation_log::tests::retention_prunes_records_older_than_configured_days ... ok
test tests::append_fact_writes_metadata_in_one_record_and_redacts_sensitive_text ... ok
test conversation_log::tests::history_is_scoped_by_user_and_secrets_are_redacted ... ok
test tests::persists_recovers_and_snapshots_records ... ok
test conversation_log::tests::concurrent_handles_keep_every_turn_and_allocate_unique_ids ... ok
test tests::concurrent_handles_allocate_unique_ids_without_lost_appends ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

     Running unittests src/lib.rs (target/debug/deps/vark_neurons-17a81037adf45b0c)
running 1 test
test tests::fires_and_recur ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_reasoning-a8ae1b5aa6ba9c69)
running 2 tests
test tests::planner_handles_empty_memory_conservatively ... ok
test tests::planner_and_retriever_use_persisted_records ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_tokenizer-889e1f40ab6fbdbc)
running 4 tests
test bpe::bpe_training_tests::training_does_not_merge_across_word_boundaries ... ok
test bpe::bpe_training_tests::training_applies_multiple_merges_to_each_word ... ok
test tests::tokenizes ... ok
test unigram::tests::segments ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests vark_api
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_config
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_conversation
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_decoder
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_hashing
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_learning
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_memory
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_neurons
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vark_reasoning
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0.00s
   Doc-tests vark_tokenizer
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Output Clippy aktual dan hasil build release:

```text
    Checking vark-eval v0.1.0 (/home/ubuntu/vark/crates/vark-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.62s
    Finished `release` profile [optimized] target(s) in 35.58s
```

Output aktual `cargo run -p vark-eval --locked`:

```text
{
  "schema_version": 1,
  "dataset_version": "vark-eval-2026-10-24-v1",
  "dataset_hashes": {
    "contradiction": "83cb636125a33ccce44db92dd3a5a92925d5d4985d63d7f192b1a14a7f6c3a1e",
    "generation": "89964f0eabdc23bb1b95aa7878368f84e4d212a30853ff0bd66edffc251f2f94",
    "long_sentence": "96d5619908b0ca2913e15ed5c74a8f96d8dc53ff960ec74d87b7dfe683261826",
    "qa_factual": "129cca807d2ce736f0df10e81dbad79dad939a2e06641176270f1bc057e268b4",
    "reasoning_compare": "e33b7f77b36506d38874040dd55655714d8e1cd8cfec2711cf43763b208e453c",
    "reasoning_rules": "46b3b5c95b4050e62873a25acd1523552fb26bf57027658ff5a0382b4bfb427c",
    "reasoning_transitive": "9ffe4e8e6db1a66d0467f529160e417a5d8e4fba1d2722a619393d1b77dc10d1"
  },
  "case_counts": {
    "contradiction": 20,
    "generation": 30,
    "long_sentence": 30,
    "qa_factual": 103,
    "reasoning_compare": 30,
    "reasoning_rules": 30,
    "reasoning_transitive": 40
  },
  "cases": 283,
  "answer_accuracy": 0.25,
  "abstain_precision": 1.0,
  "abstain_recall": 0.1935483870967742,
  "hallucination_rate": 0.8482490272373541,
  "reasoning_accuracy": {
    "long_sentence": 0.4,
    "reasoning_compare": 0.0,
    "reasoning_rules": 0.8,
    "reasoning_transitive": 0.0
  },
  "contradiction_detection_rate": 0.0,
  "generation_fact_recall": 0.0,
  "generation_unsupported_rate": 1.0,
  "perplexity": 101608.75842076582,
  "latency_p50_ms": 18.592609,
  "latency_p95_ms": 104.973333
}

Metric                         Value
------------------------------------------------
answer_accuracy                0.250000
abstain_precision              1.000000
abstain_recall                 0.193548
hallucination_rate             0.848249
long_sentence                  0.400000
reasoning_compare              0.000000
reasoning_rules                0.800000
reasoning_transitive           0.000000
contradiction_detection_rate   0.000000
generation_fact_recall         0.000000
generation_unsupported_rate    1.000000
perplexity                     101608.758421
latency_p50_ms                 18.592609
latency_p95_ms                 104.973333
cases                          283
```

# Progress Tahap B — Perbaikan Bahasa Dasar

**Status:** kriteria Tahap B terpenuhi. Perubahan mencakup splitter kalimat sadar konteks, content tokenizer berbasis stopword eksplisit, ekstraksi relasi multi-pola yang konservatif, atribut/kuantifier relasi yang kompatibel dengan record lama, dan resolusi alias entitas.

## Implementasi

- `vark-conversation/src/language.rs` kini mempertahankan batas kalimat yang berada di desimal, singkatan, inisial, URL/email, tanda kutip, kurung, dan baris baru ganda. `content_tokens()` mempertahankan kata pendek, angka, Unicode, `#`, `_`, hyphen internal, dan negasi; hanya stopword eksplisit di `stopwords.rs` yang dibuang.
- `vark-memory/src/semantic/extraction.rs` mengembalikan 0..n relasi dengan matcher predicate terpisah di `semantic/extraction/patterns.rs` dan helper normalisasi/atribut di `semantic/extraction/helpers.rs`. Koma/konjungsi hanya membelah jika sisi berikutnya berisi predicate yang dikenali; koordinasi dalam objek tetap utuh. Pola if-then menghasilkan relasi `if_then` hanya jika kondisi dan konsekuen tertulis jelas. Parser tidak memakai fallback generik untuk kalimat ambigu.
- `Relation` mendukung nilai numerik, unit, qualifier waktu/tempat/kondisi, dan quantifier negasi (`All`, `Some`, `None`). Default serde menjaga pembacaan record lama. Alias disimpan di entity dan diselesaikan ke ID kanonik.
- Test tabel meliputi batas kalimat (sedikitnya 40 kasus) dan ekstraksi relasi bilingual (sedikitnya 50 kasus), termasuk tiga klausa, daftar objek, pola lokasi/waktu ambigu, angka/unit, negasi, compositional facts, if-then, serta abstain parser.
- Tidak ada dependency baru. File implementasi dipisah berdasarkan tanggung jawab; file terbesar setelah pemisahan adalah matcher predicate, sekitar 224 baris, dan orchestrator ekstraksi sekitar 317 baris.

## Evaluasi dan kriteria

`cargo test --workspace` lulus: **52 passed, 0 failed** (27 ringkasan test suites termasuk doc-tests). `cargo clippy --workspace --all-targets -- -D warnings` lulus tanpa warning. `cargo fmt --all` dan `cargo fmt --all -- --check` selesai sukses. Regression test evaluator serta evaluasi 283 kasus lulus terhadap baseline yang tidak diubah; hasil disimpan di [`after_B.json`](../../crates/vark-eval/baselines/after_B.json).

| Metrik | Baseline v0 | Setelah B | Hasil |
|---|---:|---:|---|
| Answer accuracy | 0.250000 | 0.250000 | tidak turun |
| Abstain precision / recall | 1.000000 / 0.193548 | 1.000000 / 0.193548 | tidak turun |
| Hallucination rate | 0.848249 | 0.848249 | tidak naik |
| Long sentence | 0.400000 | 0.400000 | tidak turun |
| Reasoning comparison | 0.000000 | 0.000000 | tidak turun |
| Reasoning rules | 0.800000 | 0.800000 | tidak turun |
| Reasoning transitive | 0.000000 | 0.000000 | tidak turun |
| Contradiction detection | 0.000000 | 0.000000 | tidak turun |
| Generation fact recall / unsupported | 0.000000 / 1.000000 | 0.000000 / 1.000000 | tidak turun / tidak naik |

**Interpretasi jujur:** Tahap B memperbaiki pemrosesan bahasa dasar, bukan mesin penalaran. Metrik sistem tetap rendah pada perbandingan, transitivitas, kontradiksi, dan generasi panjang; pekerjaan Tahap berikutnya diperlukan untuk meningkatkan kemampuan tersebut. Baseline tidak diganti atau dilonggarkan.

## Keputusan desain

Keputusan konteks–opsi–pilihan–alasan terkait splitter, predicate eksplisit, dan kompatibilitas graph dicatat di [`DECISIONS.md`](../DECISIONS.md). Penambahan predicate untuk fakta faktual yang eksplisit tidak menghidupkan fallback generik; seluruh perubahan tetap memakai pola yang dapat diuji.

## Validasi — output terminal aktual

Log berikut berasal dari putaran final setelah pemecahan file parser; escape warna terminal dihilangkan agar output terbaca. Format/check menghasilkan stdout/stderr kosong dan status keluar 0. Output mentah lengkap Clippy, tes workspace, serta evaluator dilampirkan di bawah.

### `cargo fmt --all`

```text
(stdout/stderr kosong; exit status 0)
```

### `cargo fmt --all -- --check`

```text
(stdout/stderr kosong; exit status 0)
```

### `cargo clippy --workspace --all-targets -- -D warnings`

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.15s
```

### `cargo test --workspace`

```text
   Compiling vark-memory v0.1.0 (/home/ubuntu/vark/crates/vark-memory)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.39s
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

running 8 tests
test dialogue::claim_verification_tests::every_claim_must_match_an_existing_relation ... ok
test dialogue::claim_verification_tests::empty_unparseable_and_unsupported_claims_do_not_pass ... ok
test language::tests::content_tokens_preserve_short_words_digits_negation_and_unicode ... ok
test language::tests::token_normalization_is_case_insensitive_and_counts_unicode_characters ... ok
test dialogue::claim_verification_tests::grounded_evidence_must_come_from_a_matching_active_fact ... ok
test stopwords::tests::negative_words_are_preserved_while_explicit_stopwords_are_filtered ... ok
test language::tests::context_aware_sentence_boundary_table ... ok
test tests::grounded_abstain_and_recover ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

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
test dataset::tests::malformed_or_unscored_cases_are_rejected ... ok
test metrics::tests::percentile_uses_nearest_rank_for_p50_and_p95 ... ok
test metrics::tests::ratio_handles_empty_denominator_without_nan ... ok
test dataset::tests::versioned_corpus_meets_minimums_and_has_unique_ids ... ok
test dataset::tests::factual_set_has_required_unanswerable_share_and_long_text_meets_bounds ... ok
test tests::full_engine_evaluation_does_not_regress_from_baseline ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.49s

     Running unittests src/lib.rs (target/debug/deps/vark_hashing-04bec8fb4e0fac59)

running 1 test
test tests::stable_hash ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_learning-67f1741f5aab9e60)

running 2 tests
test training::tests::learns_and_consolidates ... ok
test synapse_learning_tests::learned_weights_drive_propagation_after_checkpoint_reload ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests src/lib.rs (target/debug/deps/vark_llm-6d22fd8f22a77ab7)

running 2 tests
test tests::structured_response_parser_requires_choice_and_complete_reply ... ok
test tests::cache_key_is_stable_role_scoped_and_does_not_expose_prompt ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/vark_memory-8627898004687715)

running 17 tests
test security::tests::redaction_covers_separate_and_inline_credentials ... ok
test security::tests::authenticated_encryption_hides_and_authenticates_plaintext ... ok
test security::tests::rejects_short_storage_keys ... ok
test conversation_log::tests::corrupted_log_is_rejected_during_open ... ok
test semantic::extraction::tests::comparisons_and_attributes_capture_numeric_value_and_unit ... ok
test semantic::extraction::tests::time_and_condition_are_recorded_as_relation_qualifiers ... ok
test semantic::extraction::tests::quantified_negation_keeps_scope_and_polarity ... ok
test semantic::tests::aliases_resolve_to_the_same_entity_id_in_graph_queries ... ok
test semantic::tests::legacy_entity_and_relation_records_deserialize_with_defaults ... ok
test conversation_log::tests::retention_prunes_records_older_than_configured_days ... ok
test semantic::extraction::tests::bilingual_relation_table_accepts_only_supported_patterns ... ok
test tests::corruption_is_reported_and_never_dropped_by_a_later_append ... ok
test tests::append_fact_writes_metadata_in_one_record_and_redacts_sensitive_text ... ok
test conversation_log::tests::history_is_scoped_by_user_and_secrets_are_redacted ... ok
test tests::persists_recovers_and_snapshots_records ... ok
test conversation_log::tests::concurrent_handles_keep_every_turn_and_allocate_unique_ids ... ok
test tests::concurrent_handles_allocate_unique_ids_without_lost_appends ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

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
test bpe::bpe_training_tests::training_applies_multiple_merges_to_each_word ... ok
test bpe::bpe_training_tests::training_does_not_merge_across_word_boundaries ... ok
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

   Doc-tests vark_llm

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

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests vark_tokenizer

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### `cargo run -p vark-eval -- --check-baseline --save-report crates/vark-eval/baselines/after_B.json`

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.18s
     Running `target/debug/vark-eval --check-baseline --save-report crates/vark-eval/baselines/after_B.json`
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
  "latency_p50_ms": 19.370082,
  "latency_p95_ms": 102.69666
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
latency_p50_ms                 19.370082
latency_p95_ms                 102.696660
cases                          283
```

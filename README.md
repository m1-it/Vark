# VARK — Arsitektur Hibrida SNN-LIF

VARK adalah proyek Rust modular untuk eksperimen jaringan spiking LIF, hashing fitur, tokenizer subword, memori episodik persisten, STDP/homeostasis, reasoning planner, engine inferensi, serta arsitektur guru–murid.

## Menjalankan

```bash
cargo test --workspace
cargo run -p vark-cli --bin vark-cli -- "neuron spiking bahasa Indonesia"
cargo run -p vark-cli --bin vark-cli -- --chat
cargo run -p vark-cli --bin benchmark
cargo run -p vark-api --bin server
cargo run -p vark-eval
```

Mode `--chat` menerima kalimat panjang per baris. Pertanyaan async mengaktifkan LIF, planner, BPE/Unigram, n-gram, lalu router murid dan guru sesuai confidence. Gunakan `koreksi:`, `salah, yang benar ...`, `benar`, `salah`, atau `lupakan itu` untuk pembelajaran terarah.

Server membaca konfigurasi dari environment. API menolak startup jika bearer token atau kunci penyimpanan tidak tersedia. Bind default hanya ke loopback:

```bash
export VARK_API_TOKEN="$(openssl rand -hex 32)"
export VARK_STORAGE_KEY="$(openssl rand -base64 48)"
VARK_CONFIG=config.json cargo run -p vark-api --bin server
```

Untuk banyak pengguna, gunakan `VARK_API_TOKENS` sebagai JSON object token-ke-user-id dan jangan set `VARK_API_TOKEN` bersamaan. Request memakai `Authorization: Bearer <token>`; `user_id` di body opsional, dan jika dikirim harus sama dengan identitas yang terikat ke token. Override bind hanya dengan `VARK_API_BIND`; endpoint publik harus ditempatkan di belakang TLS reverse proxy. Batas default API adalah 60 request per menit per user, 128 engine aktif, body 64 KiB, dan teks 16 KiB. Jangan commit token atau `VARK_STORAGE_KEY`; kehilangan key membuat record terenkripsi tidak dapat dipulihkan.

## Struktur

```text
crates/
├── vark-conversation/src/  # engine, dialogue, language, types
├── vark-neurons/src/       # network LIF dan CSR synapses
├── vark-tokenizer/src/     # hashing, BPE, Unigram, corpus balance
├── vark-decoder/src/       # decoder token dan n-gram
├── vark-memory/src/        # episodik, conversation log, semantic graph
├── vark-learning/src/      # STDP, replay/EWC, training
├── vark-llm/src/           # backend LLM dan router guru-murid
├── vark-api/src/           # engine, Axum router, server
└── vark-eval/              # runner, metrik, baseline, dataset JSONL ber-hash
```

`vark-config` mengatur konfigurasi jaringan, memori, user, guru, murid, retry, token budget, threshold, dan biaya. `vark-conversation` menyatukan retrieval, graph, LIF, tokenizer, decoder, router, feedback, serta log. `vark-memory` menyediakan JSONL append-only, recovery, FactNode, graph semantik, conversation log, redaksi, kompaktasi, dan backup.

## Backend guru–murid

Konfigurasi menggunakan dua slot terpisah:

```json
{
  "user_id": "user-001",
  "teacher": {
    "endpoint": "https://api.openai.com/v1/chat/completions",
    "model": "gpt-4o-mini",
    "api_key_env": "OPENAI_API_KEY",
    "max_retries": 2,
    "cost_per_1k_tokens": 0.002
  },
  "student": {
    "endpoint": "http://127.0.0.1:8080/v1/chat/completions",
    "model": "Qwen2.5-7B-Instruct",
    "api_key_env": "QWEN_API_KEY",
    "max_retries": 1,
    "cost_per_1k_tokens": 0.0
  },
  "daily_token_limit": 100000,
  "teacher_confidence_threshold": 0.65
}
```

Kunci guru hanya dibaca dari environment guru dan kunci murid hanya dari environment murid. Fallback legacy hanya memakai `VARK_LEGACY_LLM_API_KEY`. Cache respons hanya hidup di memori proses dan menggunakan hash prompt sebagai key; prompt, jawaban cache, dan usage statistics tidak disimpan ke file. Router juga menyediakan retry terbatas, token usage, estimasi biaya, daily budget, dan statistik jumlah panggilan.

Jawaban guru diberi status `Tentative`, klaimnya masuk sebagai fakta Tentative, dan tidak langsung menjadi `Confirmed`. Bila jawaban murid dan guru berbeda secara relasi klaim, perbedaannya dicatat dalam reasoning trace.

## Log dan reasoning

File `<memory_db>.conversation.jsonl` menyimpan record append-only dengan schema:

```text
turn_id, user_id, role, text, timestamp, feedback, schema_version
```

Role yang digunakan adalah `user`, `murid`, `guru`, `feedback`, dan `reasoning`. Email, nomor telepon, token panjang, `api_key`, password, dan bearer secret disamarkan sebelum ditulis. Pertanyaan dan fakta juga disamarkan sebelum dikirim ke backend eksternal. Di Unix, direktori data dibuat owner-only (`0700`) dan file owner-only (`0600`). Jika `VARK_STORAGE_KEY` tersedia, setiap record JSONL dienkripsi dan diautentikasi dengan AES-256-GCM; file legacy dimigrasikan saat dibuka. Lock file per dataset menyerialkan append, compact, backup, dan read-modify-write lintas proses. Baris JSON/checksum/AEAD yang rusak menyebabkan error eksplisit dan tidak diabaikan atau ditimpa. Log percakapan dipertahankan selama 180 hari; record fakta tetap tersedia sampai dihapus eksplisit. `ConversationLog::compact` dan `ConversationLog::backup` tersedia untuk pengelolaan data.

Reasoning trace terstruktur menyimpan pertanyaan, fakta yang dipakai, kesimpulan, dan feedback. Planner memecah langkah awal dan kasus reasoning serupa diambil dari log untuk konteks adaptasi pada pertanyaan berikutnya. Graph menyediakan relasi typed, negasi, konflik, dan reachability multi-hop. N-gram dipakai untuk perplexity, kelancaran, dan fallback; n-gram bukan mesin penalaran.

## Tokenizer dan decoder

Saat startup, corpus fakta dan percakapan melatih BPE, Unigram, dan decoder n-gram runtime. Jalur chat memanggil BPE/Unigram untuk assembly konsep dan n-gram untuk perplexity. Registry assembly LIF dikunci ke hash string subword stabil, bukan ID tokenizer, sehingga retraining tidak memutus asosiasi konsep.

## Pemrosesan bahasa dan graph semantik

`split_sentences()` menjaga desimal, singkatan, inisial, URL, email, tanda kutip, dan kurung saat memisahkan kalimat. `content_tokens()` menggunakan huruf kecil Unicode serta daftar stopword eksplisit Indonesia/Inggris; token pendek, angka, tanda `#`, `_`, hyphen, dan negasi dipertahankan. Tidak ada ambang byte-length yang membuang token secara diam-diam.

`vark-memory/src/semantic/extraction.rs` mengekstrak nol hingga beberapa relasi per input dengan predicate yang dikenali secara eksplisit. Koma/konjungsi membelah klausa hanya jika segmen sesudahnya juga berisi predicate yang dikenali, sehingga daftar kata di dalam objek tidak otomatis berubah menjadi relasi. Relasi mendukung predicate dasar bilingual, nilai/unit, qualifier tempat/waktu/kondisi, negasi berkuantor, serta bentuk `if_then`; pola ambigu menghasilkan nol relasi. `KnowledgeGraph` menyelesaikan alias ke ID entitas kanonik, dan field baru memakai default serde untuk membaca record lama.

## LIF dan pembelajaran

Jaringan LIF memakai tepat 10 incoming edge unik per neuron saat inisialisasi, batas keras fan-in 50, batch builder CSR, pertumbuhan co-active dengan penggantian bobot terlemah, registry konsep terserialisasi, STDP sepanjang trace timestep, reward konfirmasi/bantahan, dan checkpoint yang dimuat kembali saat start. Pertanyaan async sekarang melewati aktivasi LIF sebelum router memilih jawaban.

## Training

Perintah training memakai sesi terpisah dan kurikulum bertahap:

```bash
cargo run -p vark-cli --bin vark-cli -- train "Bahasa Rust"
VARK_TRAIN_MAX_QUESTIONS=3 cargo run -p vark-cli --bin vark-cli -- train config.json "Bahasa Rust"
```

Kurikulum mencakup `apa itu`, `fungsi`, `mengapa`, `beda`, dan `contoh`. Batas pertanyaan berasal dari `VARK_TRAIN_MAX_QUESTIONS`; batas token dan biaya tetap dikendalikan router.

## Isolasi user

Token memetakan ke satu `user_id` yang telah diotorisasi; klien tidak dapat memilih identitas lain. Nama folder disk diturunkan dari SHA-256 identitas agar perbedaan slash, titik, atau karakter lain tidak menimbulkan tabrakan path. Server memakai engine/lock terpisah per user, batas jumlah engine dan request, serta tidak menahan mutex global selama inferensi atau panggilan LLM.

## Evaluasi

```bash
cargo run -p vark-eval -- --json
scripts/eval.sh
```

Runner menggunakan snapshot lokal `crates/vark-eval/data/` dengan checksum SHA-256 pada `manifest.json`: 103 QA faktual (31 tidak dapat dijawab), 40 transitive, 30 perbandingan, 30 aturan, 20 kontradiksi, 30 konteks panjang unik, dan 30 tugas komposisi/generasi dengan target satu kalimat panjang. Semua kasus menjalankan `Engine` VARK; korpus tidak memanggil layanan model eksternal. Laporan mencakup akurasi QA, precision/recall abstain, tingkat klaim tak didukung, akurasi tiap jenis reasoning, deteksi kontradiksi, recall fakta generasi, perplexity decoder n-gram pada teks tahan (held-out), serta p50/p95 latensi inferensi.

`crates/vark-eval/baselines/baseline_v0.json` adalah pengukuran tersimpan atas implementasi engine saat ini. `scripts/eval.sh` menjalankan keseluruhan korpus dan menolak penurunan metrik utama lebih dari 2 poin persentase; tingkat hallucination dan klaim generasi tak didukung tidak boleh meningkat. Jalankan `cargo run -p vark-eval` untuk JSON plus tabel, atau `cargo run -p vark-eval -- --json` untuk JSON saja. Baseline tidak mengasumsikan hasil sempurna: pada snapshot awal, akurasi QA 25%, transitive 0%, comparison 0%, rule 80%, long-sentence 40%, deteksi kontradiksi 0%, recall fakta generasi 0%, dan hallucination rate 84.82%. Ini mengukur keterbatasan kemampuan saat ini, bukan bukti bahwa sistem sudah memenuhi target produk. Hasil lama empat kasus tetap dicatat di laporan Tahap A sebagai baseline historis yang tidak sebanding langsung dengan korpus baru.

Benchmark jaringan di CLI membangun CSR sparse dengan fan-in terukur, melakukan warm-up dan repetisi, lalu melaporkan median waktu langkah, jumlah edge, spikes, serta kapasitas buffer aktual.

## Artefak runtime

Memori utama berada di `data/memory.jsonl`, log percakapan di `data/memory.jsonl.conversation.jsonl`, dan checkpoint jaringan di `checkpoints/`. Cache LLM dan statistik usage hanya berada di memori proses. Semua artefak runtime diabaikan oleh `.gitignore` dan tidak masuk arsip rilis.

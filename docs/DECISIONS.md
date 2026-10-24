# Keputusan desain VARK

## A — Fondasi evaluasi

### A-1 — Format dan versi dataset

- **Konteks:** dataset harus tetap dan dapat diaudit di CI; perubahan konten tidak boleh mengubah skor diam-diam.
- **Opsi:** mendefinisikan kasus di kode runner, membangunnya saat runtime, atau menyimpan snapshot JSONL per kategori.
- **Pilihan:** tujuh file JSONL di `crates/vark-eval/data/`, diregistrasikan oleh manifest versi dengan jumlah record dan SHA-256.
- **Alasan:** teks evaluasi dapat ditinjau/didiff secara langsung, hash memaksa pembaruan versi secara eksplisit, dan runner tidak membuat data secara dinamis.

### A-2 — Jalur inferensi yang diukur

- **Konteks:** skor harus menunjukkan perilaku sistem VARK yang berjalan, bukan hasil dari provider eksternal atau implementasi pengganti.
- **Opsi:** mengukur helper algoritme secara terisolasi saja, memasang mock Engine, atau menjalankan `vark_api::Engine` dengan fakta dan query dari corpus.
- **Pilihan:** setiap kasus menjalankan Engine nyata dan memakai storage sementara yang terisolasi; kontradiksi masuk melalui jalur pernyataan biasa, sementara premis evaluasi lain diberikan sebagai koreksi eksplisit.
- **Alasan:** hasil merefleksikan integrasi retrieval/grounding yang dipakai aplikasi, tetap dapat dijalankan lokal, dan tidak membutuhkan credential maupun panggilan model.

### A-3 — Metrik dan serialisasi hasil

- **Konteks:** hasil perlu dipakai seragam oleh CLI, regression tests, CI, dan perbandingan antartahap.
- **Opsi:** hanya mencetak ringkasan teks, membuat format berbeda untuk tiap metric, atau menerbitkan satu report JSON terstruktur beserta tabel manusia.
- **Pilihan:** `EvalReport` tunggal dengan JSON (`--json`) atau JSON plus tabel (default); perplexity dilatih pada QA dan diukur pada set long-sentence yang terpisah.
- **Alasan:** JSON mudah dibandingkan/diproses otomatis, tabel memudahkan inspeksi, dan pemisahan held-out mencegah decoder diukur pada prosa yang sama dengan data latihnya.

### A-4 — Regression gate

- **Konteks:** perubahan perilaku berikutnya tidak boleh menutupi penurunan kemampuan atau menambah klaim tak didukung.
- **Opsi:** hanya mencatat metrik tanpa gate, memakai threshold absolut tetap, atau membandingkan terhadap snapshot sebelumnya.
- **Pilihan:** `cargo test -p vark-eval` dan `scripts/eval.sh` membandingkan metrik utama dengan `baseline_v0.json`; toleransi turunnya metrik keberhasilan maksimal 2 poin persentase, sedangkan kenaikan hallucination dan generation-unsupported langsung gagal. CI menjalankan fmt, test, Clippy `-D warnings`, dan evaluasi.
- **Alasan:** gate berjangkar pada kemampuan nyata saat ini; toleransi kecil menyerap variasi minor, dan metrik keamanan jawaban tidak diberi toleransi kenaikan.

### A-5 — Interpretasi baseline

- **Konteks:** evaluasi lama hanya memiliki empat pertanyaan answerable dan melaporkan skor sempurna.
- **Opsi:** menganggap skor lama sebagai bukti kemampuan umum, membuangnya, atau menyimpannya sebagai catatan historis terpisah dari corpus baru.
- **Pilihan:** simpan hasil empat kasus dalam progress report; `baseline_v0.json` berisi pengukuran pertama pada corpus 283 kasus.
- **Alasan:** kedua pengukuran tidak sebanding; menyebut keterbatasan ini mencegah kesimpulan menyesatkan. Tahap A hanya memperluas pengukuran, tidak mengubah algoritme Engine.

Langkah setelah Tahap A: prioritaskan abstain/grounding dan perbaikan reasoning transitive/perbandingan, lalu pertahankan corpus dan baseline sebagai regression gate.

use super::{content_tokens, split_sentences};
use vark_tokenizer::Tokenizer;

#[test]
fn context_aware_sentence_boundary_table() {
    let cases: &[(&str, &[&str])] = &[
        ("Halo. Dunia.", &["Halo.", "Dunia."]),
        ("Halo! Dunia.", &["Halo!", "Dunia."]),
        ("Halo? Dunia.", &["Halo?", "Dunia."]),
        ("Halo… Dunia.", &["Halo…", "Dunia."]),
        (
            "Suhu 3.14 tinggi. Lanjut.",
            &["Suhu 3.14 tinggi.", "Lanjut."],
        ),
        (
            "Populasi 1.000.000 orang. Berlanjut.",
            &["Populasi 1.000.000 orang.", "Berlanjut."],
        ),
        (
            "dr. Budi datang. Lalu pulang.",
            &["dr. Budi datang.", "Lalu pulang."],
        ),
        (
            "Prof. Ani bicara. Ia pergi.",
            &["Prof. Ani bicara.", "Ia pergi."],
        ),
        ("mis. contoh. Berikutnya.", &["mis. contoh.", "Berikutnya."]),
        ("dll. Ada lagi.", &["dll. Ada lagi."]),
        ("dsb. Selesai.", &["dsb. Selesai."]),
        ("yg. dimaksud. Teks.", &["yg. dimaksud.", "Teks."]),
        (
            "Nama a.k.a. alias. Berikut.",
            &["Nama a.k.a. alias.", "Berikut."],
        ),
        (
            "Gunakan e.g. contoh. Lanjut.",
            &["Gunakan e.g. contoh.", "Lanjut."],
        ),
        (
            "Baca i.e. penjelasan. Lanjut.",
            &["Baca i.e. penjelasan.", "Lanjut."],
        ),
        (
            "Mr. Smith pergi. Kembali.",
            &["Mr. Smith pergi.", "Kembali."],
        ),
        (
            "J. K. Rowling menulis. Terkenal.",
            &["J. K. Rowling menulis.", "Terkenal."],
        ),
        (
            "A. B. Smith setuju. Benar.",
            &["A. B. Smith setuju.", "Benar."],
        ),
        (
            "Kunjungi https://example.com/path. Lanjut.",
            &["Kunjungi https://example.com/path.", "Lanjut."],
        ),
        (
            "Buka www.example.com/docs. Lanjut.",
            &["Buka www.example.com/docs.", "Lanjut."],
        ),
        (
            "Email alice.smith@example.com dikirim. Selesai.",
            &["Email alice.smith@example.com dikirim.", "Selesai."],
        ),
        ("\"Selesai.\" Ia pergi.", &["\"Selesai.\"", "Ia pergi."]),
        ("(Selesai.) Berikutnya.", &["(Selesai.)", "Berikutnya."]),
        ("[Selesai!] Berikutnya.", &["[Selesai!]", "Berikutnya."]),
        ("{Selesai?} Berikutnya.", &["{Selesai?}", "Berikutnya."]),
        (
            "Sudah selesai.\n\nBab baru.",
            &["Sudah selesai.", "Bab baru."],
        ),
        (
            "Paragraf satu\n\nparagraf dua",
            &["Paragraf satu", "paragraf dua"],
        ),
        (
            "Kalimat satu\nlanjutan kecil.",
            &["Kalimat satu\nlanjutan kecil."],
        ),
        ("Tunggu!\nLalu lanjut.", &["Tunggu!", "Lalu lanjut."]),
        ("Benar? 42 adalah angka.", &["Benar?", "42 adalah angka."]),
        ("Nilai 42. Jawaban benar.", &["Nilai 42.", "Jawaban benar."]),
        (
            "Nilai 42.5 cukup. Lanjut.",
            &["Nilai 42.5 cukup.", "Lanjut."],
        ),
        (
            "Ini kalimat.\" Lanjutkan.",
            &["Ini kalimat.\"", "Lanjutkan."],
        ),
        (
            "Nama Dr. Smith hadir. Ia duduk.",
            &["Nama Dr. Smith hadir.", "Ia duduk."],
        ),
        (
            "Contoh e.g. satu, dua. Hasil.",
            &["Contoh e.g. satu, dua.", "Hasil."],
        ),
        (
            "Data 2.500,75 valid. Berikut.",
            &["Data 2.500,75 valid.", "Berikut."],
        ),
        (
            "Kata berakhir! berikutnya tetap sama.",
            &["Kata berakhir! berikutnya tetap sama."],
        ),
        (
            "Pertanyaan? jawaban masih satu alur.",
            &["Pertanyaan? jawaban masih satu alur."],
        ),
        (
            "Tuntas… Berikutnya dimulai.",
            &["Tuntas…", "Berikutnya dimulai."],
        ),
        ("Selesai.) Berikutnya.", &["Selesai.)", "Berikutnya."]),
        (
            "Dr. A. B. berbicara. Lalu pergi.",
            &["Dr. A. B. berbicara.", "Lalu pergi."],
        ),
        (
            "URL https://foo.bar/a.b/c tetap utuh. Selesai.",
            &["URL https://foo.bar/a.b/c tetap utuh.", "Selesai."],
        ),
        (
            "Alamat x.y+tag@foo.co.id valid. Berikut.",
            &["Alamat x.y+tag@foo.co.id valid.", "Berikut."],
        ),
        ("Akhir tanpa tanda", &["Akhir tanpa tanda"]),
        (
            "  Spasi awal.  Spasi baru.  ",
            &["Spasi awal.", "Spasi baru."],
        ),
    ];

    assert!(cases.len() >= 40);
    for (input, expected) in cases {
        let actual = split_sentences(input);
        assert_eq!(actual, *expected, "input: {input:?}");
        assert_eq!(
            actual.join(" ").split_whitespace().collect::<Vec<_>>(),
            input.split_whitespace().collect::<Vec<_>>(),
            "split must preserve all source text: {input:?}"
        );
    }
}

#[test]
fn content_tokens_preserve_short_words_digits_negation_and_unicode() {
    let tokenizer = Tokenizer::new(128);
    let tokens = content_tokens(&tokenizer, "AI di ML, OS, 3D, 42, C# dan kata-kata_model!");
    assert!(tokens.contains(&"ai".to_owned()));
    assert!(tokens.contains(&"ml".to_owned()));
    assert!(tokens.contains(&"os".to_owned()));
    assert!(tokens.contains(&"3d".to_owned()));
    assert!(tokens.contains(&"42".to_owned()));
    assert!(tokens.contains(&"c#".to_owned()));
    assert!(tokens.contains(&"kata-kata_model".to_owned()));
    assert!(!tokens.contains(&"di".to_owned()));

    let negative = content_tokens(
        &tokenizer,
        "bukan kucing tidak kecil; belum pergi tanpa izin, not safe, no risk, never again",
    );
    for negation in ["bukan", "tidak", "belum", "tanpa", "not", "no", "never"] {
        assert!(
            negative.contains(&negation.to_owned()),
            "negation {negation} was filtered"
        );
    }
}

#[test]
fn token_normalization_is_case_insensitive_and_counts_unicode_characters() {
    let tokens = content_tokens(&Tokenizer::new(64), "ÉLÈVE AI");
    assert_eq!(tokens, ["élève", "ai"]);
    assert_eq!(content_tokens(&Tokenizer::new(64), "é"), ["é"]);
}

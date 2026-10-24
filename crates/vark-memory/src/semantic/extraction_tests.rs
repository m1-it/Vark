use super::extract_relations;
use crate::semantic::Quantifier;

#[test]
fn bilingual_relation_table_accepts_only_supported_patterns() {
    let cases: &[(&str, &[&str])] = &[
        ("Rust adalah bahasa sistem.", &["is_a"]),
        ("Jakarta merupakan ibu kota Indonesia.", &["is_a"]),
        ("Bumi ialah sebuah planet.", &["is_a"]),
        ("Rust is a systems language.", &["is"]),
        ("Birds are animals.", &["is"]),
        ("The singer was born in London.", &["born_in"]),
        ("Pohon memiliki akar.", &["has"]),
        ("Anjing punya ekor.", &["has"]),
        ("Alice has a cat.", &["has"]),
        ("They have two books.", &["has"]),
        ("Jakarta terletak di Indonesia.", &["located_in"]),
        ("Bandung berada di Jawa Barat.", &["located_in"]),
        ("The Eiffel Tower is in Paris.", &["located_in"]),
        ("Bung Karno lahir di Surabaya.", &["born_in"]),
        ("Newton was born in England.", &["born_in"]),
        ("Apel berwarna merah.", &["has_color"]),
        ("The car is colored blue.", &["has_color"]),
        ("Hujan menyebabkan banjir.", &["causes"]),
        ("Smoking causes cancer.", &["causes"]),
        ("Roda bagian dari mobil.", &["part_of"]),
        ("A wheel is part of a car.", &["part_of"]),
        ("Everest lebih tinggi dari K2.", &["greater_than"]),
        ("Pohon A lebih besar dari pohon B.", &["greater_than"]),
        ("Suhu A lebih rendah dari suhu B.", &["less_than"]),
        ("Value X is greater than value Y.", &["greater_than"]),
        ("Small is less than big.", &["less_than"]),
        ("Value X is larger than value Y.", &["greater_than"]),
        ("Kucing tidak bisa terbang.", &["can"]),
        ("Not all birds can fly.", &["can"]),
        ("Semua burung bisa terbang.", &["can"]),
        ("Tidak ada burung yang bisa terbang.", &["can"]),
        ("No birds can fly.", &["can"]),
        ("Birds cannot fly.", &["can"]),
        ("Kucing bukan mamalia.", &["is_a"]),
        ("Kucing di hari Senin.", &[]),
        ("Pertemuan di bulan Mei.", &[]),
        ("Kota di pagi hari.", &[]),
        ("Kucing berlari cepat.", &[]),
        ("Mungkin kucing di bawah meja.", &[]),
        ("Jakarta adalah ibu kota Indonesia, memiliki penduduk 10 juta, dan terletak di Indonesia.", &["is_a", "has", "located_in"]),
        ("Rust adalah bahasa, Go memiliki goroutine, dan Python memiliki list.", &["is_a", "has", "has"]),
        ("Bandung berada di Jawa Barat serta Jakarta berada di Indonesia.", &["located_in", "located_in"]),
        ("Hujan menyebabkan banjir sehingga jalan memiliki genangan.", &["causes", "has"]),
        ("Kopi berwarna cokelat tetapi teh berwarna hijau.", &["has_color", "has_color"]),
        ("Cats and dogs are mammals.", &["is", "is"]),
        ("Buku itu bagian dari koleksi.", &["part_of"]),
        ("Plants have leaves.", &["has"]),
        ("Air dapat menyebabkan erosi.", &["causes"]),
        ("Paus lebih besar 10 meter dari lumba-lumba.", &["greater_than"]),
        ("Gunung A lebih pendek 300 meter dari gunung B.", &["less_than"]),
        ("Benda X located in Paris.", &["located_in"]),
        ("Bumi mengorbit Matahari.", &["orbits"]),
        ("Air murni tersusun dari hidrogen dan oksigen.", &["composed_of"]),
        ("Gaya gravitasi menarik benda bermassa.", &["attracts"]),
        ("DNA menyimpan informasi genetik.", &["stores"]),
        ("SQL dipakai untuk mengelola basis data relasional.", &["used_for"]),
        ("Jika benda bergerak melingkar, arah kecepatannya berubah.", &["if_then"]),
        ("Jika seseorang berolahraga, kebutuhan oksigen meningkat.", &["if_then"]),
        ("Jika garam larut dalam air, terbentuk larutan garam.", &["if_then"]),
        ("If rain falls, roads become wet.", &["if_then"]),
        ("Kucing tersebut mungkin lucu.", &[]),
        ("Kita bertemu di hari Senin.", &[]),
    ];

    assert!(cases.len() >= 50);
    for (sentence, expected) in cases {
        let relations = extract_relations(sentence, "table test");
        let actual = relations
            .iter()
            .map(|relation| relation.predicate.as_str())
            .collect::<Vec<_>>();
        assert_eq!(actual, *expected, "sentence: {sentence}");
    }
}

#[test]
fn quantified_negation_keeps_scope_and_polarity() {
    let not_all = extract_relations("Tidak semua burung bisa terbang.", "test");
    let individual_negative = extract_relations("Burung tidak bisa terbang.", "test");
    let none = extract_relations("Tidak ada burung yang bisa terbang.", "test");
    assert_eq!(not_all[0].subject, "burung");
    assert!(!not_all[0].polarity);
    assert_eq!(not_all[0].quantifier, Some(Quantifier::Some));
    assert_eq!(individual_negative[0].quantifier, None);
    assert!(!individual_negative[0].polarity);
    assert_eq!(none[0].quantifier, Some(Quantifier::None));
    assert!(!none[0].polarity);
}

#[test]
fn comparisons_and_attributes_capture_numeric_value_and_unit() {
    let comparison = extract_relations("Gunung A lebih tinggi 8849 meter dari Gunung B.", "test");
    assert_eq!(comparison[0].value, Some(8849.0));
    assert_eq!(comparison[0].unit.as_deref(), Some("meter"));
    let attribute = extract_relations("Gunung Everest memiliki ketinggian 8849 meter.", "test");
    assert_eq!(attribute[0].value, Some(8849.0));
    assert_eq!(attribute[0].unit.as_deref(), Some("meter"));
}

#[test]
fn time_and_condition_are_recorded_as_relation_qualifiers() {
    let relation = extract_relations(
        "Tanaman memiliki daun di Indonesia pada musim hujan jika cukup air.",
        "test",
    );
    assert!(relation[0]
        .qualifiers
        .iter()
        .any(|(kind, value)| kind == "tempat" && value.starts_with("indonesia")));
    assert!(relation[0]
        .qualifiers
        .iter()
        .any(|(kind, value)| kind == "waktu" && value.starts_with("musim hujan")));
    assert!(relation[0]
        .qualifiers
        .iter()
        .any(|(kind, value)| kind == "kondisi" && value.starts_with("cukup air")));
}

#[derive(Clone, Copy)]
pub(super) struct Pattern {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) object_start: usize,
    pub(super) predicate: &'static str,
    pub(super) place_required: bool,
}

pub(super) fn find_pattern(words: &[String]) -> Option<Pattern> {
    for index in 0..words.len() {
        let word = words[index].as_str();
        let next = words.get(index + 1).map(String::as_str).unwrap_or_default();
        let pattern = if matches!(word, "lebih" | "less" | "greater" | "larger" | "smaller") {
            let comparison = if word == "lebih" {
                match next {
                    "kecil" | "rendah" | "pendek" | "muda" | "sedikit" => "less_than",
                    "besar" | "tinggi" | "panjang" | "tua" | "banyak" => "greater_than",
                    _ => continue,
                }
            } else if matches!(word, "less" | "smaller") {
                "less_than"
            } else {
                "greater_than"
            };
            let Some(marker) = words[index + 1..]
                .iter()
                .position(|candidate| matches!(candidate.as_str(), "dari" | "than"))
                .map(|offset| index + 1 + offset)
            else {
                continue;
            };
            Pattern {
                start: index,
                end: marker,
                object_start: marker + 1,
                predicate: comparison,
                place_required: false,
            }
        } else if matches!(word, "lahir" | "born") && matches!(next, "di" | "in") {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: "born_in",
                place_required: true,
            }
        } else if matches!(word, "terletak" | "berada" | "located") && matches!(next, "di" | "in") {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: "located_in",
                place_required: true,
            }
        } else if (word == "bagian" && next == "dari") || (word == "part" && next == "of") {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: "part_of",
                place_required: false,
            }
        } else if (matches!(word, "tersusun" | "composed") && matches!(next, "dari" | "of"))
            || (word == "consists" && next == "of")
        {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: "composed_of",
                place_required: false,
            }
        } else if matches!(word, "mengorbit" | "mengelilingi" | "orbits" | "orbit") {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "orbits",
                place_required: false,
            }
        } else if matches!(word, "menarik" | "attracts" | "attract") {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "attracts",
                place_required: false,
            }
        } else if matches!(word, "menyimpan" | "stores" | "store") {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "stores",
                place_required: false,
            }
        } else if (word == "dipakai" && next == "untuk") || (word == "used" && next == "for") {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: "used_for",
                place_required: false,
            }
        } else if matches!(word, "berwarna" | "colored" | "coloured") {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "has_color",
                place_required: false,
            }
        } else if matches!(word, "menyebabkan" | "mengakibatkan" | "causes" | "cause") {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "causes",
                place_required: false,
            }
        } else if matches!(word, "memiliki" | "punya" | "has" | "have" | "had") {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "has",
                place_required: false,
            }
        } else if matches!(word, "bisa" | "dapat" | "can" | "cannot" | "can't")
            && !(word == "dapat" && matches!(next, "menyebabkan" | "mengakibatkan"))
        {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "can",
                place_required: false,
            }
        } else if matches!(word, "do" | "does") && next == "not" {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: "can",
                place_required: false,
            }
        } else if word == "di" || word == "in" {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: "located_in",
                place_required: true,
            }
        } else if matches!(
            word,
            "adalah" | "ialah" | "merupakan" | "is" | "are" | "was" | "were"
        ) && next == "not"
            && words.get(index + 2).is_some()
        {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: if matches!(word, "is" | "are" | "was" | "were") {
                    "is"
                } else {
                    "is_a"
                },
                place_required: false,
            }
        } else if matches!(
            word,
            "adalah" | "ialah" | "merupakan" | "is" | "are" | "was" | "were" | "bukan"
        ) && matches!(next, "di" | "in")
            && words.get(index + 2).is_some()
        {
            Pattern {
                start: index,
                end: index + 2,
                object_start: index + 2,
                predicate: "located_in",
                place_required: true,
            }
        } else if matches!(word, "is" | "are" | "was" | "were")
            && matches!(
                next,
                "part"
                    | "born"
                    | "composed"
                    | "colored"
                    | "coloured"
                    | "greater"
                    | "less"
                    | "larger"
                    | "smaller"
            )
        {
            continue;
        } else if matches!(
            word,
            "adalah" | "ialah" | "merupakan" | "is" | "are" | "was" | "were" | "bukan"
        ) {
            Pattern {
                start: index,
                end: index + 1,
                object_start: index + 1,
                predicate: if matches!(word, "is" | "are" | "was" | "were") {
                    "is"
                } else {
                    "is_a"
                },
                place_required: false,
            }
        } else {
            continue;
        };
        if pattern.object_start < words.len() {
            return Some(pattern);
        }
    }
    None
}

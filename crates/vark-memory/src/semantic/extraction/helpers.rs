pub(super) fn clean_word(raw: &str) -> String {
    raw.trim_matches(|character: char| {
        matches!(
            character,
            '.' | '!'
                | '?'
                | '…'
                | ','
                | ';'
                | ':'
                | '"'
                | '\''
                | '“'
                | '”'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
        )
    })
    .to_lowercase()
}

pub(super) fn looks_like_place(raw_words: &[&str]) -> bool {
    let words = raw_words
        .iter()
        .map(|word| clean_word(word))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.is_empty()
        || words.first().is_some_and(|word| {
            matches!(
                word.as_str(),
                "hari" | "bulan" | "minggu" | "tahun" | "pagi" | "siang" | "sore" | "malam"
            )
        })
    {
        return false;
    }
    let known_places = [
        "indonesia",
        "jakarta",
        "bandung",
        "surabaya",
        "yogyakarta",
        "jawa",
        "bali",
        "sumatra",
        "kalimantan",
        "sulawesi",
        "papua",
        "singapura",
        "malaysia",
        "thailand",
        "vietnam",
        "amerika",
        "inggris",
        "perancis",
        "prancis",
        "jepang",
        "china",
        "tiongkok",
        "australia",
        "paris",
        "london",
        "tokyo",
        "new york",
        "berlin",
        "rome",
        "moscow",
        "egypt",
        "afrika",
        "asia",
        "eropa",
        "pasifik",
        "samudra",
        "laut",
        "gunung",
        "sungai",
        "danau",
        "kota",
        "desa",
        "provinsi",
        "negara",
    ];
    if known_places
        .iter()
        .any(|place| words.join(" ").contains(place))
    {
        return true;
    }
    raw_words
        .iter()
        .map(|word| {
            word.trim_matches(|character: char| {
                matches!(character, ',' | '.' | '!' | '?' | '"' | '\'' | ')' | ']')
            })
        })
        .find(|word| !word.is_empty())
        .and_then(|word| word.chars().next())
        .is_some_and(char::is_uppercase)
}

pub(super) fn parse_number(token: &str) -> Option<f64> {
    let cleaned = token.trim_matches(|character: char| {
        matches!(
            character,
            ',' | '.' | ';' | ':' | '(' | ')' | '[' | ']' | '"' | '\''
        )
    });
    let normalized = if cleaned.matches('.').count() > 1
        && cleaned
            .split('.')
            .skip(1)
            .all(|group| group.chars().count() == 3)
    {
        cleaned.replace('.', "")
    } else if cleaned.contains(',')
        && cleaned
            .split(',')
            .nth(1)
            .is_some_and(|part| part.len() == 3)
    {
        cleaned.replace(',', "")
    } else {
        cleaned.replace(',', ".")
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

pub(super) fn measurement(words: &[String]) -> (Option<f64>, Option<String>) {
    const UNITS: &[&str] = &[
        "m",
        "meter",
        "meters",
        "metre",
        "metres",
        "km",
        "kilometer",
        "kilometers",
        "cm",
        "mm",
        "kg",
        "gram",
        "grams",
        "g",
        "ton",
        "tons",
        "%",
        "persen",
        "derajat",
        "celsius",
        "°c",
        "tahun",
        "years",
        "year",
        "hari",
        "day",
        "days",
    ];
    for (index, word) in words.iter().enumerate() {
        if let Some(value) = parse_number(word) {
            let unit = words
                .get(index + 1)
                .filter(|candidate| UNITS.contains(&candidate.as_str()))
                .cloned();
            return (Some(value), unit);
        }
    }
    (None, None)
}

pub(super) fn extract_qualifiers(words: &[String], predicate: &str) -> Vec<(String, String)> {
    let mut qualifiers = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let qualifier_type = match word.as_str() {
            "pada" | "saat" | "ketika" | "during" | "at" => Some("waktu"),
            "jika" | "apabila" | "if" => Some("kondisi"),
            "di" | "in" if !matches!(predicate, "located_in" | "born_in") => Some("tempat"),
            _ => None,
        };
        let Some(kind) = qualifier_type else {
            continue;
        };
        let value = words[index + 1..]
            .iter()
            .take_while(|candidate| !matches!(candidate.as_str(), "dan" | "and" | "tetapi" | "but"))
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        if !value.is_empty() && !qualifiers.iter().any(|(existing, _)| existing == kind) {
            qualifiers.push((kind.to_owned(), value));
        }
    }
    qualifiers
}

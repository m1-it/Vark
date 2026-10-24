mod helpers;
mod patterns;

use self::helpers::{clean_word, extract_qualifiers, looks_like_place, measurement, parse_number};
use self::patterns::find_pattern;
use super::{normalize_entity, Quantifier, Relation};

pub fn extract_relations(text: &str, source: &str) -> Vec<Relation> {
    let mut relations = Vec::new();
    if let Some(relation) = conditional_relation(text, source) {
        relations.push(relation);
    }
    let mut inherited_subject: Option<String> = None;
    for clause in split_clauses(text) {
        for relation in parse_clause(&clause, source, inherited_subject.as_deref()) {
            inherited_subject = Some(relation.subject.clone());
            relations.push(relation);
        }
    }
    relations
}

fn conditional_relation(text: &str, source: &str) -> Option<Relation> {
    let normalized = text.to_lowercase();
    let condition_start = if normalized.starts_with("jika ") {
        5
    } else if normalized.starts_with("if ") {
        3
    } else {
        return None;
    };
    let comma = text.get(condition_start..)?.find(',')?;
    let condition = text.get(condition_start..condition_start + comma)?.trim();
    let consequence = text.get(condition_start + comma + 1..)?.trim();
    let subject = normalize_entity(condition);
    let object = normalize_entity(consequence);
    if subject.is_empty() || object.is_empty() {
        return None;
    }
    Some(Relation {
        subject,
        predicate: "if_then".to_owned(),
        object,
        polarity: true,
        confidence: 70,
        source: source.to_owned(),
        value: None,
        unit: None,
        qualifiers: vec![("kondisi".to_owned(), normalize_entity(condition))],
        quantifier: None,
    })
}

fn split_clauses(text: &str) -> Vec<String> {
    let tokens = text.split_whitespace().collect::<Vec<_>>();
    let mut clauses = Vec::new();
    let mut current = Vec::new();
    for (index, raw) in tokens.iter().enumerate() {
        let word = clean_word(raw);
        let comma_ends_clause = raw.ends_with(',')
            && !raw
                .trim_matches(|character: char| matches!(character, ',' | ';' | ':' | '"' | '\''))
                .chars()
                .all(|character| character.is_numeric() || character == '.');
        let is_conjunction = matches!(
            word.as_str(),
            "dan"
                | "serta"
                | "and"
                | "while"
                | "whereas"
                | "sedangkan"
                | "tetapi"
                | "namun"
                | "but"
                | "however"
                | "karena"
                | "because"
                | "sehingga"
                | "therefore"
                | "yang"
        );
        let current_has_predicate = find_pattern(
            &current
                .iter()
                .map(|part: &&str| clean_word(part))
                .collect::<Vec<_>>(),
        )
        .is_some();
        let comma_clause_has_predicate = if comma_ends_clause {
            let mut candidate = current
                .iter()
                .map(|part: &&str| clean_word(part))
                .collect::<Vec<_>>();
            candidate.push(clean_word(raw));
            find_pattern(&candidate).is_some()
        } else {
            false
        };
        if is_conjunction && current.is_empty() {
            continue;
        }
        let next_has_predicate = next_segment_has_predicate(&tokens, index + 1);
        if comma_ends_clause && comma_clause_has_predicate && next_has_predicate {
            current.push(raw.trim_end_matches(','));
            let clause = current.join(" ");
            if !clause.trim().is_empty() {
                clauses.push(clause);
            }
            current.clear();
            continue;
        }
        if is_conjunction && current_has_predicate && next_has_predicate {
            let clause = current.join(" ");
            if !clause.trim().is_empty() {
                clauses.push(clause);
            }
            current.clear();
            continue;
        }
        current.push(*raw);
    }
    if !current.is_empty() {
        let clause = current.join(" ");
        if !clause.trim().is_empty() {
            clauses.push(clause);
        }
    }
    clauses
}

fn next_segment_has_predicate(tokens: &[&str], start: usize) -> bool {
    let mut segment = Vec::new();
    for raw in tokens.iter().skip(start) {
        let word = clean_word(raw);
        let comma = raw.ends_with(',')
            && !raw
                .trim_matches(|character: char| matches!(character, ',' | ';' | ':' | '"' | '\''))
                .chars()
                .all(|character| character.is_numeric() || character == '.');
        if comma {
            segment.push(word);
            break;
        }
        if matches!(
            word.as_str(),
            "dan"
                | "serta"
                | "and"
                | "while"
                | "whereas"
                | "sedangkan"
                | "tetapi"
                | "namun"
                | "but"
                | "however"
                | "karena"
                | "because"
                | "sehingga"
                | "therefore"
                | "yang"
        ) {
            break;
        }
        segment.push(word);
    }
    find_pattern(&segment).is_some()
}

fn parse_clause(clause: &str, source: &str, inherited_subject: Option<&str>) -> Vec<Relation> {
    let raw_words = clause.split_whitespace().collect::<Vec<_>>();
    let words = raw_words
        .iter()
        .map(|word| clean_word(word))
        .collect::<Vec<_>>();
    let Some(pattern) = find_pattern(&words) else {
        return Vec::new();
    };
    if pattern.start >= words.len() || pattern.object_start >= words.len() {
        return Vec::new();
    }

    let object_raw = raw_words[pattern.object_start..].join(" ");
    let object_words = words[pattern.object_start..].to_vec();
    if object_words.is_empty() {
        return Vec::new();
    }
    if pattern.place_required && !looks_like_place(&raw_words[pattern.object_start..]) {
        return Vec::new();
    }

    let mut prefix = words[..pattern.start].to_vec();
    let mut quantifier = None;
    let mut polarity = true;
    if prefix.starts_with(&["tidak".to_owned(), "semua".to_owned()])
        || prefix.starts_with(&["not".to_owned(), "all".to_owned()])
    {
        prefix.drain(..2);
        quantifier = Some(Quantifier::Some);
        polarity = false;
    } else if prefix.starts_with(&["tidak".to_owned(), "ada".to_owned()])
        || prefix.starts_with(&["not".to_owned(), "any".to_owned()])
        || prefix
            .first()
            .is_some_and(|word| matches!(word.as_str(), "no" | "none"))
    {
        if prefix
            .first()
            .is_some_and(|word| matches!(word.as_str(), "no" | "none"))
        {
            prefix.remove(0);
        } else {
            prefix.drain(..2);
        }
        quantifier = Some(Quantifier::None);
        polarity = false;
    } else if prefix.first().is_some_and(|word| {
        matches!(
            word.as_str(),
            "semua" | "seluruh" | "all" | "every" | "each"
        )
    }) {
        prefix.remove(0);
        quantifier = Some(Quantifier::All);
    }

    if prefix
        .last()
        .is_some_and(|word| matches!(word.as_str(), "is" | "are" | "was" | "were"))
    {
        prefix.pop();
    }

    for word in &mut prefix {
        if matches!(word.as_str(), "tidak" | "bukan" | "not" | "never" | "no") {
            polarity = false;
            word.clear();
        }
    }
    for word in words[pattern.start..pattern.object_start].iter() {
        if matches!(
            word.as_str(),
            "tidak" | "bukan" | "not" | "never" | "cannot" | "can't"
        ) {
            polarity = false;
        }
    }
    prefix.retain(|word| !word.is_empty());
    let subjects = if prefix.is_empty() {
        inherited_subject
            .map(|subject| vec![normalize_entity(subject)])
            .unwrap_or_default()
    } else {
        let mut subject_parts = Vec::new();
        let mut current = Vec::new();
        for word in prefix {
            if matches!(word.as_str(), "dan" | "and" | "serta") {
                if !current.is_empty() {
                    subject_parts.push(current.join(" "));
                    current.clear();
                }
            } else if !matches!(word.as_str(), "yang" | "who" | "which") {
                current.push(word);
            }
        }
        if !current.is_empty() {
            subject_parts.push(current.join(" "));
        }
        subject_parts
            .into_iter()
            .map(|subject| normalize_entity(&subject))
            .filter(|subject| !subject.is_empty())
            .collect::<Vec<_>>()
    };
    if subjects.is_empty() {
        return Vec::new();
    }

    let object = normalize_entity(&object_raw);
    if object.is_empty() {
        return Vec::new();
    }
    let measurement_words = if matches!(pattern.predicate, "greater_than" | "less_than") {
        &words[pattern.start + 1..pattern.end]
    } else {
        &[]
    };
    let value_source = if measurement_words
        .iter()
        .any(|word| parse_number(word).is_some())
    {
        measurement_words
    } else {
        &object_words
    };
    let (value, unit) = measurement(value_source);
    let qualifiers = extract_qualifiers(&words, pattern.predicate);
    subjects
        .into_iter()
        .map(|subject| Relation {
            subject,
            predicate: pattern.predicate.to_owned(),
            object: object.clone(),
            polarity,
            confidence: 70,
            source: source.to_owned(),
            value,
            unit: unit.clone(),
            qualifiers: qualifiers.clone(),
            quantifier,
        })
        .collect()
}

#[cfg(test)]
#[path = "extraction_tests.rs"]
mod tests;

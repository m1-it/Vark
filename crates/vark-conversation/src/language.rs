use crate::stopwords;
use vark_tokenizer::Tokenizer;

pub(crate) fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect()
}

pub(crate) fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut cursor = 0;

    while cursor < text.len() {
        let character = text[cursor..].chars().next().unwrap_or_default();
        let width = character.len_utf8();
        let is_blank_line = character == '\n'
            && text[cursor + width..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
            && text[cursor + width..]
                .chars()
                .take_while(|next| next.is_whitespace())
                .filter(|next| *next == '\n')
                .count()
                >= 1;
        let punctuation = matches!(character, '.' | '!' | '?' | '…');
        let should_split = if is_blank_line {
            true
        } else if punctuation {
            (character != '.' || !period_is_internal(text, cursor))
                && follows_sentence_boundary(text, cursor + width)
        } else {
            false
        };

        if should_split {
            let punctuation_end = if is_blank_line {
                cursor
            } else {
                cursor + width
            };
            let sentence_end = closing_punctuation_end(text, punctuation_end);
            let sentence = text[start..sentence_end].trim();
            if !sentence.is_empty() {
                sentences.push(sentence.to_owned());
            }
            start = skip_whitespace(text, sentence_end);
            cursor = start;
        } else {
            cursor += width;
        }
    }

    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        sentences.push(remainder.to_owned());
    }
    sentences
}

fn period_is_internal(text: &str, period: usize) -> bool {
    let previous = text[..period].chars().next_back();
    let following = text[period + 1..].chars().next();
    if previous.is_some_and(char::is_numeric) && following.is_some_and(char::is_numeric) {
        return true;
    }

    let token_start = text[..period]
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or(0, |(index, character)| index + character.len_utf8());
    let token_end = text[period + 1..]
        .char_indices()
        .find(|(_, character)| character.is_whitespace())
        .map_or(text.len(), |(index, _)| period + 1 + index);
    let token_prefix = text[token_start..=period].to_lowercase();
    let abbreviations = [
        "dr.", "prof.", "mis.", "dll.", "dsb.", "yg.", "a.k.a.", "e.g.", "i.e.", "mr.", "mrs.",
        "ms.", "etc.",
    ];
    if abbreviations
        .iter()
        .any(|abbreviation| abbreviation.starts_with(&token_prefix))
    {
        return true;
    }

    let token = &text[token_start..token_end];
    if (token.contains("@") || token.contains("://") || token.to_lowercase().starts_with("www."))
        && text[period + 1..token_end]
            .trim_matches(|character: char| matches!(character, '\'' | '"' | ')' | ']' | '}'))
            .chars()
            .any(|character| !character.is_whitespace())
    {
        return true;
    }

    let current_word = text[token_start..period]
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect::<String>();
    if current_word.chars().count() == 1 {
        let after_period = &text[period + 1..];
        let next_word = after_period
            .trim_start_matches(char::is_whitespace)
            .trim_start_matches(['\'', '"', '“', '‘'])
            .chars()
            .next();
        if next_word.is_some_and(char::is_uppercase) {
            return true;
        }
    }
    false
}

fn follows_sentence_boundary(text: &str, after_punctuation: usize) -> bool {
    let next = text[after_punctuation..].chars().find(|character| {
        !character.is_whitespace()
            && !matches!(
                character,
                '\'' | '"' | '“' | '‘' | '”' | '’' | '»' | '(' | '[' | '{' | ')' | ']' | '}'
            )
    });
    next.is_none_or(|character| character.is_uppercase() || character.is_numeric())
}

fn closing_punctuation_end(text: &str, mut end: usize) -> usize {
    while end < text.len() {
        let Some(character) = text[end..].chars().next() else {
            break;
        };
        if matches!(character, '\'' | '"' | '”' | '’' | '»' | ')' | ']' | '}') {
            end += character.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn skip_whitespace(text: &str, mut index: usize) -> usize {
    while index < text.len() {
        let Some(character) = text[index..].chars().next() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        index += character.len_utf8();
    }
    index
}

pub(crate) fn is_question(s: &str) -> bool {
    let n = normalize(s);
    n.contains('?')
        || [
            "apa ",
            "siapa ",
            "kapan ",
            "dimana ",
            "di mana ",
            "mengapa ",
            "kenapa ",
            "bagaimana ",
            "what ",
            "who ",
            "when ",
            "where ",
            "why ",
            "how ",
        ]
        .iter()
        .any(|p| n.starts_with(p))
}

pub(crate) fn content_tokens(_tokenizer: &Tokenizer, text: &str) -> Vec<String> {
    let characters = text.to_lowercase().chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut current = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        let previous_is_word = index > 0
            && (characters[index - 1].is_alphanumeric()
                || matches!(characters[index - 1], '_' | '-'));
        let next_is_word = characters
            .get(index + 1)
            .is_some_and(|next| next.is_alphanumeric());
        let keep = character.is_alphanumeric()
            || character == '_'
            || (character == '-' && previous_is_word && next_is_word)
            || (character == '#' && previous_is_word && !next_is_word);

        if keep {
            current.push(character);
        } else if !current.is_empty() {
            if !stopwords::contains(&current) {
                tokens.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if !current.is_empty() && !stopwords::contains(&current) {
        tokens.push(current);
    }
    tokens
}

pub(crate) fn key_for(tokens: &[String]) -> String {
    tokens.join(" ")
}

#[cfg(test)]
#[path = "language_tests.rs"]
mod tests;

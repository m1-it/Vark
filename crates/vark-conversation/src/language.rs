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
pub(crate) fn split_sentences(s: &str) -> Vec<String> {
    s.split(['.', '!', '?', '\n'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(String::from)
        .collect()
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
pub(crate) fn content_tokens(t: &Tokenizer, s: &str) -> Vec<String> {
    t.tokenize(s)
        .into_iter()
        .map(|x| x.text)
        .filter(|x| x.len() > 2)
        .collect()
}
pub(crate) fn key_for(t: &[String]) -> String {
    t.join(" ")
}

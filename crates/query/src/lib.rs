use lss_types::SearchQuery;

#[must_use]
pub fn parse_query(raw: impl Into<String>) -> SearchQuery {
    let raw = raw.into();
    let mut query = SearchQuery::new(raw.clone());

    let mut terms = Vec::new();
    let parts = raw.split_whitespace();

    for part in parts {
        if let Some(stripped) = part.strip_prefix("filetype:") {
            query.filetype_filters.push(stripped.to_string());
        } else if let Some(stripped) = part.strip_prefix("name:") {
            query.name_filters.push(stripped.to_string());
        } else {
            terms.push(part);
        }
    }

    let stop_words = [
        "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is",
        "it", "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there",
        "these", "they", "this", "to", "was", "will", "with", "where", "how", "what", "who",
        "why", "when", "can", "do", "does",
    ];

    let filtered_terms: Vec<&str> = terms
        .into_iter()
        .filter(|t| !stop_words.contains(&t.to_lowercase().as_str()))
        .collect();

    query.terms = filtered_terms.join(" ");
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_filters() {
        let q = parse_query("filetype:rs fn main name:test");
        assert_eq!(q.terms, "fn main");
        assert_eq!(q.filetype_filters, vec!["rs"]);
        assert_eq!(q.name_filters, vec!["test"]);
    }
}

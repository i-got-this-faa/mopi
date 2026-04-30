use mopi_types::SearchQuery;

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

    query.terms = terms.join(" ");
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

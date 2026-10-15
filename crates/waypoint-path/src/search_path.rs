// The path of a search: a query written as ordered key/value pairs, or a saved smart folder.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::encoding::{decode, encode_into};
use crate::PathError;

/// The URI scheme of a search location.
pub const SEARCH_SCHEME: &str = "search";

/// The path segment that introduces a smart folder's id: `search:/saved/{id}`.
const SAVED_SEGMENT: &str = "/saved/";

/// Bytes kept as they are inside a key or a value. `,`, `:` and `/` read well in
/// `kind=image,video` and in a scope written as a location, and none separates anything here.
const KEEP: &[u8] = b",:/";

/// A search location. The query's own model lives in a later crate (it needs this one), so the
/// query is held here as the ordered pairs it is written as; that crate maps its type to and from
/// them and owns defaults and validation. Order and unknown keys are kept as read, so a URI
/// written by a newer version survives a round trip through an older one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SearchPath {
    /// An ad hoc search: `search:?in=home&q=report`.
    Query(Vec<(String, String)>),
    /// A smart folder, by id: `search:/saved/{id}`.
    Saved(String),
}

impl SearchPath {
    /// Whether `uri` is written in the search scheme, whatever follows.
    pub fn is_search_uri(uri: &str) -> bool {
        uri.get(..SEARCH_SCHEME.len() + 1).is_some_and(|head| {
            head[..SEARCH_SCHEME.len()].eq_ignore_ascii_case(SEARCH_SCHEME) && head.ends_with(':')
        })
    }

    /// The canonical URI. An empty query is `search:?`.
    pub fn to_uri(&self) -> String {
        match self {
            SearchPath::Query(pairs) => {
                let mut out = format!("{SEARCH_SCHEME}:?");
                for (index, (key, value)) in pairs.iter().enumerate() {
                    if index > 0 {
                        out.push('&');
                    }
                    encode_into(&mut out, key.as_bytes(), KEEP);
                    out.push('=');
                    encode_into(&mut out, value.as_bytes(), KEEP);
                }
                out
            }
            SearchPath::Saved(id) => {
                let mut out = format!("{SEARCH_SCHEME}:{SAVED_SEGMENT}");
                encode_into(&mut out, id.as_bytes(), b"");
                out
            }
        }
    }

    /// Reads a search URI. `search:` and `search:?` are the empty query; anything that is neither
    /// a query nor `/saved/{id}` is refused, as is a key with no `=`, an empty key or id, or text
    /// that does not decode to UTF-8.
    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        if !Self::is_search_uri(uri) {
            let scheme = uri.split(':').next().unwrap_or_default().to_owned();
            return Err(PathError::UnsupportedScheme(scheme));
        }
        let rest = &uri[SEARCH_SCHEME.len() + 1..];
        if rest.contains('#') {
            return Err(PathError::InvalidUri("a search has no fragment"));
        }
        if let Some(id) = rest.strip_prefix(SAVED_SEGMENT) {
            // An id is one segment as written; an encoded `/` inside it is part of the id.
            if id.is_empty() || id.contains('/') {
                return Err(PathError::InvalidUri("a smart folder needs one id"));
            }
            return Ok(SearchPath::Saved(text(id)?));
        }
        let query = match rest.strip_prefix('?') {
            Some(query) => query,
            None if rest.is_empty() => "",
            None => return Err(PathError::InvalidUri("a search is `?query` or `/saved/id`")),
        };
        if query.is_empty() {
            return Ok(SearchPath::Query(Vec::new()));
        }
        query
            .split('&')
            .map(|pair| {
                let (key, value) = pair
                    .split_once('=')
                    .ok_or(PathError::InvalidUri("a search key has no `=`"))?;
                let key = text(key)?;
                if key.is_empty() {
                    return Err(PathError::InvalidUri("a search key is empty"));
                }
                Ok((key, text(value)?))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(SearchPath::Query)
    }

    /// The path for people.
    pub fn display(&self) -> String {
        "Search".to_owned()
    }

    /// The breadcrumb text. The query's own wording ("Search: {text} in {scope}", or a smart
    /// folder's name) is the page's to give; the path alone knows only that it is a search.
    pub fn label(&self) -> String {
        self.display()
    }

    /// The value of the first pair named `key`, when this is a query.
    pub fn get(&self, key: &str) -> Option<&str> {
        match self {
            SearchPath::Query(pairs) => pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.as_str()),
            SearchPath::Saved(_) => None,
        }
    }
}

fn text(encoded: &str) -> Result<String, PathError> {
    String::from_utf8(decode(encoded)?)
        .map_err(|_| PathError::InvalidUri("a search holds text that is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(items: &[(&str, &str)]) -> SearchPath {
        SearchPath::Query(
            items
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        )
    }

    #[test]
    fn a_query_round_trips_in_every_key_order() {
        let items = [
            ("v", "1"),
            ("in", "file:///home/a%20b"),
            ("q", "a&b=c d#é+%"),
            ("kind", "image,video"),
            ("modified", "2026-01-01..2026-06-30"),
        ];
        // Every rotation and its reversal, so no order is privileged.
        let mut orders = Vec::new();
        for shift in 0..items.len() {
            let mut rotated = items.to_vec();
            rotated.rotate_left(shift);
            orders.push(rotated.clone());
            rotated.reverse();
            orders.push(rotated);
        }
        for order in orders {
            let path = pairs(&order);
            let uri = path.to_uri();
            assert_eq!(SearchPath::from_uri(&uri).unwrap(), path, "{uri}");
            assert_eq!(SearchPath::from_uri(&uri).unwrap().to_uri(), uri);
        }
    }

    #[test]
    fn the_uri_is_written_in_the_order_given_and_reads_plainly() {
        let path = pairs(&[("in", "home"), ("q", "a b"), ("kind", "image,video")]);
        assert_eq!(path.to_uri(), "search:?in=home&q=a%20b&kind=image,video");
        assert_eq!(
            pairs(&[("in", "file:///a b")]).to_uri(),
            "search:?in=file:///a%20b"
        );
    }

    #[test]
    fn unknown_keys_duplicates_and_empty_values_are_kept() {
        let uri = "search:?zz=9&q=&q=x&v=2";
        let path = SearchPath::from_uri(uri).unwrap();
        assert_eq!(
            path,
            pairs(&[("zz", "9"), ("q", ""), ("q", "x"), ("v", "2")])
        );
        assert_eq!(path.to_uri(), uri);
        assert_eq!(path.get("q"), Some(""));
        assert_eq!(path.get("none"), None);
    }

    #[test]
    fn only_the_pairs_held_are_written() {
        // Defaults are the query type's to leave out; the path writes exactly what it holds.
        assert_eq!(pairs(&[("q", "x")]).to_uri(), "search:?q=x");
        assert_eq!(pairs(&[]).to_uri(), "search:?");
        assert_eq!(SearchPath::from_uri("search:").unwrap(), pairs(&[]));
        assert_eq!(SearchPath::from_uri("search:?").unwrap(), pairs(&[]));
    }

    #[test]
    fn a_smart_folder_round_trips() {
        let path = SearchPath::Saved("a b/é".to_owned());
        let uri = path.to_uri();
        assert_eq!(uri, "search:/saved/a%20b%2F%C3%A9");
        assert_eq!(SearchPath::from_uri(&uri).unwrap(), path);
        assert_eq!(
            SearchPath::from_uri("SEARCH:/saved/x").unwrap(),
            SearchPath::Saved("x".to_owned())
        );
    }

    #[test]
    fn malformed_uris_are_refused() {
        for uri in [
            "search:/saved/",
            "search:/saved/a/b",
            "search:/other",
            "search:x",
            "search:?q",
            "search:?=x",
            "search:?q=%zz",
            "search:?q=%",
            "search:?q=%FF",
            "search:?q=x&",
            "search:?q=x#frag",
        ] {
            assert!(SearchPath::from_uri(uri).is_err(), "{uri}");
        }
        assert!(matches!(
            SearchPath::from_uri("file:///a"),
            Err(PathError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn it_reads_as_a_search_uri_only_by_scheme() {
        assert!(SearchPath::is_search_uri("search:?q=x"));
        assert!(SearchPath::is_search_uri("Search:/saved/a"));
        assert!(!SearchPath::is_search_uri("searching:"));
        assert!(!SearchPath::is_search_uri("file:///search:"));
        assert!(!SearchPath::is_search_uri("sear"));
    }

    #[test]
    fn people_read_search() {
        assert_eq!(pairs(&[("q", "x")]).display(), "Search");
        assert_eq!(SearchPath::Saved("a".into()).label(), "Search");
    }
}

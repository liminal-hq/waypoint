// The small, bounded map of extra facts a provider can attach to an entry, for plugin columns to show.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

/// The most attributes one entry carries.
pub const MAX_ATTRIBUTES: usize = 8;
/// The longest key, in bytes.
pub const MAX_KEY_BYTES: usize = 32;
/// The longest value, in bytes (cut at a character boundary).
pub const MAX_VALUE_BYTES: usize = 128;

/// Facts about an entry that only some providers have (an S3 object's storage class and ETag, a
/// WebDAV entry's lock state), keyed by `provider.name` in lower camel case (`s3.storageClass`). A
/// listing carries them to the page beside the standard fields, where a column reads the key it
/// knows; nothing else interprets them. They are bounded (`MAX_ATTRIBUTES` keys of at most
/// `MAX_KEY_BYTES`, values of at most `MAX_VALUE_BYTES`) so a provider cannot make a 500 000-entry
/// folder expensive: an attribute over the limits is dropped, and a long value is cut.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryAttributes(BTreeMap<String, String>);

impl EntryAttributes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `key` with `value`, unless the key is empty, too long, or the entry already holds
    /// `MAX_ATTRIBUTES` others. A value over the limit is cut.
    pub fn with(mut self, key: &str, value: &str) -> Self {
        if key.is_empty() || key.len() > MAX_KEY_BYTES {
            return self;
        }
        if self.0.len() >= MAX_ATTRIBUTES && !self.0.contains_key(key) {
            return self;
        }
        let mut end = value.len().min(MAX_VALUE_BYTES);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.0.insert(key.to_owned(), value[..end].to_owned());
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The attributes as the wire carries them.
    pub fn into_map(self) -> BTreeMap<String, String> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_are_bounded() {
        let mut attributes = EntryAttributes::new();
        for n in 0..20 {
            attributes = attributes.with(&format!("k{n}"), "v");
        }
        assert_eq!(attributes.len(), MAX_ATTRIBUTES);
        // A key already there may change even when the map is full.
        let attributes = attributes.with("k0", "new");
        assert_eq!(attributes.get("k0"), Some("new"));
        assert_eq!(attributes.get("k19"), None);
        let attributes = EntryAttributes::new()
            .with("", "x")
            .with(&"k".repeat(MAX_KEY_BYTES + 1), "x");
        assert!(attributes.is_empty());
    }

    #[test]
    fn a_long_value_is_cut_at_a_character() {
        let value = "é".repeat(MAX_VALUE_BYTES);
        let attributes = EntryAttributes::new().with("a", &value);
        let kept = attributes.get("a").unwrap();
        assert!(kept.len() <= MAX_VALUE_BYTES);
        assert!(kept.chars().all(|c| c == 'é'));
    }
}

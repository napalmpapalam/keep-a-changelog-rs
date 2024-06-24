use std::fmt::Display;

use derive_getters::Getters;
use eyre::{eyre, OptionExt, Result};

/// Represents a link in a changelog.
#[derive(Debug, Clone, Getters, PartialEq, Eq)]
pub struct Link {
    pub anchor: String,
    pub url: String,
}

impl Link {
    /// Parse a link from a string.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use keep_a_changelog::Link;
    ///
    /// let link = Link::parse("[anchor]: https://example.com".to_string()).unwrap();
    /// assert_eq!(link.anchor(), "anchor");
    /// assert_eq!(link.url(), "https://example.com");
    /// ```
    pub fn parse(line: String) -> Result<Self> {
        let mut parts = line.splitn(2, ": ").map(|s| s.to_string());
        let anchor = parts
            .next()
            .ok_or_eyre(format!("Missing anchor: {line}"))?
            .replace(['[', ']'], "");
        let url = parts.next().ok_or_eyre("Missing url")?;

        Ok(Self { anchor, url })
    }

    pub fn new<S: Into<String>>(anchor: S, url: S) -> Result<Self> {
        let anchor = anchor.into();
        let anchor = anchor.replace(['[', ']', ':'], "");

        if anchor.is_empty() {
            return Err(eyre!("Missing anchor: {anchor}"));
        }

        let url = url.into();

        if url.is_empty() {
            return Err(eyre!("Missing url"));
        }

        Ok(Self { anchor, url })
    }
}

impl Display for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}]: {}", self.anchor, self.url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_invalid_anchor() {
        let result = Link::new("", "https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_new_with_invalid_url() {
        let result = Link::new("anchor", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_new_with_valid_anchor_and_url() {
        let result = Link::new("anchor", "https://example.com");
        assert!(result.is_ok());
        let link = result.unwrap();
        assert_eq!(link.anchor(), "anchor");
        assert_eq!(link.url(), "https://example.com");
    }

    #[test]
    fn test_new_with_valid_decorarted_anchor_and_url() {
        let result = Link::new("[anchor]:", "https://example.com");
        assert!(result.is_ok());
        let link = result.unwrap();
        assert_eq!(link.anchor(), "anchor");
        assert_eq!(link.url(), "https://example.com");
    }
}

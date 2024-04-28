use std::fmt::Display;

use derive_getters::Getters;
use eyre::{OptionExt, Result};

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
}

impl Display for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}]: {}", self.anchor, self.url)
    }
}

use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use eyre::{bail, Error};

use crate::utils::substring;

/// Represents a change kind.
///
/// This is used to categorize changes in a changelog.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ChangeKind {
    Added,
    Changed,
    Deprecated,
    Removed,
    Fixed,
    Security,
}

impl FromStr for ChangeKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "added" => Ok(Self::Added),
            "changed" => Ok(Self::Changed),
            "deprecated" => Ok(Self::Deprecated),
            "removed" => Ok(Self::Removed),
            "fixed" => Ok(Self::Fixed),
            "security" => Ok(Self::Security),
            _ => bail!("Unknown change type: {}", s),
        }
    }
}

/// Represents a set of changes.
///
/// This is used to represent a set of changes in a changelog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    added: Vec<String>,
    changed: Vec<String>,
    deprecated: Vec<String>,
    removed: Vec<String>,
    fixed: Vec<String>,
    security: Vec<String>,
    compact: bool,
}

impl Changes {
    /// Add a change to the set based on its kind.
    ///
    /// # Parameters
    /// - `kind`: The kind of change.
    /// - `change`: The change to add.
    ///
    /// # Examples
    ///
    /// ```
    /// use keep_a_changelog::{Changes, ChangeKind};
    ///
    /// let mut changes = Changes::default();
    /// changes.add(ChangeKind::Added, "Added a new feature".to_string());
    /// ```
    pub fn add(&mut self, kind: ChangeKind, change: String) {
        match kind {
            ChangeKind::Added => self.added.push(change),
            ChangeKind::Changed => self.changed.push(change),
            ChangeKind::Deprecated => self.deprecated.push(change),
            ChangeKind::Removed => self.removed.push(change),
            ChangeKind::Fixed => self.fixed.push(change),
            ChangeKind::Security => self.security.push(change),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.changed.is_empty()
            && self.deprecated.is_empty()
            && self.removed.is_empty()
            && self.fixed.is_empty()
            && self.security.is_empty()
    }

    pub(crate) fn set_compact(&mut self, value: bool) -> &mut Self {
        self.compact = value;
        self
    }
}

impl Display for Changes {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut first_printed = false;

        if !self.added.is_empty() {
            ensure_newline(f, &mut first_printed)?;
            writeln!(f, "### Added")?;
            if !self.compact {
                writeln!(f)?;
            }
            print_changes(f, &self.added)?;
            writeln!(f)?;
        }

        if !self.changed.is_empty() {
            ensure_newline(f, &mut first_printed)?;
            writeln!(f, "### Changed")?;
            if !self.compact {
                writeln!(f)?;
            }
            print_changes(f, &self.changed)?;
            writeln!(f)?;
        }

        if !self.deprecated.is_empty() {
            ensure_newline(f, &mut first_printed)?;
            writeln!(f, "### Deprecated")?;
            if !self.compact {
                writeln!(f)?;
            }
            print_changes(f, &self.deprecated)?;
            writeln!(f)?;
        }

        if !self.removed.is_empty() {
            ensure_newline(f, &mut first_printed)?;
            writeln!(f, "### Removed")?;
            if !self.compact {
                writeln!(f)?;
            }
            print_changes(f, &self.removed)?;
            writeln!(f)?;
        }

        if !self.fixed.is_empty() {
            ensure_newline(f, &mut first_printed)?;
            writeln!(f, "### Fixed")?;
            if !self.compact {
                writeln!(f)?;
            }
            print_changes(f, &self.fixed)?;
            writeln!(f)?;
        }

        if !self.security.is_empty() {
            ensure_newline(f, &mut first_printed)?;
            writeln!(f, "### Security")?;
            if !self.compact {
                writeln!(f)?;
            }
            print_changes(f, &self.security)?;
            writeln!(f)?;
        }

        Ok(())
    }
}

fn ensure_newline(f: &mut Formatter, first_printed: &mut bool) -> fmt::Result {
    if *first_printed {
        writeln!(f)?;
    } else {
        *first_printed = true;
    }

    Ok(())
}

fn print_changes(f: &mut Formatter, changes: &[String]) -> fmt::Result {
    changes.iter().try_for_each(|change| {
        let mut title = change
            .split('\n')
            .map(|line| format!("  {line}").trim_end().to_string())
            .collect::<Vec<String>>();
        title[0] = format!("- {}", substring(title[0].clone(), 1));
        writeln!(f, "{}", title.join("\n"))
    })
}

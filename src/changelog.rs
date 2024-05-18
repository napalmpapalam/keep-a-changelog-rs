use std::{
    fmt::{self, Display},
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use derive_builder::Builder;
use derive_getters::Getters;
use eyre::{Context, OptionExt, Result};
use regex::Regex;
use semver::Version;

use crate::{
    consts::{CHANGELOG_DESCRIPTION, CHANGELOG_TITLE},
    link::Link,
    parser::Parser,
    release::Release,
    utils::{get_compare_url, get_release_url},
};

#[derive(Debug, Clone, Builder, Getters)]
pub struct Changelog {
    #[builder(setter(into), default)]
    flag: Option<String>,
    /// Changelog title, default is "Changelog"
    #[builder(setter(into))]
    title: Option<String>,
    #[builder(setter(into))]
    /// Changelog description, default is "
    /// All notable changes to this project will be documented in this file.
    /// The format is based on [Keep a Changelog](https://keepachangelog.com/)
    /// and this project adheres to [Semantic Versioning](https://semver.org/)."
    description: Option<String>,
    /// Git HEAD reference, default is "HEAD", used for compare links, could be a branch name or a tag
    #[builder(default = "self.default_head()")]
    head: String,
    /// Footer text
    #[builder(setter(into), default)]
    footer: Option<String>,
    /// Repository URL, used for generating release and compare links, required for compare links,
    /// could be extracted from the CHANGELOG.md file if the links are present
    #[builder(setter(into), default)]
    url: Option<String>,
    /// Releases
    #[builder(setter(custom), public, default)]
    releases: Vec<Release>,
    /// All links which present in the CHANGELOG.md file
    #[builder(setter(custom), public, default)]
    links: Vec<Link>,
    /// Tag prefix, used for generating release and compare links, by default it's empty, could be
    /// used to add a prefix to the version number, for example, "v"
    #[builder(setter(into), default)]
    tag_prefix: Option<String>,
}

impl ChangelogBuilder {
    fn default_head(&self) -> String {
        "HEAD".into()
    }

    pub fn releases(&mut self, releases: Vec<Release>) -> &mut Self {
        self.releases = Some(releases);
        self.sort_releases()
    }

    fn sort_releases(&mut self) -> &mut Self {
        let mut releases = self.releases.clone().unwrap_or_default();

        let unreleased: Option<Release> = releases
            .iter()
            .position(|r| r.version().is_none() && r.date().is_none())
            .map(|idx| releases.remove(idx));

        releases.sort_by(|a, b| b.cmp(a));

        if let Some(unreleased) = unreleased {
            releases.insert(0, unreleased);
        }

        self.releases = Some(releases);
        self
    }

    pub fn links(&mut self, links: Vec<String>) -> Result<&mut Self> {
        let links = links
            .iter()
            .map(|link| Link::parse(link.clone()))
            .collect::<Result<Vec<Link>>>()
            .wrap_err_with(|| "Failed to parse links")?;
        self.links = Some(links);
        Ok(self)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChangelogParseOptions {
    pub url: Option<String>,
    pub tag_prefix: Option<String>,
    pub head: Option<String>,
}

impl Changelog {
    /// Parse CHANGELOG.md file
    ///
    /// # Examples
    ///
    /// ```
    /// use keep_a_changelog::{ChangelogParseOptions, Changelog};
    ///
    /// let markdown = "# Changelog\n## 0.1.0 - 2024-04-28\n- Initial release\n";
    ///
    /// let changelog = Changelog::parse(
    ///    markdown.to_string(),
    ///    Some(ChangelogParseOptions {
    ///        url: Some("https://github.com/napalmpapalam/keep-a-changelog-rs".to_string()),
    ///        head: Some("master".to_string()),
    ///        tag_prefix: Some("v".to_string()),
    ///    }),
    /// );
    ///
    /// assert!(changelog.is_ok());
    /// ```
    ///
    pub fn parse(markdown: String, opts: Option<ChangelogParseOptions>) -> Result<Self> {
        Parser::parse(markdown, opts)
    }

    pub fn parse_from_file(path: &str, opts: Option<ChangelogParseOptions>) -> Result<Self> {
        let path = Path::new(path);
        let mut markdown = String::new();
        File::open(path)?
            .read_to_string(&mut markdown)
            .wrap_err_with(|| "Failed to read CHANGELOG.md")?;
        Parser::parse(markdown, opts)
    }

    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.write_all(self.to_string().as_bytes())?;
        file.flush()?;
        Ok(())
    }

    pub fn releases_mut(&mut self) -> &mut Vec<Release> {
        &mut self.releases
    }

    /// Find release by version
    pub fn find_release(&self, version: String) -> Result<Option<&Release>> {
        let version = Version::parse(&version).wrap_err_with(|| {
            format!("Failed to parse version: {version} during finding release")
        })?;

        Ok(self
            .releases()
            .iter()
            .find(|r| r.version() == &Some(version.clone())))
    }

    /// Find release by version and return mutable reference
    pub fn find_release_mut(&mut self, version: String) -> Result<Option<&mut Release>> {
        let version = Version::parse(&version).wrap_err_with(|| {
            format!("Failed to parse version: {version} during finding release")
        })?;

        Ok(self
            .releases_mut()
            .iter_mut()
            .find(|r| r.version() == &Some(version.clone())))
    }

    /// Get unreleased release from changelog
    /// If there is no unreleased release, it will return None
    pub fn get_unreleased(&self) -> Option<&Release> {
        self.releases()
            .iter()
            .find(|r| r.version().is_none() && r.date().is_none())
    }

    /// Same as get_unreleased but mutable
    pub fn get_unreleased_mut(&mut self) -> Option<&mut Release> {
        self.releases_mut()
            .iter_mut()
            .find(|r| r.version().is_none() && r.date().is_none())
    }

    /// Add release to changelog
    /// It will add release to the beginning of the releases list and sort them by date
    ///
    /// # Examples
    ///
    /// ```
    /// use keep_a_changelog::{Changelog, ChangelogParseOptions, Release, NaiveDate, Version};
    ///
    /// let markdown = "# Changelog\n## 0.1.0 - 2024-04-28\n- Initial release\n";
    ///
    /// let mut changelog = Changelog::parse(
    ///    markdown.to_string(),
    ///    Some(ChangelogParseOptions {
    ///        url: Some("https://github.com/napalmpapalam/keep-a-changelog-rs".to_string()),
    ///        head: Some("master".to_string()),
    ///        tag_prefix: Some("v".to_string()),
    ///    }),
    /// ).unwrap();
    ///
    /// let release = Release::builder()
    ///    .version(Version::parse("0.1.1").unwrap())
    ///    .date(NaiveDate::from_ymd_opt(2024, 4, 30).unwrap())
    ///    .build().unwrap();
    ///
    /// changelog.add_release(release);
    /// assert_eq!(changelog.releases().len(), 2);
    /// ```
    ///
    pub fn add_release(&mut self, release: Release) -> &mut Self {
        self.releases.insert(0, release);
        self.sort_releases()
    }

    fn sort_releases(&mut self) -> &mut Self {
        let unreleased: Option<Release> = self
            .releases
            .iter()
            .position(|r| r.version().is_none() && r.date().is_none())
            .map(|idx| self.releases.remove(idx));

        self.releases.sort_by(|a, b| b.cmp(a));

        if let Some(unreleased) = unreleased {
            self.releases.insert(0, unreleased);
        }

        self
    }

    pub(crate) fn compare_link(
        &self,
        current: &Release,
        previous: Option<&Release>,
    ) -> Result<Option<Link>> {
        let repo_url = self.url().clone().ok_or_eyre("Missing repo URL")?;

        if previous.is_none() {
            let version = current
                .version()
                .clone()
                .ok_or_eyre("Missing version for current release")?
                .to_string();
            return Ok(Some(Link {
                anchor: version.clone(),
                url: get_release_url(repo_url, self.tag_name(version)),
            }));
        }

        let previous = previous.unwrap();

        if current.date().is_none() || current.version().is_none() {
            let version = previous
                .version()
                .clone()
                .ok_or_eyre("Missing version for previous release")?
                .to_string();
            return Ok(Some(Link {
                anchor: "Unreleased".into(),
                url: get_compare_url(repo_url, self.tag_name(version), self.head().clone()),
            }));
        }

        let current_version = current
            .version()
            .clone()
            .ok_or_eyre("Missing version for current release")?
            .to_string();
        let previous_version = previous
            .version()
            .clone()
            .ok_or_eyre("Missing version for previous release")?
            .to_string();

        Ok(Some(Link {
            anchor: current_version.clone(),
            url: get_compare_url(
                repo_url,
                self.tag_name(previous_version),
                self.tag_name(current_version),
            ),
        }))
    }

    fn tag_name(&self, version: String) -> String {
        if let Some(tag_prefix) = self.tag_prefix() {
            return format!("{}{}", tag_prefix, version);
        }
        version.to_string()
    }
}

impl Display for Changelog {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(flag) = self.flag.clone() {
            writeln!(f, "<!-- ${flag} -->")?;
        }

        let title = self.title.clone().unwrap_or_else(|| CHANGELOG_TITLE.into());
        writeln!(f, "# {title}",)?;

        let description = match self.description.clone() {
            Some(description) => description.trim().to_owned(),
            None => CHANGELOG_DESCRIPTION.into(),
        };

        writeln!(f, "{description}")?;

        self.releases()
            .iter()
            .try_for_each(|release| write!(f, "\n{release}"))?;

        writeln!(f)?;

        let tag_regex = Regex::new(r"\d+\.\d+\.\d+((-rc|-x)\.\d+)?").unwrap();

        let mut is_non_compare_links = false;

        self.links
            .iter()
            .filter(|link| {
                !tag_regex.is_match(link.anchor()) && !link.anchor().contains("Unreleased")
            })
            .try_for_each(|link| {
                if !is_non_compare_links {
                    is_non_compare_links = true;
                }

                write!(f, "\n{link}")
            })?;

        if is_non_compare_links {
            writeln!(f)?;
        }

        self.releases
            .iter()
            .filter_map(|release| {
                release
                    .compare_link(self)
                    .expect("Failed to get compare link")
            })
            .try_for_each(|link| write!(f, "\n{link}"))?;

        if let Some(footer) = self.footer.clone() {
            write!(f, "---\n{footer}\n")?;
        }

        writeln!(f)?;

        Ok(())
    }
}

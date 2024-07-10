use std::{
    collections::HashSet,
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
#[builder(derive(Debug))]
pub struct Changelog {
    #[builder(setter(into), default)]
    lint: Option<HashSet<String>>,
    #[builder(setter(into), default)]
    flag: Option<String>,
    /// Changelog title, default is "Changelog"
    #[builder(setter(into), default)]
    title: Option<String>,
    /// Changelog description, default is "
    /// All notable changes to this project will be documented in this file.
    /// The format is based on [Keep a Changelog](https://keepachangelog.com/)
    /// and this project adheres to [Semantic Versioning](https://semver.org/)."
    #[builder(setter(into), default)]
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
    /// Allow compact output, default is false.
    ///
    /// Compact output removes blank lines after headers and lists and inserts a flag to disable
    /// checking for these lines by markdownlint.
    #[builder(setter(custom), default = "false")]
    compact: bool,
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

    pub fn compact(&mut self, compact: bool) -> &mut Self {
        self.compact = Some(compact);
        if compact {
            let mut set = HashSet::new();
            set.insert("MD022".into());
            set.insert("MD032".into());
            self.lint(set);
        } else {
            self.lint = None;
        }
        self
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
        file.write_all(self.file_contents().as_bytes())?;
        file.flush()?;
        Ok(())
    }

    /// Format the changelog as a string for output as a valid Markdown file
    ///
    /// To ensure compliance with the requirements of the Markdown standard any blank
    /// line at the end of the string needs to be removed.
    ///
    fn file_contents(&self) -> String {
        let contents = self.to_string();
        let mut contents = contents.replace("\n\n\n", "\n\n");
        contents = contents.trim_end_matches('\n').to_string();
        contents.push('\n');
        contents
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

    /// Set compact option on.
    pub fn set_compact(&mut self) -> &mut Self {
        self.compact = true;
        self.disable_lint("MD022");
        self.disable_lint("MD032");

        self
    }

    /// Set compact option off.
    pub fn unset_compact(&mut self) -> &mut Self {
        self.compact = false;
        self.enable_lint("MD022");
        self.enable_lint("MD032");
        self
    }

    /// Add a lint to the list of markdown lints that will be ignored.
    ///
    pub fn disable_lint(&mut self, lint: &str) -> &mut Self {
        let set = match self.lint.clone() {
            Some(mut disabled) => {
                disabled.insert(lint.to_string());
                disabled
            }
            None => {
                let mut set = HashSet::new();
                set.insert(lint.to_string());
                set
            }
        };

        self.lint = Some(set);

        self
    }

    /// Remove a lint from the list of markdown lints that will be ignored.
    ///
    pub fn enable_lint(&mut self, lint: &str) -> &mut Self {
        if let Some(mut disabled) = self.lint.clone() {
            disabled.remove(lint);

            if disabled.is_empty() {
                self.lint = None;
            } else {
                self.lint = Some(disabled);
            }
        };

        self
    }

    /// Add a link to the list of links
    ///
    /// # Examples
    /// ```
    /// # fn main() {
    /// let mut changelog = ChangelogBuilder::default().build().unwrap();
    ///
    /// changelog.add_link("[anchor]: https://example.com".to_string());
    ///
    /// // Assert that the link was added correctly
    /// assert_eq!(changelog.links().len(), 1);
    /// assert_eq!(changelog.links().first().unwrap().anchor(), "anchor");
    /// assert_eq!(changelog.links().first().unwrap().url(),"https://example.com");
    /// # }    
    ///     
    pub fn add_link<S: Into<String>>(&mut self, anchor: S, url: S) -> &mut Self {
        let link = Link::new(anchor, url);

        if let Ok(link) = link {
            self.links.push(link);
        };
        self
    }
}

impl Display for Changelog {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(md_lints) = self.lint.clone() {
            let mut lints = md_lints.iter().cloned().collect::<Vec<_>>();
            lints.sort();
            let joined = lints.join(" ");
            writeln!(f, "<!-- markdownlint-disable {joined} -->",)?;
        }

        if let Some(flag) = self.flag.clone() {
            writeln!(f, "<!-- {flag} -->")?;
        }

        let title = self.title.clone().unwrap_or_else(|| CHANGELOG_TITLE.into());
        writeln!(f, "# {title}",)?;
        if !self.compact {
            writeln!(f)?;
        }

        let description = match self.description.clone() {
            Some(description) => description.trim().to_owned(),
            None => CHANGELOG_DESCRIPTION.into(),
        };

        writeln!(f, "{description}\n")?;

        self.releases().iter().try_for_each(|release| {
            let mut release = release.clone(); // clone the release so that we mutate if required
            release.set_compact(self.compact);
            write!(f, "{release}")
        })?;

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
            .try_for_each(|link| writeln!(f, "{link}"))?;

        if let Some(footer) = self.footer.clone() {
            write!(f, "---\n{footer}\n")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use chrono::NaiveDate;
    use log::LevelFilter;
    use log4rs_test_utils::test_logging;
    use rstest::rstest;
    use uuid::Uuid;

    use super::*;

    fn are_the_same(file_a: &str, file_b: &str) -> Result<bool> {
        let file_a_contents = fs::read_to_string(file_a)?;
        let file_b_contents = fs::read_to_string(file_b)?;

        if file_a_contents.len() != file_b_contents.len() {
            return Ok(false);
        }

        let a_lines: Vec<_> = file_a_contents.lines().collect();
        let b_lines: Vec<_> = file_b_contents.lines().collect();

        if a_lines.len() != b_lines.len() {
            return Ok(false);
        }

        for (a, b) in a_lines.iter().zip(b_lines.iter()) {
            if a != b {
                return Ok(false);
            }
        }

        Ok(true)
    }

    #[rstest]
    fn create_default_changelog(#[values(false, true)] compact: bool) -> Result<()> {
        test_logging::init_logging_once_for(vec![], LevelFilter::Debug, None);

        let mut expected_file = "tests/data/default_changelog.md";
        let mut file_name = "tests/tmp/test_default.md";

        let mut changelog = ChangelogBuilder::default().build()?;

        if compact {
            log::debug!("Changelog: {:#?}", changelog);
            expected_file = "tests/data/default_changelog_compact.md";
            file_name = "tests/tmp/test_default_compact.md";
            changelog.set_compact();
        }

        log::debug!("Changelog: {:#?}", changelog);

        changelog.save_to_file(file_name)?;

        assert!(are_the_same(expected_file, file_name)?);

        Ok(())
    }

    #[rstest]
    fn create_default_changelog_with_unreleased(
        #[values(false, true)] compact: bool,
    ) -> Result<()> {
        test_logging::init_logging_once_for(vec![], LevelFilter::Debug, None);

        let mut expected_file = "tests/data/default_changelog_with_unreleased.md";
        let mut file_name = "tests/tmp/test_default_with_unreleased.md";

        let mut changelog = if compact {
            expected_file = "tests/data/default_changelog_with_unreleased_compact.md";
            file_name = "tests/tmp/test_default_with_unreleased_compact.md";
            ChangelogBuilder::default().compact(true).build()?
        } else {
            ChangelogBuilder::default().build()?
        };

        log::debug!("Changelog: {:#?}", changelog);

        let release = Release::builder().build()?;

        changelog.add_release(release);

        log::debug!("Changelog: {:#?}", changelog);

        changelog.save_to_file(file_name)?;

        assert!(are_the_same(expected_file, file_name)?);

        Ok(())
    }

    #[rstest]
    fn create_initial_changelog_unreleased(#[values(false, true)] compact: bool) -> Result<()> {
        test_logging::init_logging_once_for(vec![], LevelFilter::Debug, None);

        let mut expected_filename = "tests/data/initial_changelog_unreleased.md";
        let mut file_name = "tests/tmp/test_initial_unreleased.md";

        let mut changelog = ChangelogBuilder::default().build()?;

        if compact {
            log::debug!("Changelog: {:#?}\nSetting compact output", changelog);
            expected_filename = "tests/data/initial_changelog_unreleased_compact.md";
            file_name = "tests/tmp/test_initial_unreleased_compact.md";
            changelog.set_compact();
        }

        let mut release = Release::builder().build()?;

        release.added("Initial commit".to_string());

        changelog.add_release(release);

        log::debug!("Changelog: {:#?}", changelog);

        changelog.save_to_file(file_name)?;

        assert!(are_the_same(expected_filename, file_name)?);

        Ok(())
    }

    #[rstest]
    fn create_early_changelog(#[values(false, true)] compact: bool) -> Result<()> {
        test_logging::init_logging_once_for(vec![], LevelFilter::Debug, None);

        let mut expected_filename = "tests/data/early_changelog.md";
        let mut file_name = "tests/tmp/test_early.md";

        let mut changelog = ChangelogBuilder::default()
            .flag("test flag".to_string())
            .url(Some(
                "https://github.com/napalmpapalam/keep-a-changelog-rs".to_string(),
            ))
            .build()?;

        if compact {
            log::debug!("Changelog: {:#?}\nSetting compact output", changelog);
            expected_filename = "tests/data/early_changelog_compact.md";
            file_name = "tests/tmp/test_early_compact.md";
            changelog.set_compact();
        }

        // First Release

        let mut release = Release::builder()
            .version(Version::parse("0.1.0")?)
            .date(NaiveDate::from_ymd_opt(2024, 4, 28).unwrap())
            .build()?;

        release.added("Initial release".to_string());

        changelog.add_release(release);

        // Second Release

        let mut release = Release::builder()
            .version(Version::parse("0.1.1")?)
            .date(NaiveDate::from_ymd_opt(2024, 5, 18).unwrap())
            .build()?;

        release.fixed("Parsing anchor links in the middle of the file".to_string());
        release.fixed("Error readability".to_string());

        changelog.add_release(release);

        // Third Release

        let mut release = Release::builder()
            .version(Version::parse("0.1.2")?)
            .date(NaiveDate::from_ymd_opt(2024, 5, 20).unwrap())
            .build()?;

        release.fixed("Default changelog description".to_string());
        release.fixed(
            "Changelog builder error when title and description are not provided".to_string(),
        );

        changelog.add_release(release);

        // Unreleased

        let release = Release::builder().build()?;

        changelog.add_release(release);

        // Ready to save

        log::debug!("Changelog: {:#?}", changelog);

        changelog.save_to_file(file_name)?;

        assert!(are_the_same(expected_filename, file_name)?);

        Ok(())
    }

    #[rstest]
    fn create_early_changelog_with_multiple_sections(
        #[values(false, true)] compact: bool,
    ) -> Result<()> {
        test_logging::init_logging_once_for(vec![], LevelFilter::Debug, None);

        let mut expected_filename = "tests/data/early_changelog_multiple_sections.md";
        let mut file_name = "tests/tmp/test_early_changelog_multiple_sections.md";

        let mut changelog = ChangelogBuilder::default()
            .flag("test flag".to_string())
            .url(Some(
                "https://github.com/napalmpapalam/keep-a-changelog-rs".to_string(),
            ))
            .build()?;

        if compact {
            log::debug!("Changelog: {:#?}\nSetting compact output", changelog);
            expected_filename = "tests/data/early_changelog_multiple_sections_compact.md";
            file_name = "tests/tmp/test_early_changelog_multiple_sections_compact.md";
            changelog.set_compact();
        }

        // First Release

        let mut release = Release::builder()
            .version(Version::parse("0.1.0")?)
            .date(NaiveDate::from_ymd_opt(2024, 4, 28).unwrap())
            .build()?;

        release.added("Initial release".to_string());

        changelog.add_release(release);

        // Second Release

        let mut release = Release::builder()
            .version(Version::parse("0.1.1")?)
            .date(NaiveDate::from_ymd_opt(2024, 5, 18).unwrap())
            .build()?;

        release.fixed("Parsing anchor links in the middle of the file".to_string());
        release.fixed("Error readability".to_string());

        changelog.add_release(release);

        // Third Release

        let mut release = Release::builder()
            .version(Version::parse("0.1.2")?)
            .date(NaiveDate::from_ymd_opt(2024, 5, 20).unwrap())
            .build()?;

        release.fixed("Default changelog description".to_string());
        release.fixed(
            "Changelog builder error when title and description are not provided".to_string(),
        );

        changelog.add_release(release);

        // Unreleased

        let mut release = Release::builder().build()?;

        release.added("New feature".to_string());
        release.added("Another new feature".to_string());
        release.fixed("Bug fix to one old feature".to_string());
        release.fixed("Bug fix to another old feature".to_string());

        changelog.add_release(release);

        // Ready to save

        log::debug!("Changelog: {:#?}", changelog);

        changelog.save_to_file(file_name)?;

        assert!(are_the_same(expected_filename, file_name)?);

        Ok(())
    }

    #[rstest]
    #[case("tests/data/default_changelog.md")]
    #[case("tests/data/default_changelog_with_unreleased.md")]
    #[case("tests/data/initial_changelog_unreleased.md")]
    #[case("tests/data/early_changelog.md")]
    #[case("tests/data/early_changelog_multiple_sections.md")]
    #[case("tests/data/default_changelog_compact.md")]
    #[case("tests/data/default_changelog_with_unreleased_compact.md")]
    #[case("tests/data/initial_changelog_unreleased_compact.md")]
    #[case("tests/data/early_changelog_compact.md")]
    #[case("tests/data/early_changelog_multiple_sections_compact.md")]
    fn test_save_to_file(#[case] test_input_file: &str) -> Result<()> {
        test_logging::init_logging_once_for(vec![], LevelFilter::Debug, None);

        let temp_dir_string = format!("tests/tmp/test-{}", Uuid::new_v4());
        let temp_dir = Path::new(&temp_dir_string);

        fs::create_dir_all(temp_dir).expect("failed to create temporary directory");

        let test_output_file = format!("{}/CHANGELOG.md", temp_dir_string);

        log::debug!("Temporary directory: {:?}", temp_dir);

        let changelog = Changelog::parse_from_file(test_input_file, None)?;

        changelog.save_to_file(&test_output_file)?;
        let mut file = File::open(&test_output_file)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        assert!(are_the_same(test_input_file, &test_output_file)?);

        Ok(())
    }

    #[test]
    fn test_add_link() {
        // Create a new ChangelogBuilder instance
        let builder = ChangelogBuilder::default();
        let mut changelog = builder.build().unwrap();

        // Add a link to the builder
        changelog.add_link("[anchor]:", "https://example.com");

        // Assert that the link was added correctly
        assert_eq!(changelog.links().len(), 1);
        assert_eq!(changelog.links().first().unwrap().anchor(), "anchor");
        assert_eq!(
            changelog.links().first().unwrap().url(),
            "https://example.com"
        );
    }
}

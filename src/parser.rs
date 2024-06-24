use std::collections::HashSet;

use eyre::{bail, eyre, Result};
use regex::Regex;
use semver::Version;

use crate::{
    changelog::ChangelogBuilder,
    release::{Release, ReleaseBuilder},
    token::{tokenize, Token, TokenKind},
    Changelog, ChangelogParseOptions,
};

#[derive(Debug)]
pub struct Parser {
    builder: ChangelogBuilder,
    tokens: Vec<Token>,
    opts: ChangelogParseOptions,
    idx: usize,
}

impl Parser {
    pub fn parse(markdown: String, opts: Option<ChangelogParseOptions>) -> Result<Changelog> {
        let (compact, tokens) = tokenize(markdown)?;
        let (links, tokens): (Vec<Token>, Vec<Token>) =
            tokens.into_iter().partition(|t| t.kind == TokenKind::Link);
        let builder = ChangelogBuilder::default();
        let opts = opts.unwrap_or_default();

        let mut parse_output = Self {
            builder,
            tokens,
            opts,
            idx: 0,
        };
        parse_output
            .parse_opts()?
            .parse_meta()?
            .parse_releases()?
            .parse_links(links)?
            .parse_footer()?
            .parse_compact(compact);
        log::trace!("Parse output: {:#?}", parse_output);
        parse_output.build()
    }

    fn parse_opts(&mut self) -> Result<&mut Self> {
        self.builder
            .url(self.opts.url.clone())
            .tag_prefix(self.opts.tag_prefix.clone());

        if let Some(head) = self.opts.head.clone() {
            self.builder.head(head);
        }

        Ok(self)
    }

    fn parse_meta(&mut self) -> Result<&mut Self> {
        let (lint, _) = self.get_lint_content()?;
        let (flag, _) = self.get_content(vec![TokenKind::Flag])?;
        let (title, _) = self.get_content(vec![TokenKind::H1])?;
        let description = self.get_text_content()?;

        self.builder
            .lint(lint)
            .flag(flag)
            .title(title)
            .description(description);

        Ok(self)
    }

    fn parse_releases(&mut self) -> Result<&mut Self> {
        let mut releases: Vec<Release> = vec![];
        let unreleased_regex = Regex::new(r"\[?([^\]]+)\]?\s*-\s*unreleased(\s+\[yanked\])?$")?;
        let release_regex =
            Regex::new(r"\[?([^\]]+)\]?\s*-\s*([\d]{4}-[\d]{1,2}-[\d]{1,2})(\s+\[yanked\])?$")?;

        while let (Some(release), token) = self.get_content(vec![TokenKind::H2])? {
            let mut builder = ReleaseBuilder::default();
            let release_lc = release.clone().to_lowercase();

            builder.yanked(release_lc.contains("[yanked]"));

            if let Some(captures) = release_regex.captures(&release_lc) {
                let version = Version::parse(captures[1].trim())
                    .map_err(|e| eyre!("Failed to parse version: {e}"))?;

                let date = chrono::NaiveDate::parse_from_str(captures[2].trim(), "%Y-%m-%d")
                    .map_err(|e| eyre!("Failed to parse date: {e}"))?;

                builder.version(version).date(date);
            } else if release_lc.contains("unreleased") {
                if let Some(captures) = unreleased_regex.captures(&release_lc) {
                    builder.version(Version::parse(captures[1].trim())?);
                }
            } else {
                let token = token.expect("Token is None");
                bail!(
                    "Failed to parse release token at line: {}, kind: {}, content: `## {release}`. Expected format: `## [VERSION] - [DATE]` or `## [Unreleased]`",
                    token.line,
                    token.kind
                )
            }

            builder.description(self.get_text_content()?);

            while let (Some(_), Some(change_kind)) = self.get_content(vec![TokenKind::H3])? {
                while let (Some(_), Some(change)) = self.get_content(vec![TokenKind::Li])? {
                    builder.add_change(change_kind.clone(), change.clone())?;
                }
            }

            releases.push(builder.build()?);
        }

        self.builder.releases(releases);

        Ok(self)
    }

    fn parse_links(&mut self, tokens: Vec<Token>) -> Result<&mut Self> {
        let release_link_regex = Regex::new(r"^\[.*\]\:\s*(http.*?)\/(?:-\/)?compare\/.*$")?;

        let links = tokens
            .into_iter()
            .map(|t| {
                let link = t.content.join("\n");

                if self.opts.url.is_none() {
                    if let Some(captures) = release_link_regex.captures(&link) {
                        self.builder.url(Some(captures[1].to_string()));
                    }
                }

                link
            })
            .collect::<Vec<_>>();

        self.builder.links(links)?;
        Ok(self)
    }

    fn parse_footer(&mut self) -> Result<&mut Self> {
        let (footer, _) = self.get_content(vec![TokenKind::Hr])?;
        self.builder.footer(footer);
        Ok(self)
    }

    fn parse_compact(&mut self, compact: bool) {
        self.builder.compact(compact);
    }

    fn build(&self) -> Result<Changelog> {
        log::debug!("idx is {} and len is {}", self.idx, self.tokens.len());
        if self.idx != self.tokens.len() {
            bail!(
                "Unexpected tokens: {:?}, index: {}, tokens length: {}",
                self.tokens[self.idx..].to_vec(),
                self.idx,
                self.tokens.len(),
            );
        }

        self.builder
            .build()
            .map_err(|e| eyre!("Failed to build Changelog: {e}"))
    }

    fn get_content(&mut self, kinds: Vec<TokenKind>) -> Result<(Option<String>, Option<Token>)> {
        let token = self.tokens.get(self.idx);

        if token.is_none() {
            return Ok((None, None));
        }

        let token = token.unwrap().clone();

        if !kinds.iter().any(|k| *k == token.kind) {
            return Ok((None, Some(token)));
        }

        self.idx += 1;
        Ok((Some(token.content.join("\n")), Some(token)))
    }

    fn get_text_content(&mut self) -> Result<Option<String>> {
        let mut lines: Vec<String> = vec![];
        let kinds = [TokenKind::P, TokenKind::Li];

        while let Some(token) = self.tokens.get(self.idx) {
            if !kinds.iter().any(|tt| *tt == token.kind) {
                break;
            }

            self.idx += 1;

            if token.kind == TokenKind::Li {
                lines.push(format!("- {}", token.content.join("\n")));
            } else {
                lines.push(token.content.join("\n"));
            }
        }

        if lines.is_empty() {
            return Ok(None);
        }

        Ok(Some(lines.join("\n")))
    }

    fn get_lint_content(&mut self) -> Result<(Option<HashSet<String>>, Option<Token>)> {
        let kinds: Vec<TokenKind> = vec![TokenKind::Lint];

        let token = self.tokens.get(self.idx);

        if token.is_none() {
            return Ok((None, None));
        }

        let token = token.unwrap().clone();

        if !kinds.iter().any(|k| *k == token.kind) {
            return Ok((None, Some(token)));
        }

        self.idx += 1;

        let re = Regex::new(r"markdownlint-disable(?P<lints>( MD\d{3})+)")?;

        if let Some(captures) = re.captures(&token.content[0]) {
            let lints = captures
                .name("lints")
                .unwrap()
                .as_str()
                .trim()
                .split(' ')
                .map(|s| s.to_string())
                .collect::<Vec<_>>();

            let mut set = HashSet::new();
            for s in &lints {
                set.insert(s.to_string());
            }

            Ok((Some(set), Some(token)))
        } else {
            Err(eyre!("Failed to get lint content"))
        }
    }
}

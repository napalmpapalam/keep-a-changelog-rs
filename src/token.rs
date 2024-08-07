use std::fmt::Display;

use eyre::Result;
use regex::Regex;

use crate::{
    consts::*,
    utils::{is_empty_str, is_empty_str_vec, substring},
};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    H1,
    H2,
    H3,
    Li,
    P,
    Link,
    Flag,
    Hr,
    Lint,
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            TokenKind::H1 => "Heading 1",
            TokenKind::H2 => "Heading 2",
            TokenKind::H3 => "Heading 3",
            TokenKind::Li => "List Item",
            TokenKind::P => "Paragraph",
            TokenKind::Link => "Link",
            TokenKind::Flag => "Flag",
            TokenKind::Hr => "Horizontal Rule",
            TokenKind::Lint => "Lint",
        };

        write!(f, "{}", kind)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub line: usize,
    pub kind: TokenKind,
    pub content: Vec<String>,
}

impl Token {
    pub fn new(line: usize, kind: TokenKind, content: Vec<String>) -> Self {
        Self {
            line,
            kind,
            content,
        }
    }
}

pub fn tokenize(markdown: String) -> Result<(bool, Vec<Token>)> {
    let tokens: Vec<Token> = extract_tokens(markdown);
    log::trace!("Tokens: {:#?}", tokens);

    let mut compact = false;

    for (idx, token) in tokens.iter().enumerate() {
        if token.kind == TokenKind::H1 && !tokens[idx + 1].content[0].is_empty() {
            compact = true;
            break;
        }
    }

    let regex = Regex::new(r"^\s\s")?;
    let mut result: Vec<Token> = vec![];

    for (idx, token) in tokens.into_iter().enumerate() {
        let Token {
            line,
            kind,
            content,
        } = token;
        let content = content[0].clone();

        if idx > 0 {
            let prev_item_idx = result.len() - 1;
            let prev_token_kind = result[prev_item_idx].kind.clone();

            if kind == TokenKind::P {
                if prev_token_kind == TokenKind::P {
                    result[prev_item_idx].content.push(content);
                    continue;
                }

                if prev_token_kind == TokenKind::Li {
                    result[prev_item_idx]
                        .content
                        .push(regex.replace(&content, "").to_string());
                    continue;
                }
            }
        }

        result.push(Token::new(line, kind, vec![content]));
    }

    Ok((
        compact,
        result
            .into_iter()
            .filter(|t| !is_empty_str_vec(t.content.clone()))
            .map(|mut token| {
                while is_empty_str(token.content[token.content.len() - 1].clone()) {
                    token.content.pop();
                }

                while is_empty_str(token.content[0].clone()) {
                    token.content.remove(0);
                }

                token
            })
            .collect(),
    ))
}

fn extract_tokens(markdown: String) -> Vec<Token> {
    let link_regex: Regex = Regex::new(r"^\[.*\]\:\s*http.*$").unwrap();
    let link_ref_regex: Regex = Regex::new(r"^\[.*\]\:$").unwrap();
    let comment_regex: Regex = Regex::new(r"^<!--(.*)-->$").unwrap();
    let link_prefix_regex: Regex = Regex::new(r"\s+http.*$").unwrap();

    let lines = markdown.trim().split('\n').collect::<Vec<_>>();
    let mut empty_next_line = false;

    lines
        .clone()
        .into_iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let ln = idx + 1;
            let mut line = line.to_string();

            if empty_next_line {
                line = "".to_string();
                empty_next_line = false;
            }

            if line.starts_with(PREFIX_HR) {
                return Some(Token::new(ln, TokenKind::Hr, vec!["-".to_string()]));
            }

            if line.starts_with(PREFIX_H1) {
                return Some(Token::new(ln, TokenKind::H1, vec![substring(line, 1)]));
            }

            if line.starts_with(PREFIX_H2) {
                return Some(Token::new(ln, TokenKind::H2, vec![substring(line, 2)]));
            }

            if line.starts_with(PREFIX_H3) {
                return Some(Token::new(ln, TokenKind::H3, vec![substring(line, 3)]));
            }

            if line.starts_with(PREFIX_LI) || line.starts_with(PREFIX_LI2) {
                return Some(Token::new(ln, TokenKind::Li, vec![substring(line, 1)]));
            }

            if link_regex.is_match(&line) {
                return Some(Token::new(
                    ln,
                    TokenKind::Link,
                    vec![line.clone().trim().to_string()],
                ));
            }

            if link_ref_regex.is_match(&line) {
                let next_line = lines.get(idx + 1);

                if let Some(next_line) = next_line {
                    if link_prefix_regex.is_match(next_line) {
                        empty_next_line = true;
                        let line = format!("{}\n{}", line.trim(), next_line.trim_end());
                        return Some(Token::new(ln, TokenKind::Link, vec![line]));
                    }
                }
                return None;
            }

            if let Some(captures) = comment_regex.captures(&line) {
                log::debug!("Found comments: {:#?}", captures);
                let line = captures[1].trim().to_string();
                if line.starts_with("markdownlint-disable") {
                    return Some(Token::new(ln, TokenKind::Lint, vec![line]));
                } else {
                    return Some(Token::new(ln, TokenKind::Flag, vec![line]));
                }
            }

            Some(Token::new(
                ln,
                TokenKind::P,
                vec![line.trim_end().to_string()],
            ))
        })
        .collect::<Vec<_>>()
}

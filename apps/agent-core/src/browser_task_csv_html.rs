//! Bounded HTML tokenizer and entity decoding for T5.1 CSV normalization.
//!
//! This is intentionally not a general HTML5 parser. It accepts balanced,
//! bounded tags sufficient for the approved browser DOM fragment and fails
//! closed on malformed input or unsupported entities.

use std::collections::BTreeMap;

use super::BrowserCsvError;

pub(super) enum HtmlToken<'a> {
    Text(&'a str),
    Start {
        name: String,
        attributes: BTreeMap<String, String>,
        self_closing: bool,
    },
    End {
        name: String,
    },
}

pub(super) struct HtmlTokenizer<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> HtmlTokenizer<'a> {
    pub(super) const fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    pub(super) fn next_token(&mut self) -> Result<Option<HtmlToken<'a>>, BrowserCsvError> {
        if self.cursor >= self.input.len() {
            return Ok(None);
        }
        let remaining = self.input.get(self.cursor..).ok_or_else(|| {
            BrowserCsvError::Malformed("DOM cursor is outside the input".to_owned())
        })?;
        let Some(tag_offset) = remaining.find('<') else {
            self.cursor = self.input.len();
            return Ok(Some(HtmlToken::Text(remaining)));
        };
        if tag_offset > 0 {
            return self.consume_text(tag_offset).map(Some);
        }
        if remaining.starts_with("<!--") {
            self.skip_comment(remaining)?;
            return self.next_token();
        }
        self.consume_tag(remaining).map(Some)
    }

    fn consume_text(&mut self, length: usize) -> Result<HtmlToken<'a>, BrowserCsvError> {
        let text_end = self.cursor.saturating_add(length);
        let text = self
            .input
            .get(self.cursor..text_end)
            .ok_or_else(|| BrowserCsvError::Malformed("DOM text range is invalid".to_owned()))?;
        self.cursor = text_end;
        Ok(HtmlToken::Text(text))
    }

    fn skip_comment(&mut self, remaining: &str) -> Result<(), BrowserCsvError> {
        let comment_end = remaining.find("-->").ok_or_else(|| {
            BrowserCsvError::Malformed("HTML comment is not terminated".to_owned())
        })?;
        self.cursor = self.cursor.saturating_add(comment_end).saturating_add(3);
        Ok(())
    }

    fn consume_tag(&mut self, remaining: &str) -> Result<HtmlToken<'a>, BrowserCsvError> {
        let tag_end = find_tag_end(remaining)?;
        let body_start = self.cursor.saturating_add(1);
        let body_end = self.cursor.saturating_add(tag_end);
        let body = self
            .input
            .get(body_start..body_end)
            .ok_or_else(|| BrowserCsvError::Malformed("HTML tag range is invalid".to_owned()))?
            .trim();
        self.cursor = self.cursor.saturating_add(tag_end).saturating_add(1);
        if body.is_empty() || body.starts_with('!') || body.starts_with('?') {
            return self
                .next_token()?
                .ok_or_else(|| BrowserCsvError::Malformed("ignored terminal HTML tag".to_owned()));
        }
        if let Some(end_body) = body.strip_prefix('/') {
            let name = normalized_tag_name(end_body)?;
            return Ok(HtmlToken::End { name });
        }
        let (name, attributes, self_closing) = parse_start_tag(body)?;
        Ok(HtmlToken::Start {
            name,
            attributes,
            self_closing,
        })
    }
}

fn find_tag_end(remaining: &str) -> Result<usize, BrowserCsvError> {
    let mut quote = None;
    for (offset, character) in remaining.char_indices().skip(1) {
        match (quote, character) {
            (Some(active), value) if value == active => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, '>') => return Ok(offset),
            _ => {}
        }
    }
    Err(BrowserCsvError::Malformed(
        "HTML tag is not terminated".to_owned(),
    ))
}

fn normalized_tag_name(value: &str) -> Result<String, BrowserCsvError> {
    let name = value
        .trim()
        .split_ascii_whitespace()
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| BrowserCsvError::Malformed("HTML tag name is empty".to_owned()))?;
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
    {
        return Err(BrowserCsvError::Malformed(format!(
            "HTML tag name `{name}` is invalid"
        )));
    }
    Ok(name.to_ascii_lowercase())
}

fn parse_start_tag(
    body: &str,
) -> Result<(String, BTreeMap<String, String>, bool), BrowserCsvError> {
    let trimmed = body.trim();
    let self_closing = trimmed.ends_with('/');
    let without_slash = trimmed.strip_suffix('/').unwrap_or(trimmed).trim_end();
    let name_end = without_slash
        .find(|character: char| character.is_ascii_whitespace())
        .unwrap_or(without_slash.len());
    let raw_name = without_slash.get(..name_end).unwrap_or_default();
    let name = normalized_tag_name(raw_name)?;
    let attributes_text = without_slash
        .get(name_end..)
        .unwrap_or_default()
        .trim_start();
    Ok((name, parse_attributes(attributes_text)?, self_closing))
}

fn parse_attributes(input: &str) -> Result<BTreeMap<String, String>, BrowserCsvError> {
    let mut attributes = BTreeMap::new();
    let mut cursor = 0usize;
    while cursor < input.len() {
        let remaining = input.get(cursor..).ok_or_else(|| {
            BrowserCsvError::Malformed("attribute cursor is outside the tag".to_owned())
        })?;
        let skipped = remaining
            .chars()
            .take_while(char::is_ascii_whitespace)
            .map(char::len_utf8)
            .sum::<usize>();
        cursor = cursor.saturating_add(skipped);
        if cursor >= input.len() {
            break;
        }
        let remaining = input.get(cursor..).ok_or_else(|| {
            BrowserCsvError::Malformed("attribute cursor is outside the tag".to_owned())
        })?;
        let name_end = remaining
            .find(|character: char| {
                character.is_ascii_whitespace() || matches!(character, '=' | '/' | '>')
            })
            .unwrap_or(remaining.len());
        let raw_name = remaining.get(..name_end).unwrap_or_default();
        if raw_name.is_empty() {
            return Err(BrowserCsvError::Malformed(
                "attribute name is empty".to_owned(),
            ));
        }
        cursor = cursor.saturating_add(name_end);
        let after_name = input.get(cursor..).ok_or_else(|| {
            BrowserCsvError::Malformed("attribute cursor is outside the tag".to_owned())
        })?;
        let value = if after_name.trim_start().starts_with('=') {
            let whitespace = after_name
                .chars()
                .take_while(char::is_ascii_whitespace)
                .map(char::len_utf8)
                .sum::<usize>();
            cursor = cursor.saturating_add(whitespace).saturating_add(1);
            parse_attribute_value(input, &mut cursor)?
        } else {
            String::new()
        };
        let name = raw_name.to_ascii_lowercase();
        if attributes.insert(name.clone(), value).is_some() {
            return Err(BrowserCsvError::Malformed(format!(
                "duplicate HTML attribute `{name}`"
            )));
        }
    }
    Ok(attributes)
}

fn parse_attribute_value(input: &str, cursor: &mut usize) -> Result<String, BrowserCsvError> {
    let remaining = input
        .get(*cursor..)
        .ok_or_else(|| BrowserCsvError::Malformed("attribute value is missing".to_owned()))?;
    let Some(first) = remaining.chars().next() else {
        return Err(BrowserCsvError::Malformed(
            "attribute value is missing".to_owned(),
        ));
    };
    if first == '"' || first == '\'' {
        *cursor = cursor.saturating_add(first.len_utf8());
        let content = input
            .get(*cursor..)
            .ok_or_else(|| BrowserCsvError::Malformed("attribute value is malformed".to_owned()))?;
        let end = content.find(first).ok_or_else(|| {
            BrowserCsvError::Malformed("quoted attribute is not terminated".to_owned())
        })?;
        let raw = content
            .get(..end)
            .ok_or_else(|| BrowserCsvError::Malformed("attribute value is malformed".to_owned()))?;
        *cursor = cursor.saturating_add(end).saturating_add(first.len_utf8());
        decode_html_entities(raw)
    } else {
        let end = remaining
            .find(|character: char| character.is_ascii_whitespace() || character == '/')
            .unwrap_or(remaining.len());
        let raw = remaining
            .get(..end)
            .ok_or_else(|| BrowserCsvError::Malformed("attribute value is malformed".to_owned()))?;
        *cursor = cursor.saturating_add(end);
        decode_html_entities(raw)
    }
}

pub(super) fn decode_html_entities(value: &str) -> Result<String, BrowserCsvError> {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find('&') {
        let (before, entity_and_after) = remaining.split_at(start);
        output.push_str(before);
        let Some(end) = entity_and_after.find(';') else {
            return Err(BrowserCsvError::Malformed(
                "HTML entity is missing a terminator".to_owned(),
            ));
        };
        let entity = entity_and_after
            .get(1..end)
            .ok_or_else(|| BrowserCsvError::Malformed("HTML entity is malformed".to_owned()))?;
        output.push(decode_entity(entity)?);
        remaining = entity_and_after
            .get(end.saturating_add(1)..)
            .unwrap_or_default();
    }
    output.push_str(remaining);
    Ok(output)
}

fn decode_entity(entity: &str) -> Result<char, BrowserCsvError> {
    match entity {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "quot" => Ok('"'),
        "apos" => Ok('\''),
        "pound" => Ok('\u{00a3}'),
        "nbsp" => Ok('\u{00a0}'),
        numeric if numeric.starts_with("#x") || numeric.starts_with("#X") => {
            decode_numeric_entity(numeric.get(2..).unwrap_or_default(), 16)
        }
        numeric if numeric.starts_with('#') => {
            decode_numeric_entity(numeric.get(1..).unwrap_or_default(), 10)
        }
        _ => Err(BrowserCsvError::Malformed(format!(
            "unsupported HTML entity `&{entity};`"
        ))),
    }
}

fn decode_numeric_entity(value: &str, radix: u32) -> Result<char, BrowserCsvError> {
    u32::from_str_radix(value, radix)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| BrowserCsvError::Malformed("numeric HTML entity is invalid".to_owned()))
}

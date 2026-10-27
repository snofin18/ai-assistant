//! Strict T5.1 DOM-to-CSV normalization for the private browser task runner.
//!
//! Responsibilities:
//! - consume only the three committed DOM outputs from the existing runner;
//! - enforce the approved 60-row contract and serialize deterministic RFC 4180 CSV;
//! - expose bounded evidence without executing page content.
//!
//! Boundaries:
//! - no browser, network, GUI, task execution, or credential access;
//! - no general HTML parser or expression engine;
//! - no silent row skipping, defaults, overwrite, or partial success.

use std::collections::BTreeSet;

use serde_json::{Map, Value};
use thiserror::Error;

#[path = "browser_task_csv_html.rs"]
mod browser_task_csv_html;
#[path = "browser_task_csv_page.rs"]
mod browser_task_csv_page;

use browser_task_csv_page::parse_page_dom;

pub(super) const MAX_DOM_BYTES: usize = 2 * 1024 * 1024;
const PAGE_COUNT: usize = 3;
const ROWS_PER_PAGE: usize = 20;
const TOTAL_ROWS: usize = PAGE_COUNT * ROWS_PER_PAGE;
const CSV_HEADER: [&str; 8] = [
    "page_number",
    "row_number",
    "title",
    "price",
    "currency",
    "availability",
    "rating",
    "product_url",
];

/// A validated HTTP(S) origin used to resolve relative product links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CsvOrigin {
    value: String,
}

impl CsvOrigin {
    pub(super) fn parse(value: &str) -> Result<Self, BrowserCsvError> {
        let trimmed = value.trim();
        if trimmed != value || trimmed.is_empty() {
            return Err(BrowserCsvError::InvalidOrigin(
                "origin must be non-empty without surrounding whitespace".to_owned(),
            ));
        }
        let without_slash = trimmed.strip_suffix('/').unwrap_or(trimmed);
        if without_slash.contains(['?', '#']) {
            return Err(BrowserCsvError::InvalidOrigin(
                "origin must not contain a query or fragment".to_owned(),
            ));
        }
        let (scheme, authority) = without_slash.split_once("://").ok_or_else(|| {
            BrowserCsvError::InvalidOrigin("origin must use http or https".to_owned())
        })?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(BrowserCsvError::InvalidOrigin(
                "origin must use http or https".to_owned(),
            ));
        }
        if authority.is_empty()
            || authority.contains(['/', '@'])
            || authority.chars().any(|character| {
                character.is_whitespace()
                    || character.is_control()
                    || matches!(character, '\\' | '"' | '\'' | '<' | '>')
            })
        {
            return Err(BrowserCsvError::InvalidOrigin(
                "origin authority is invalid".to_owned(),
            ));
        }
        Ok(Self {
            value: format!("{scheme}://{}", authority.to_ascii_lowercase()),
        })
    }

    fn resolve_product_url(&self, raw: &str) -> Result<String, BrowserCsvError> {
        let href = raw.trim();
        if href.is_empty()
            || href
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(BrowserCsvError::Malformed(
                "product URL is empty or contains whitespace/control characters".to_owned(),
            ));
        }
        let without_fragment = href.split_once('#').map_or(href, |(value, _)| value);
        if without_fragment.contains('?') || without_fragment.contains("/../") {
            return Err(BrowserCsvError::Malformed(
                "product URL must not contain a query or parent traversal".to_owned(),
            ));
        }
        let resolved = if without_fragment.starts_with("http://")
            || without_fragment.starts_with("https://")
        {
            let origin_text = absolute_origin(without_fragment)?;
            let observed = Self::parse(origin_text)?;
            if observed != *self {
                return Err(BrowserCsvError::Contract(
                    "product URL is outside the requested origin".to_owned(),
                ));
            }
            without_fragment.to_owned()
        } else if without_fragment.starts_with('/') {
            format!("{}{without_fragment}", self.value)
        } else {
            format!("{}/{}", self.value, without_fragment)
        };
        if !resolved.starts_with(&format!("{}/", self.value)) {
            return Err(BrowserCsvError::Contract(
                "resolved product URL is outside the requested origin".to_owned(),
            ));
        }
        Ok(resolved)
    }
}

/// CSV bytes plus bounded evidence suitable for the private JSON result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CsvArtifact {
    pub(super) bytes: Vec<u8>,
    pub(super) rows: usize,
    pub(super) first_product_url: String,
    pub(super) last_product_url: String,
    pub(super) expected_origin: String,
}

/// Strict normalization failures.
#[derive(Debug, Error)]
pub(super) enum BrowserCsvError {
    #[error("CSV origin: {0}")]
    InvalidOrigin(String),
    #[error("DOM input: {0}")]
    Malformed(String),
    #[error("CSV contract: {0}")]
    Contract(String),
}

/// Normalizes the three committed DOM outputs into the approved T5.1 CSV.
pub(super) fn normalize_t5_1_outputs(
    outputs: &Map<String, Value>,
    origin: &CsvOrigin,
) -> Result<CsvArtifact, BrowserCsvError> {
    let mut rows = Vec::with_capacity(TOTAL_ROWS);
    for page_number in 1..=PAGE_COUNT {
        let output_name = format!("page_{page_number}_dom");
        let dom = outputs
            .get(&output_name)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BrowserCsvError::Contract(format!(
                    "committed output `{output_name}` is missing or not a string"
                ))
            })?;
        let page_rows = parse_page_dom(dom, page_number, origin)?;
        if page_rows.len() != ROWS_PER_PAGE {
            return Err(BrowserCsvError::Contract(format!(
                "page {page_number} contains {} rows, expected {ROWS_PER_PAGE}",
                page_rows.len()
            )));
        }
        rows.extend(page_rows);
    }
    if rows.len() != TOTAL_ROWS {
        return Err(BrowserCsvError::Contract(format!(
            "normalizer produced {} rows, expected {TOTAL_ROWS}",
            rows.len()
        )));
    }
    let mut seen_urls = BTreeSet::new();
    for row in &rows {
        if !seen_urls.insert(row.product_url.clone()) {
            return Err(BrowserCsvError::Contract(format!(
                "duplicate product_url: {}",
                row.product_url
            )));
        }
    }
    let first_product_url = rows
        .first()
        .map(|row| row.product_url.clone())
        .ok_or_else(|| BrowserCsvError::Contract("CSV contains no rows".to_owned()))?;
    let last_product_url = rows
        .last()
        .map(|row| row.product_url.clone())
        .ok_or_else(|| BrowserCsvError::Contract("CSV contains no rows".to_owned()))?;
    Ok(CsvArtifact {
        bytes: serialize_csv(&rows),
        rows: rows.len(),
        first_product_url,
        last_product_url,
        expected_origin: origin.value.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductRow {
    page_number: usize,
    row_number: usize,
    title: String,
    price: String,
    currency: &'static str,
    availability: &'static str,
    rating: u8,
    product_url: String,
}

fn serialize_csv(rows: &[ProductRow]) -> Vec<u8> {
    let mut output = String::new();
    write_csv_row(&mut output, CSV_HEADER);
    for row in rows {
        let page_number = row.page_number.to_string();
        let row_number = row.row_number.to_string();
        let rating = row.rating.to_string();
        write_csv_row(
            &mut output,
            [
                page_number.as_str(),
                row_number.as_str(),
                row.title.as_str(),
                row.price.as_str(),
                row.currency,
                row.availability,
                rating.as_str(),
                row.product_url.as_str(),
            ],
        );
    }
    output.into_bytes()
}

fn write_csv_row<'a>(output: &mut String, fields: impl IntoIterator<Item = &'a str>) {
    let mut first = true;
    for field in fields {
        if !first {
            output.push(',');
        }
        first = false;
        if field
            .chars()
            .any(|character| matches!(character, ',' | '"' | '\r' | '\n'))
        {
            output.push('"');
            for character in field.chars() {
                if character == '"' {
                    output.push('"');
                }
                output.push(character);
            }
            output.push('"');
        } else {
            output.push_str(field);
        }
    }
    output.push_str("\r\n");
}

fn absolute_origin(url: &str) -> Result<&str, BrowserCsvError> {
    let scheme_end = url.find("://").ok_or_else(|| {
        BrowserCsvError::Malformed("absolute product URL has no scheme".to_owned())
    })?;
    let authority_start = scheme_end.saturating_add(3);
    let authority_tail = url.get(authority_start..).ok_or_else(|| {
        BrowserCsvError::Malformed("absolute product URL is malformed".to_owned())
    })?;
    let authority_end = authority_tail
        .find(['/', '?', '#'])
        .unwrap_or(authority_tail.len());
    url.get(..authority_start.saturating_add(authority_end))
        .ok_or_else(|| BrowserCsvError::Malformed("absolute product URL is malformed".to_owned()))
}

#[cfg(test)]
#[path = "browser_task_csv_tests.rs"]
mod tests;

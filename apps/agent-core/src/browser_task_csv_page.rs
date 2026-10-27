//! Product-article extraction for the approved T5.1 list-page DOM shapes.

use std::collections::{BTreeMap, BTreeSet};

use super::browser_task_csv_html::{HtmlToken, HtmlTokenizer, decode_html_entities};
use super::{BrowserCsvError, CsvOrigin, ProductRow};

const RATING_NAMES: [&str; 5] = ["One", "Two", "Three", "Four", "Five"];
const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub(super) fn parse_page_dom(
    dom: &str,
    expected_page: usize,
    origin: &CsvOrigin,
) -> Result<Vec<ProductRow>, BrowserCsvError> {
    if dom.is_empty() {
        return Err(BrowserCsvError::Malformed(format!(
            "page {expected_page} DOM is empty"
        )));
    }
    if dom.len() > super::MAX_DOM_BYTES {
        return Err(BrowserCsvError::Malformed(format!(
            "page {expected_page} DOM exceeds {} bytes",
            super::MAX_DOM_BYTES
        )));
    }
    let mut tokenizer = HtmlTokenizer::new(dom);
    let mut stack = Vec::<String>::new();
    let mut article: Option<ArticleAccumulator> = None;
    let mut rows = Vec::new();
    while let Some(token) = tokenizer.next_token()? {
        match token {
            HtmlToken::Text(text) => {
                if let Some(active) = article.as_mut() {
                    active.append_text(text)?;
                }
            }
            HtmlToken::Start {
                name,
                attributes,
                self_closing,
            } => process_start(&name, &attributes, self_closing, &mut stack, &mut article)?,
            HtmlToken::End { name } => process_end(
                &name,
                &mut stack,
                &mut article,
                &mut rows,
                expected_page,
                origin,
            )?,
        }
    }
    if article.is_some() {
        return Err(BrowserCsvError::Malformed(
            "product article was not closed".to_owned(),
        ));
    }
    if !stack.is_empty() {
        return Err(BrowserCsvError::Malformed(
            "DOM contains unclosed elements".to_owned(),
        ));
    }
    Ok(rows)
}

fn process_start(
    name: &str,
    attributes: &BTreeMap<String, String>,
    self_closing: bool,
    stack: &mut Vec<String>,
    article: &mut Option<ArticleAccumulator>,
) -> Result<(), BrowserCsvError> {
    let is_article = is_product_article(attributes);
    if is_article {
        if article.is_some() {
            return Err(BrowserCsvError::Malformed(
                "nested product articles are not allowed".to_owned(),
            ));
        }
        if self_closing {
            return Err(BrowserCsvError::Malformed(
                "product article cannot be self-closing".to_owned(),
            ));
        }
        let mut accumulator = ArticleAccumulator::new(attributes);
        accumulator.article_depth = stack.len().saturating_add(1);
        *article = Some(accumulator);
    }
    if !self_closing && !is_void_element(name) {
        stack.push(name.to_owned());
    }
    if let Some(active) = article.as_mut()
        && !is_article
        && let Some(field) = field_for_tag(name, attributes)
    {
        active.begin_capture(field, attributes, stack.len())?;
        if self_closing {
            active.finish_capture_at(stack.len())?;
        }
    }
    Ok(())
}

fn process_end(
    name: &str,
    stack: &mut Vec<String>,
    article: &mut Option<ArticleAccumulator>,
    rows: &mut Vec<ProductRow>,
    expected_page: usize,
    origin: &CsvOrigin,
) -> Result<(), BrowserCsvError> {
    let expected_name = stack
        .pop()
        .ok_or_else(|| BrowserCsvError::Malformed(format!("unexpected closing tag `{name}`")))?;
    if expected_name != name {
        return Err(BrowserCsvError::Malformed(format!(
            "closing tag `{name}` does not match `{expected_name}`"
        )));
    }
    let depth = stack.len();
    if let Some(active) = article.as_mut() {
        active.finish_capture_at(depth)?;
        if name == "article" && depth.saturating_add(1) == active.article_depth {
            let row = active.finish(expected_page, rows.len().saturating_add(1), origin)?;
            rows.push(row);
            *article = None;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct ArticleAccumulator {
    article_depth: usize,
    page_number: Option<usize>,
    row_number: Option<usize>,
    title: Option<CapturedValue>,
    price: Option<CapturedValue>,
    availability: Option<CapturedValue>,
    rating: Option<CapturedValue>,
    product_url: Option<String>,
    capture: Option<FieldCapture>,
}

impl ArticleAccumulator {
    fn new(attributes: &BTreeMap<String, String>) -> Self {
        Self {
            article_depth: 1,
            page_number: parse_optional_usize(attributes.get("data-page-number")),
            row_number: parse_optional_usize(attributes.get("data-row-number")),
            title: None,
            price: None,
            availability: None,
            rating: None,
            product_url: None,
            capture: None,
        }
    }

    fn begin_capture(
        &mut self,
        field: CapturedField,
        attributes: &BTreeMap<String, String>,
        depth: usize,
    ) -> Result<(), BrowserCsvError> {
        if self.capture.is_some() {
            return Err(BrowserCsvError::Malformed(
                "product fields may not be nested".to_owned(),
            ));
        }
        if self.field_is_set(field) {
            return Err(BrowserCsvError::Malformed(format!(
                "duplicate product field `{}`",
                field.name()
            )));
        }
        if field == CapturedField::Title {
            self.product_url = attributes
                .get("data-product-url")
                .or_else(|| attributes.get("href"))
                .cloned();
        }
        self.capture = Some(FieldCapture {
            field,
            start_depth: depth,
            attributes: attributes.clone(),
            text: String::new(),
        });
        Ok(())
    }

    fn append_text(&mut self, text: &str) -> Result<(), BrowserCsvError> {
        if let Some(capture) = self.capture.as_mut() {
            let decoded = decode_html_entities(text)?;
            if !decoded.is_empty() {
                capture.text.push_str(&decoded);
            }
        }
        Ok(())
    }

    fn finish_capture_at(&mut self, depth: usize) -> Result<(), BrowserCsvError> {
        if self
            .capture
            .as_ref()
            .is_none_or(|capture| depth >= capture.start_depth)
        {
            return Ok(());
        }
        let capture = self
            .capture
            .take()
            .ok_or_else(|| BrowserCsvError::Malformed("field capture disappeared".to_owned()))?;
        let value = CapturedValue {
            attributes: capture.attributes,
            text: capture.text,
        };
        match capture.field {
            CapturedField::Title => self.title = Some(value),
            CapturedField::Price => self.price = Some(value),
            CapturedField::Availability => self.availability = Some(value),
            CapturedField::Rating => self.rating = Some(value),
        }
        Ok(())
    }

    fn finish(
        &mut self,
        expected_page: usize,
        expected_row: usize,
        origin: &CsvOrigin,
    ) -> Result<ProductRow, BrowserCsvError> {
        if self.capture.is_some() {
            return Err(BrowserCsvError::Malformed(
                "product article closed before a field".to_owned(),
            ));
        }
        if self.page_number.is_some_and(|page| page != expected_page) {
            return Err(BrowserCsvError::Contract(format!(
                "page marker does not match expected page {expected_page}"
            )));
        }
        if self.row_number.is_some_and(|row| row != expected_row) {
            return Err(BrowserCsvError::Contract(format!(
                "row marker does not match expected row {expected_row}"
            )));
        }
        let title = self.finish_title()?;
        let price = self.finish_price()?;
        self.validate_availability()?;
        let rating = self.finish_rating()?;
        let raw_url = self.product_url.take().ok_or_else(|| {
            BrowserCsvError::Contract("product URL is missing from the title anchor".to_owned())
        })?;
        let product_url = origin.resolve_product_url(&raw_url)?;
        Ok(ProductRow {
            page_number: expected_page,
            row_number: expected_row,
            title,
            price,
            currency: "GBP",
            availability: "in_stock",
            rating,
            product_url,
        })
    }

    fn finish_title(&mut self) -> Result<String, BrowserCsvError> {
        let title_value = self
            .title
            .take()
            .ok_or_else(|| BrowserCsvError::Contract("product title is missing".to_owned()))?;
        let title = if title_value.text.trim().is_empty() {
            title_value
                .attributes
                .get("title")
                .cloned()
                .unwrap_or_default()
        } else {
            title_value.text
        };
        let title = collapse_whitespace(&decode_html_entities(&title)?);
        if title.is_empty() {
            return Err(BrowserCsvError::Contract(
                "product title is empty".to_owned(),
            ));
        }
        Ok(title)
    }

    fn finish_price(&mut self) -> Result<String, BrowserCsvError> {
        let price_value = self
            .price
            .take()
            .ok_or_else(|| BrowserCsvError::Contract("product price is missing".to_owned()))?;
        parse_gbp_price(&decode_html_entities(&price_value.text)?)
    }

    fn validate_availability(&mut self) -> Result<(), BrowserCsvError> {
        let availability_value = self.availability.take().ok_or_else(|| {
            BrowserCsvError::Contract("product availability is missing".to_owned())
        })?;
        let availability_text = decode_html_entities(&availability_value.text)?;
        if availability_text.trim().eq_ignore_ascii_case("in stock")
            || availability_text.trim().eq_ignore_ascii_case("in_stock")
        {
            Ok(())
        } else {
            Err(BrowserCsvError::Contract(format!(
                "unsupported availability `{}`",
                availability_text.trim()
            )))
        }
    }

    fn finish_rating(&mut self) -> Result<u8, BrowserCsvError> {
        let rating_value = self
            .rating
            .take()
            .ok_or_else(|| BrowserCsvError::Contract("product rating is missing".to_owned()))?;
        parse_rating(&rating_value)
    }

    const fn field_is_set(&self, field: CapturedField) -> bool {
        match field {
            CapturedField::Title => self.title.is_some(),
            CapturedField::Price => self.price.is_some(),
            CapturedField::Availability => self.availability.is_some(),
            CapturedField::Rating => self.rating.is_some(),
        }
    }
}

#[derive(Debug)]
struct FieldCapture {
    field: CapturedField,
    start_depth: usize,
    attributes: BTreeMap<String, String>,
    text: String,
}

#[derive(Debug)]
struct CapturedValue {
    attributes: BTreeMap<String, String>,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturedField {
    Title,
    Price,
    Availability,
    Rating,
}

impl CapturedField {
    const fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Price => "price",
            Self::Availability => "availability",
            Self::Rating => "rating",
        }
    }
}

fn field_for_tag(tag_name: &str, attributes: &BTreeMap<String, String>) -> Option<CapturedField> {
    match attributes.get("data-field").map(String::as_str) {
        Some("title") => return Some(CapturedField::Title),
        Some("price") => return Some(CapturedField::Price),
        Some("availability") => return Some(CapturedField::Availability),
        Some("rating") => return Some(CapturedField::Rating),
        _ => {}
    }
    let classes = class_tokens(attributes);
    if tag_name == "a"
        && (attributes.contains_key("title")
            || attributes.contains_key("href")
            || attributes.contains_key("data-product-url"))
    {
        return Some(CapturedField::Title);
    }
    if classes.contains("price") || classes.contains("price_color") {
        return Some(CapturedField::Price);
    }
    if classes.contains("availability") {
        return Some(CapturedField::Availability);
    }
    if classes.contains("star-rating") {
        return Some(CapturedField::Rating);
    }
    None
}

fn is_product_article(attributes: &BTreeMap<String, String>) -> bool {
    let classes = class_tokens(attributes);
    attributes
        .get("data-testid")
        .is_some_and(|value| value == "product-row")
        || classes.contains("product-pod")
        || classes.contains("product_pod")
}

fn class_tokens(attributes: &BTreeMap<String, String>) -> BTreeSet<&str> {
    attributes
        .get("class")
        .map(|value| value.split_ascii_whitespace().collect())
        .unwrap_or_default()
}

fn parse_rating(value: &CapturedValue) -> Result<u8, BrowserCsvError> {
    if let Some(raw) = value.attributes.get("data-rating") {
        return raw
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|rating| (1..=5).contains(rating))
            .ok_or_else(|| BrowserCsvError::Contract(format!("invalid data-rating `{raw}`")));
    }
    let classes = class_tokens(&value.attributes);
    RATING_NAMES
        .iter()
        .position(|name| classes.contains(*name))
        .and_then(|index| u8::try_from(index.saturating_add(1)).ok())
        .ok_or_else(|| BrowserCsvError::Contract("star-rating name is missing".to_owned()))
}

fn parse_gbp_price(raw: &str) -> Result<String, BrowserCsvError> {
    let value = raw.trim();
    let value = value
        .strip_prefix('\u{00a3}')
        .ok_or_else(|| BrowserCsvError::Contract(format!("price does not use GBP: `{value}`")))?;
    let (whole, fraction) = value.split_once('.').ok_or_else(|| {
        BrowserCsvError::Contract(format!("price is not a decimal amount: `{value}`"))
    })?;
    if fraction.len() != 2
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (whole.len() > 1 && whole.starts_with('0'))
    {
        return Err(BrowserCsvError::Contract(format!(
            "price is not normalized: `{value}`"
        )));
    }
    Ok(format!("{whole}.{fraction}"))
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_optional_usize(value: Option<&String>) -> Option<usize> {
    value.and_then(|value| value.trim().parse::<usize>().ok())
}

fn is_void_element(name: &str) -> bool {
    VOID_ELEMENTS.contains(&name)
}

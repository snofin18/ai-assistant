//! Private tests for strict T5.1 DOM-to-CSV normalization.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use std::fmt::Write as _;

use serde_json::{Map, Value};

use super::browser_task_csv_html::decode_html_entities;
use super::{BrowserCsvError, CsvOrigin, normalize_t5_1_outputs, parse_page_dom};

const ORIGIN: &str = "http://127.0.0.1:8765";
const RATING_NAMES: [&str; 5] = ["One", "Two", "Three", "Four", "Five"];

#[test]
fn test_normalizes_three_fixture_pages_to_exact_csv_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let outputs = fixture_outputs();
    let origin = CsvOrigin::parse(ORIGIN)?;
    let artifact = normalize_t5_1_outputs(&outputs, &origin)?;
    assert_eq!(artifact.rows, 60);
    assert_eq!(
        artifact.first_product_url,
        format!("{ORIGIN}/catalogue/product-001/index.html")
    );
    assert_eq!(
        artifact.last_product_url,
        format!("{ORIGIN}/catalogue/product-060/index.html")
    );
    assert!(!artifact.bytes.starts_with(&[0xef, 0xbb, 0xbf]));

    let text = std::str::from_utf8(&artifact.bytes)?;
    let lines = text.split("\r\n").collect::<Vec<_>>();
    assert_eq!(lines.len(), 62);
    assert_eq!(
        lines.first().copied(),
        Some("page_number,row_number,title,price,currency,availability,rating,product_url")
    );
    assert!(lines.get(1).is_some_and(|line| {
        line == &format!(
            "1,1,\"Alpha, Beta\",50.00,GBP,in_stock,1,{ORIGIN}/catalogue/product-001/index.html"
        )
    }));
    assert!(lines.get(2).is_some_and(|line| {
        line == &format!(
            "1,2,\"Quoted \"\"Book\"\"\",51.37,GBP,in_stock,2,{ORIGIN}/catalogue/product-002/index.html"
        )
    }));
    assert!(
        lines
            .get(3)
            .is_some_and(|line| line.contains("Caf\u{00e9} \u{62a5}\u{544a} 003"))
    );
    assert_eq!(lines.last().copied(), Some(""));
    if let Ok(path) = std::env::var("TASK330_CSV_ARTIFACT") {
        std::fs::write(path, &artifact.bytes)?;
    }
    Ok(())
}

#[test]
fn test_real_books_list_shape_resolves_relative_product_url()
-> Result<(), Box<dyn std::error::Error>> {
    let dom = r#"
        <ol class="row">
          <li>
            <article class="product_pod">
              <div class="image_container">
                <a href="catalogue/a-light-in-the-attic_1000/index.html">
                  <img src="media/cache/cover.jpg" alt="A Light in the Attic">
                </a>
              </div>
              <h3><a href="catalogue/a-light-in-the-attic_1000/index.html"
                title="A Light in the Attic">A Light in the Attic</a></h3>
              <p class="price_color">&pound;51.77</p>
              <p class="instock availability">In stock</p>
              <p class="star-rating Three"></p>
            </article>
          </li>
        </ol>
    "#;
    let origin = CsvOrigin::parse("https://books.toscrape.com")?;
    let rows = parse_page_dom(dom, 1, &origin)?;
    assert_eq!(rows.len(), 1);
    let row = rows.first().ok_or("row is missing")?;
    assert_eq!(row.title, "A Light in the Attic");
    assert_eq!(row.price, "51.77");
    assert_eq!(row.rating, 3);
    assert_eq!(
        row.product_url,
        "https://books.toscrape.com/catalogue/a-light-in-the-attic_1000/index.html"
    );
    Ok(())
}

#[test]
fn test_href_only_anchor_is_not_product_title() {
    let dom = r#"
        <article class="product_pod">
          <div class="image_container">
            <a href="/catalogue/book/index.html">
              <img src="/media/cache/cover.jpg" alt="Book">
            </a>
          </div>
          <p class="price_color">&pound;10.00</p>
          <p class="instock availability">In stock</p>
          <p class="star-rating One"></p>
        </article>
    "#;
    let origin = CsvOrigin::parse(ORIGIN).expect("origin");
    let error = parse_page_dom(dom, 1, &origin).expect_err("href-only title must fail");
    assert!(error.to_string().contains("product title is missing"));
}

#[test]
fn test_normalizer_rejects_short_page() -> Result<(), Box<dyn std::error::Error>> {
    let mut short_outputs = fixture_outputs();
    let origin = CsvOrigin::parse(ORIGIN)?;
    short_outputs.insert("page_2_dom".to_owned(), Value::String(fixture_page(2, 19)));
    let error = normalize_t5_1_outputs(&short_outputs, &origin).expect_err("short page must fail");
    assert!(error.to_string().contains("19 rows, expected 20"));
    Ok(())
}

#[test]
fn test_normalizer_rejects_missing_dom_output() -> Result<(), Box<dyn std::error::Error>> {
    let mut outputs = fixture_outputs();
    outputs.remove("page_3_dom");
    let origin = CsvOrigin::parse(ORIGIN)?;
    let error = normalize_t5_1_outputs(&outputs, &origin).expect_err("missing DOM must fail");
    assert!(error.to_string().contains("page_3_dom"));
    Ok(())
}

#[test]
fn test_normalizer_rejects_duplicate_product_url() -> Result<(), Box<dyn std::error::Error>> {
    let mut outputs = fixture_outputs();
    let first_url = format!("{ORIGIN}/catalogue/product-001/index.html");
    let duplicate_url = format!("{ORIGIN}/catalogue/product-002/index.html");
    let page = fixture_page(1, 20).replace(&duplicate_url, &first_url);
    outputs.insert("page_1_dom".to_owned(), Value::String(page));
    let origin = CsvOrigin::parse(ORIGIN)?;
    let error = normalize_t5_1_outputs(&outputs, &origin).expect_err("duplicate URL must fail");
    assert!(error.to_string().contains("duplicate product_url"));
    Ok(())
}

#[test]
fn test_normalizer_rejects_unnormalized_price() -> Result<(), Box<dyn std::error::Error>> {
    let mut outputs = fixture_outputs();
    let page = fixture_page(1, 20).replace("&pound;51.37", "&pound;051.37");
    outputs.insert("page_1_dom".to_owned(), Value::String(page));
    let origin = CsvOrigin::parse(ORIGIN)?;
    let error = normalize_t5_1_outputs(&outputs, &origin).expect_err("invalid price must fail");
    assert!(error.to_string().contains("price is not normalized"));
    Ok(())
}

#[test]
fn test_normalizer_rejects_cross_origin_product_url() -> Result<(), Box<dyn std::error::Error>> {
    let origin = CsvOrigin::parse(ORIGIN)?;
    let dom = real_page_with_url("https://example.com/catalogue/product/index.html");
    let error = parse_page_dom(&dom, 1, &origin).expect_err("cross-origin URL must fail");
    assert!(matches!(error, BrowserCsvError::Contract(_)));
    assert!(error.to_string().contains("outside the requested origin"));
    Ok(())
}

#[test]
fn test_unknown_entity_fails_closed() {
    let error = decode_html_entities("&unknown;").expect_err("unknown entity must fail");
    assert!(error.to_string().contains("unsupported HTML entity"));
}

#[test]
fn test_real_books_rating_requires_known_name() {
    let dom = r#"
        <article class="product_pod">
          <h3><a href="/catalogue/book/index.html"
            title="Book">Book</a></h3>
          <p class="price_color">&pound;10.00</p>
          <p class="instock availability">In stock</p>
          <p class="star-rating Unknown"></p>
        </article>
    "#;
    let origin = CsvOrigin::parse(ORIGIN).expect("origin");
    let error = parse_page_dom(dom, 1, &origin).expect_err("unknown rating must fail");
    assert!(error.to_string().contains("star-rating name is missing"));
}

fn fixture_outputs() -> Map<String, Value> {
    let mut outputs = Map::new();
    for page in 1..=3 {
        outputs.insert(
            format!("page_{page}_dom"),
            Value::String(fixture_page(page, 20)),
        );
        outputs.insert(format!("page_{page}_ready"), Value::Bool(true));
    }
    outputs
}

fn fixture_page(page_number: usize, row_count: usize) -> String {
    let mut cards = String::new();
    for row_number in 1..=row_count {
        let global_index = (page_number - 1) * 20 + row_number - 1;
        let number = global_index + 1;
        let title = match number {
            1 => "Alpha, Beta".to_owned(),
            2 => "Quoted \"Book\"".to_owned(),
            3 => "Caf\u{00e9} \u{62a5}\u{544a} 003".to_owned(),
            _ => format!("Synthetic Product {number:03}"),
        };
        let escaped_title = html_escape(&title);
        let price_cents = 5000 + (global_index * 137) % 5000;
        let price = format!("{}.{:02}", price_cents / 100, price_cents % 100);
        let rating = global_index % 5 + 1;
        let product_url = format!("{ORIGIN}/catalogue/product-{number:03}/index.html");
        write!(
            cards,
            r#"
      <article class="product-pod" data-testid="product-row"
        data-page-number="{page_number}" data-row-number="{row_number}">
        <h3><a href="{product_url}" data-field="title"
          data-product-url="{product_url}">{escaped_title}</a></h3>
        <p class="price" data-field="price">&pound;{price}</p>
        <p class="availability" data-field="availability">In stock</p>
        <p class="star-rating {rating_name}" data-field="rating"
          data-rating="{rating}">{rating_name}</p>
      </article>"#,
            rating_name = RATING_NAMES[rating - 1]
        )
        .expect("writing String cannot fail");
    }
    format!(
        r#"<!doctype html><html><body><main class="product-list" data-page-number="{page_number}">{cards}
    </main></body></html>"#
    )
}

fn real_page_with_url(url: &str) -> String {
    format!(
        r#"
        <article class="product_pod">
          <h3><a href="{url}" title="Book">Book</a></h3>
          <p class="price_color">&pound;10.00</p>
          <p class="instock availability">In stock</p>
          <p class="star-rating One"></p>
        </article>
        "#
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

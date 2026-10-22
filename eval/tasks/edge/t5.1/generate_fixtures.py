#!/usr/bin/env python3
"""Generate deterministic T5.1 browser fixtures and the expected CSV."""

from __future__ import annotations

import argparse
import csv
import hashlib
import html
import io
import json
from pathlib import Path
from typing import Iterable


ORIGIN = "http://127.0.0.1:8765"
PAGE_COUNT = 3
ROWS_PER_PAGE = 20
HEADER = (
    "page_number",
    "row_number",
    "title",
    "price",
    "currency",
    "availability",
    "rating",
    "product_url",
)
RATING_NAMES = ("One", "Two", "Three", "Four", "Five")


def product_row(global_index: int) -> dict[str, str | int]:
    page_number = global_index // ROWS_PER_PAGE + 1
    row_number = global_index % ROWS_PER_PAGE + 1
    number = global_index + 1

    if number == 1:
        title = "Alpha, Beta"
    elif number == 2:
        title = 'Quoted "Book"'
    elif number == 3:
        title = "Café 报告 003"
    else:
        title = f"Synthetic Product {number:03d}"

    price_cents = 5000 + (global_index * 137) % 5000
    return {
        "page_number": page_number,
        "row_number": row_number,
        "title": title,
        "price": f"{price_cents // 100}.{price_cents % 100:02d}",
        "currency": "GBP",
        "availability": "in_stock",
        "rating": global_index % 5 + 1,
        "product_url": f"{ORIGIN}/catalogue/product-{number:03d}/index.html",
    }


def rows_for_page(page_number: int) -> list[dict[str, str | int]]:
    start = (page_number - 1) * ROWS_PER_PAGE
    return [product_row(index) for index in range(start, start + ROWS_PER_PAGE)]


def render_page(page_number: int) -> str:
    rows = rows_for_page(page_number)
    cards = []
    for row in rows:
        title = html.escape(str(row["title"]), quote=True)
        price = html.escape(str(row["price"]), quote=True)
        product_url = html.escape(str(row["product_url"]), quote=True)
        rating_number = int(row["rating"])
        rating_name = RATING_NAMES[rating_number - 1]
        cards.append(
            f"""      <article class="product-pod" data-testid="product-row"
        data-page-number="{row['page_number']}" data-row-number="{row['row_number']}">
        <h3><a href="{product_url}" data-field="title"
          data-product-url="{product_url}">{title}</a></h3>
        <p class="price" data-field="price">&pound;{price}</p>
        <p class="availability" data-field="availability">In stock</p>
        <p class="star-rating {rating_name}" data-field="rating"
          data-rating="{rating_number}">{rating_name}</p>
      </article>"""
        )

    return f"""<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>T5.1 Fixture Page {page_number}</title>
    <style>
      body {{ font-family: sans-serif; margin: 2rem; }}
      .product-pod {{ border-bottom: 1px solid #ccc; padding: 1rem 0; }}
      .price {{ font-weight: bold; }}
    </style>
  </head>
  <body>
    <h1>T5.1 Fixture Page {page_number}</h1>
    <main class="product-list" data-page-number="{page_number}">
{chr(10).join(cards)}
    </main>
  </body>
</html>
"""


def expected_rows() -> Iterable[dict[str, str | int]]:
    for page_number in range(1, PAGE_COUNT + 1):
        yield from rows_for_page(page_number)


def render_expected_csv() -> bytes:
    output = io.StringIO(newline="")
    writer = csv.DictWriter(
        output,
        fieldnames=HEADER,
        lineterminator="\r\n",
        quoting=csv.QUOTE_MINIMAL,
    )
    writer.writeheader()
    writer.writerows(expected_rows())
    return output.getvalue().encode("utf-8")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def generate(output_directory: Path) -> dict[str, object]:
    output_directory.mkdir(parents=True, exist_ok=True)
    page_hashes: dict[str, str] = {}

    for page_number in range(1, PAGE_COUNT + 1):
        page_body = render_page(page_number).encode("utf-8")
        page_path = output_directory / f"page-{page_number}.html"
        page_path.write_bytes(page_body)
        page_hashes[page_path.name] = sha256(page_body)

    expected_csv = render_expected_csv()
    expected_path = output_directory / "expected.csv"
    expected_path.write_bytes(expected_csv)

    manifest: dict[str, object] = {
        "schema_version": 1,
        "task_id": "edge.t5_1.books_list_to_csv",
        "origin": ORIGIN,
        "page_count": PAGE_COUNT,
        "rows_per_page": ROWS_PER_PAGE,
        "total_rows": PAGE_COUNT * ROWS_PER_PAGE,
        "csv_sha256": sha256(expected_csv),
        "pages": page_hashes,
    }
    (output_directory / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return manifest


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path(__file__).resolve().parent / "fixtures",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    manifest = generate(args.output_dir.resolve())
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

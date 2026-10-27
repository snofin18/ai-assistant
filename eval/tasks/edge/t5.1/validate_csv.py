#!/usr/bin/env python3
"""Validate a T5.1 product CSV against the approved fail-closed contract."""

from __future__ import annotations

import argparse
import csv
import io
import re
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import SplitResult, urlsplit


HEADER = [
    "page_number",
    "row_number",
    "title",
    "price",
    "currency",
    "availability",
    "rating",
    "product_url",
]
PAGE_COUNT = 3
ROWS_PER_PAGE = 20
TOTAL_ROWS = PAGE_COUNT * ROWS_PER_PAGE
EXPECTED_ORIGIN = "http://127.0.0.1:8765"
PRICE_PATTERN = re.compile(r"^(?:0|[1-9][0-9]*)\.[0-9]{2}$")


class CsvContractError(ValueError):
    """Raised when a CSV violates any T5.1 contract invariant."""


@dataclass(frozen=True)
class ValidationResult:
    """Summary returned after a fully validated CSV."""

    rows: int
    first_product_url: str
    last_product_url: str


def _read_strict_bytes(path: Path) -> str:
    try:
        raw = path.read_bytes()
    except FileNotFoundError as error:
        raise CsvContractError(f"output file does not exist: {path}") from error
    if not raw:
        raise CsvContractError("output file is empty")
    if raw.startswith(b"\xef\xbb\xbf"):
        raise CsvContractError("UTF-8 BOM is not allowed")

    crlf_count = raw.count(b"\r\n")
    lf_count = raw.count(b"\n")
    cr_count = raw.count(b"\r")
    if lf_count != crlf_count or cr_count != crlf_count:
        raise CsvContractError("CSV must use CRLF line endings only")

    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CsvContractError(f"CSV is not valid UTF-8: {error}") from error


def _parse_rows(text: str) -> list[list[str]]:
    reader = csv.reader(io.StringIO(text, newline=""), strict=True)
    try:
        rows = list(reader)
    except csv.Error as error:
        raise CsvContractError(f"CSV parsing failed: {error}") from error
    if not rows:
        raise CsvContractError("CSV contains no header")
    if rows[0] != HEADER:
        raise CsvContractError(f"unexpected header: {rows[0]!r}")
    return rows[1:]


def _validate_row(
    row: list[str],
    expected_page: int,
    expected_row: int,
    seen_urls: set[str],
    expected_origin: str,
) -> None:
    if len(row) != len(HEADER):
        raise CsvContractError(
            f"page {expected_page} row {expected_row} has {len(row)} fields, "
            f"expected {len(HEADER)}"
        )
    if any(field == "" for field in row):
        raise CsvContractError(
            f"page {expected_page} row {expected_row} contains an empty field"
        )

    page_text, row_text, title, price, currency, availability, rating_text, url = row
    if page_text != str(expected_page):
        raise CsvContractError(
            f"expected page_number={expected_page}, got {page_text!r}"
        )
    if row_text != str(expected_row):
        raise CsvContractError(
            f"expected row_number={expected_row}, got {row_text!r}"
        )
    if title.strip() != title:
        raise CsvContractError("title must not contain surrounding whitespace")
    if not PRICE_PATTERN.fullmatch(price):
        raise CsvContractError(f"invalid price value: {price!r}")
    if currency != "GBP":
        raise CsvContractError(f"currency must be GBP, got {currency!r}")
    if availability != "in_stock":
        raise CsvContractError(
            f"availability must be in_stock, got {availability!r}"
        )
    if rating_text not in {"1", "2", "3", "4", "5"}:
        raise CsvContractError(f"rating must be an integer in 1..5, got {rating_text!r}")
    if url in seen_urls:
        raise CsvContractError(f"duplicate product_url: {url}")

    parsed = urlsplit(url)
    if not _matches_origin(parsed, expected_origin) or parsed.fragment:
        raise CsvContractError(
            f"product_url is outside {expected_origin} or contains a fragment: {url}"
        )
    seen_urls.add(url)


def _matches_origin(parsed: SplitResult, expected_origin: str) -> bool:
    expected = urlsplit(expected_origin)
    if (
        expected.scheme not in {"http", "https"}
        or expected.hostname is None
        or expected.username is not None
        or expected.password is not None
        or expected.path not in {"", "/"}
        or expected.query
        or expected.fragment
    ):
        raise CsvContractError(f"invalid expected origin: {expected_origin!r}")
    default_port = 443 if expected.scheme == "https" else 80
    try:
        expected_port = expected.port or default_port
        actual_port = parsed.port or (443 if parsed.scheme == "https" else 80)
    except ValueError as error:
        raise CsvContractError(f"invalid URL port: {error}") from error
    return (
        parsed.scheme == expected.scheme
        and parsed.hostname is not None
        and parsed.hostname.lower() == expected.hostname.lower()
        and actual_port == expected_port
    )


def validate_file(
    path: Path,
    expected: Path | None = None,
    expected_origin: str = EXPECTED_ORIGIN,
) -> ValidationResult:
    """Validate one output CSV and optionally compare it byte-for-byte to expected."""

    text = _read_strict_bytes(path)
    rows = _parse_rows(text)
    if len(rows) != TOTAL_ROWS:
        raise CsvContractError(
            f"expected exactly {TOTAL_ROWS} data rows, got {len(rows)}"
        )

    seen_urls: set[str] = set()
    for index, row in enumerate(rows):
        expected_page = index // ROWS_PER_PAGE + 1
        expected_row = index % ROWS_PER_PAGE + 1
        _validate_row(row, expected_page, expected_row, seen_urls, expected_origin)

    if expected is not None:
        try:
            expected_bytes = expected.read_bytes()
        except FileNotFoundError as error:
            raise CsvContractError(f"expected CSV does not exist: {expected}") from error
        if path.read_bytes() != expected_bytes:
            raise CsvContractError("output CSV differs byte-for-byte from expected CSV")

    return ValidationResult(
        rows=len(rows),
        first_product_url=rows[0][7],
        last_product_url=rows[-1][7],
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--expected", type=Path)
    parser.add_argument("--expected-origin", default=EXPECTED_ORIGIN)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        result = validate_file(args.input.resolve(), args.expected, args.expected_origin)
    except CsvContractError as error:
        print(f"CSV CONTRACT FAILED: {error}")
        return 2
    print(
        "CSV CONTRACT OK: "
        f"rows={result.rows} first={result.first_product_url} "
        f"last={result.last_product_url}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

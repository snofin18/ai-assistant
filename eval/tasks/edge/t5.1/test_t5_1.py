"""Offline contract tests for the deterministic T5.1 fixture and CSV validator."""

from __future__ import annotations

import functools
import json
import sys
import tempfile
import threading
import unittest
from html.parser import HTMLParser
from http.server import ThreadingHTTPServer
from pathlib import Path
from urllib.request import urlopen


HERE = Path(__file__).resolve().parent
REPOSITORY_ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE))

import generate_fixtures  # noqa: E402
import serve_fixture  # noqa: E402
import validate_csv  # noqa: E402


class FixtureParser(HTMLParser):
    """Collect row and field attributes from one generated page."""

    def __init__(self) -> None:
        super().__init__()
        self.rows = 0
        self.fields: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        del tag
        attributes = dict(attrs)
        if attributes.get("data-testid") == "product-row":
            self.rows += 1
        field = attributes.get("data-field")
        if field is not None:
            self.fields.append(field)


class T51ContractTests(unittest.TestCase):
    """Exercise the positive and negative contract boundaries."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.fixture_directory = Path(self.temporary_directory.name)
        generate_fixtures.generate(self.fixture_directory)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_generated_fixture_has_three_complete_pages(self) -> None:
        for page_number in range(1, generate_fixtures.PAGE_COUNT + 1):
            parser = FixtureParser()
            parser.feed(
                (self.fixture_directory / f"page-{page_number}.html").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(parser.rows, generate_fixtures.ROWS_PER_PAGE)
            self.assertEqual(parser.fields.count("title"), 20)
            self.assertEqual(parser.fields.count("price"), 20)
            self.assertEqual(parser.fields.count("availability"), 20)
            self.assertEqual(parser.fields.count("rating"), 20)

    def test_expected_csv_passes_strict_validation(self) -> None:
        expected = self.fixture_directory / "expected.csv"
        result = validate_csv.validate_file(expected, expected)
        self.assertEqual(result.rows, 60)
        self.assertTrue(result.first_product_url.endswith("product-001/index.html"))
        self.assertTrue(result.last_product_url.endswith("product-060/index.html"))

    def test_loopback_server_serves_page_without_caching(self) -> None:
        handler = functools.partial(
            serve_fixture.FixtureRequestHandler,
            directory=str(self.fixture_directory),
        )
        server = ThreadingHTTPServer((serve_fixture.LOOPBACK_HOST, 0), handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            host, port = server.server_address
            with urlopen(f"http://{host}:{port}/page-1.html") as response:
                body = response.read().decode("utf-8")
                self.assertEqual(response.status, 200)
                self.assertEqual(response.headers["Cache-Control"], "no-store")
                self.assertEqual(body.count('data-testid="product-row"'), 20)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_declared_task_uses_only_registered_browser_tools(self) -> None:
        task_path = (
            REPOSITORY_ROOT
            / "adapters"
            / "browser.edge"
            / "tasks"
            / "t5.1.books-list-to-csv.json"
        )
        tools_path = (
            REPOSITORY_ROOT / "adapters" / "browser.edge" / "tools" / "tools.json"
        )
        task = json.loads(task_path.read_text(encoding="utf-8"))
        tools_document = json.loads(tools_path.read_text(encoding="utf-8"))
        registered = {
            definition["schema"]["name"]
            for definition in tools_document["tools"]
        }
        used = {
            step["tool"]
            for step in task["steps"]
            if step["kind"] == "tool"
        }

        self.assertEqual(task["app_id"], "com.microsoft.edge")
        self.assertEqual(
            task["runtime_status"],
            "blocked_pending_task_package_assertion_mapping",
        )
        self.assertLessEqual(used, registered)
        self.assertEqual(
            [step["id"] for step in task["steps"]],
            [
                "navigate_page_1",
                "read_page_1",
                "navigate_page_2",
                "read_page_2",
                "navigate_page_3",
                "read_page_3",
            ],
        )

    def test_cases_match_validator_header(self) -> None:
        cases = json.loads((HERE / "cases.json").read_text(encoding="utf-8"))
        self.assertEqual(cases["expected"]["header"], validate_csv.HEADER)
        self.assertEqual(cases["expected"]["total_rows"], validate_csv.TOTAL_ROWS)

    def test_external_smoke_contract_is_bounded_and_complete(self) -> None:
        contract = json.loads(
            (HERE / "external-smoke-contract.json").read_text(encoding="utf-8")
        )
        self.assertEqual(contract["base_url"], "https://books.toscrape.com")
        self.assertEqual(contract["limits"]["page_count"], 3)
        self.assertEqual(contract["limits"]["rows_per_page"], 20)
        self.assertEqual(contract["limits"]["total_rows"], 60)
        self.assertFalse(contract["limits"]["requires_login"])
        self.assertFalse(contract["limits"]["requires_credentials"])
        self.assertEqual(contract["limits"]["allowed_methods"], ["GET"])
        self.assertEqual(
            [page["page_number"] for page in contract["allowed_pages"]],
            [1, 2, 3],
        )
        self.assertTrue(
            all(page["http_status"] == 200 for page in contract["allowed_pages"])
        )
        self.assertTrue(
            all(page["article_count"] == 20 for page in contract["allowed_pages"])
        )

    def _write_variant(self, transform) -> Path:  # noqa: ANN001
        source = (self.fixture_directory / "expected.csv").read_bytes()
        target = self.fixture_directory / "variant.csv"
        target.write_bytes(transform(source))
        return target

    def test_rejects_utf8_bom(self) -> None:
        target = self._write_variant(lambda raw: b"\xef\xbb\xbf" + raw)
        with self.assertRaisesRegex(validate_csv.CsvContractError, "BOM"):
            validate_csv.validate_file(target)

    def test_rejects_bare_lf(self) -> None:
        target = self._write_variant(lambda raw: raw.replace(b"\r\n", b"\n"))
        with self.assertRaisesRegex(validate_csv.CsvContractError, "CRLF"):
            validate_csv.validate_file(target)

    def test_rejects_wrong_header(self) -> None:
        target = self._write_variant(
            lambda raw: raw.replace(b"page_number", b"page", 1)
        )
        with self.assertRaisesRegex(validate_csv.CsvContractError, "header"):
            validate_csv.validate_file(target)

    def test_rejects_invalid_rating(self) -> None:
        target = self._write_variant(
            lambda raw: raw.replace(b",1,http://127.0.0.1:8765", b",9,http://127.0.0.1:8765", 1)
        )
        with self.assertRaisesRegex(validate_csv.CsvContractError, "rating"):
            validate_csv.validate_file(target)

    def test_rejects_cross_origin_url(self) -> None:
        target = self._write_variant(
            lambda raw: raw.replace(
                b"http://127.0.0.1:8765/catalogue/product-001",
                b"https://example.com/catalogue/product-001",
                1,
            )
        )
        with self.assertRaisesRegex(validate_csv.CsvContractError, "outside"):
            validate_csv.validate_file(target)

    def test_rejects_missing_output_file(self) -> None:
        with self.assertRaisesRegex(validate_csv.CsvContractError, "does not exist"):
            validate_csv.validate_file(self.fixture_directory / "missing.csv")


if __name__ == "__main__":
    unittest.main()

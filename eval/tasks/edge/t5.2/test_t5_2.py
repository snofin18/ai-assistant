"""Offline contract tests for the deterministic T5.2 form fixture."""

from __future__ import annotations

import tempfile
import threading
import unittest
from html.parser import HTMLParser
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urljoin, urlparse
from urllib.request import urlopen

import generate_fixtures
import serve_fixture


class FormParser(HTMLParser):
    """Collect the bounded form shape needed by the T5.2 contract."""

    def __init__(self) -> None:
        super().__init__()
        self.tags: list[tuple[str, dict[str, str]]] = []

    def handle_starttag(
        self,
        tag: str,
        attrs: list[tuple[str, str | None]],
    ) -> None:
        attributes = {
            name: value or ""
            for name, value in attrs
        }
        self.tags.append((tag, attributes))

    def by_id(self, element_id: str) -> list[tuple[str, dict[str, str]]]:
        return [
            (tag, attributes)
            for tag, attributes in self.tags
            if attributes.get("id") == element_id
        ]


def parse_form(path: Path) -> FormParser:
    parser = FormParser()
    parser.feed(path.read_text(encoding="utf-8"))
    return parser


class T52ContractTests(unittest.TestCase):
    """Exercise fixture generation, browser shape, and loopback serving."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.fixture_directory = Path(self.temporary_directory.name)
        generate_fixtures.generate(self.fixture_directory)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_generated_form_has_exact_unique_contract_ids(self) -> None:
        parser = parse_form(self.fixture_directory / "form.html")
        expected = {
            "profile-form": "form",
            "display-name": "input",
            "email": "input",
            "note": "textarea",
            "profile-submit": "button",
        }
        for element_id, expected_tag in expected.items():
            matches = parser.by_id(element_id)
            self.assertEqual(len(matches), 1, element_id)
            self.assertEqual(matches[0][0], expected_tag, element_id)

    def test_form_fields_and_action_match_the_frozen_contract(self) -> None:
        parser = parse_form(self.fixture_directory / "form.html")
        form = parser.by_id("profile-form")[0][1]
        self.assertEqual(form["action"], "/t5.2/submitted")
        self.assertEqual(form["method"].lower(), "get")

        self.assertEqual(parser.by_id("display-name")[0][1]["type"], "text")
        self.assertEqual(parser.by_id("email")[0][1]["type"], "email")
        self.assertEqual(parser.by_id("profile-submit")[0][1]["type"], "submit")
        self.assertNotIn("formaction", parser.by_id("profile-submit")[0][1])
        self.assertNotIn("formtarget", parser.by_id("profile-submit")[0][1])

    def test_form_has_no_forbidden_control_or_external_asset(self) -> None:
        parser = parse_form(self.fixture_directory / "form.html")
        forbidden_input_types = {
            "button",
            "file",
            "hidden",
            "image",
            "password",
            "reset",
            "submit",
        }
        for tag, attributes in parser.tags:
            if tag == "input":
                self.assertNotIn(
                    attributes.get("type", "text").lower(),
                    forbidden_input_types,
                )
            self.assertFalse(attributes.get("src", "").startswith(("http:", "https:")))
            self.assertFalse(attributes.get("href", "").startswith(("http:", "https:")))
            self.assertNotEqual(tag, "script")

    def test_form_action_resolves_to_same_origin_submission_url(self) -> None:
        parser = parse_form(self.fixture_directory / "form.html")
        action = parser.by_id("profile-form")[0][1]["action"]
        base = "http://127.0.0.1:8766/form.html"
        resolved = urljoin(base, action)
        self.assertEqual(resolved, "http://127.0.0.1:8766/t5.2/submitted")
        self.assertEqual(urlparse(resolved).netloc, urlparse(base).netloc)

    def test_success_page_contains_the_fixed_marker(self) -> None:
        submitted = (
            self.fixture_directory / "t5.2" / "submitted"
        ).read_text(encoding="utf-8")
        self.assertIn('data-testid="submitted-marker"', submitted)
        self.assertIn('data-contract="t5.2-submitted-v1"', submitted)
        self.assertIn("Submission received.", submitted)

    def test_generation_is_deterministic(self) -> None:
        second_directory = self.fixture_directory / "second"
        generate_fixtures.generate(second_directory)
        for relative_path in (Path("form.html"), Path("t5.2") / "submitted"):
            self.assertEqual(
                (self.fixture_directory / relative_path).read_bytes(),
                (second_directory / relative_path).read_bytes(),
            )

    def test_loopback_server_serves_form_without_caching(self) -> None:
        server = serve_fixture.make_server(self.fixture_directory)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            self.assertEqual(server.server_address[0], "127.0.0.1")
            host, port = server.server_address
            with urlopen(f"http://{host}:{port}/form.html") as response:
                body = response.read().decode("utf-8")
                self.assertEqual(response.status, 200)
                self.assertEqual(response.headers["Cache-Control"], "no-store")
                self.assertEqual(response.headers["X-Content-Type-Options"], "nosniff")
                self.assertEqual(response.headers["X-Frame-Options"], "DENY")
                self.assertIn('id="profile-form"', body)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_loopback_server_serves_submission_marker_as_html(self) -> None:
        server = serve_fixture.make_server(self.fixture_directory)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            host, port = server.server_address
            url = (
                f"http://{host}:{port}/t5.2/submitted"
                "?display-name=Ada&email=ada%40example.test&note=hello"
            )
            with urlopen(url) as response:
                body = response.read().decode("utf-8")
                self.assertEqual(response.status, 200)
                self.assertEqual(response.headers["Content-Type"], "text/html")
                self.assertIn('data-testid="submitted-marker"', body)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_loopback_server_disables_directory_listing(self) -> None:
        server = serve_fixture.make_server(self.fixture_directory)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            host, port = server.server_address
            try:
                urlopen(f"http://{host}:{port}/")
            except HTTPError as error:
                try:
                    self.assertEqual(error.code, 404)
                finally:
                    error.close()
            else:
                self.fail("directory listing must return HTTP 404")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


if __name__ == "__main__":
    unittest.main()

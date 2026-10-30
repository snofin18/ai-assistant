#!/usr/bin/env python3
"""Serve the generated T5.2 form fixture on the loopback interface only."""

from __future__ import annotations

import argparse
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


LOOPBACK_HOST = "127.0.0.1"
DEFAULT_PORT = 8766


class FixtureRequestHandler(SimpleHTTPRequestHandler):
    """Serve fixture files without caching, directory listings, or framing."""

    def list_directory(self, path: str):  # noqa: ANN001, D401
        self.send_error(404, "Directory listing disabled")
        return None

    def guess_type(self, path: str) -> str:
        if Path(path).name == "submitted":
            return "text/html"
        return super().guess_type(path)

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("X-Frame-Options", "DENY")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'none'; form-action 'self'; frame-ancestors 'none'",
        )
        super().end_headers()


def make_server(root: Path, port: int = 0) -> ThreadingHTTPServer:
    """Bind a fixture server to loopback and return it for serving or tests."""

    resolved_root = root.resolve()
    if not resolved_root.is_dir():
        raise ValueError(
            f"fixture directory does not exist: {resolved_root}; "
            "run generate_fixtures.py first"
        )
    if port < 0 or port > 65535:
        raise ValueError("port must be in 0..65535")

    handler = functools.partial(
        FixtureRequestHandler,
        directory=str(resolved_root),
    )
    server = ThreadingHTTPServer((LOOPBACK_HOST, port), handler)
    server.daemon_threads = True
    return server


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent / "fixtures",
    )
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.port < 1 or args.port > 65535:
        raise SystemExit("port must be in 1..65535")

    server = make_server(args.root, args.port)
    host, port = server.server_address
    print(f"serving T5.2 fixture at http://{host}:{port}/form.html")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

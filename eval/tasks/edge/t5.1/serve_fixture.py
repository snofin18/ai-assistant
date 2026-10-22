#!/usr/bin/env python3
"""Serve generated T5.1 fixtures on the loopback interface only."""

from __future__ import annotations

import argparse
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


LOOPBACK_HOST = "127.0.0.1"


class FixtureRequestHandler(SimpleHTTPRequestHandler):
    """Serve only generated fixture files with deterministic cache headers."""

    def list_directory(self, path: str):  # noqa: ANN001, D401
        self.send_error(404, "Directory listing disabled")
        return None

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        super().end_headers()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent / "fixtures",
    )
    parser.add_argument("--port", type=int, default=8765)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    if not root.is_dir():
        raise SystemExit(
            f"fixture directory does not exist: {root}; "
            "run generate_fixtures.py first"
        )
    if args.port < 1 or args.port > 65535:
        raise SystemExit("port must be in 1..65535")

    handler = functools.partial(FixtureRequestHandler, directory=str(root))
    server = ThreadingHTTPServer((LOOPBACK_HOST, args.port), handler)
    print(f"serving {root} at http://{LOOPBACK_HOST}:{args.port}/")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

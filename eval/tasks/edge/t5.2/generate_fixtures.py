#!/usr/bin/env python3
"""Generate the deterministic T5.2 local form fixture."""

from __future__ import annotations

import argparse
from pathlib import Path


FORM_HTML = """<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>T5.2 local form</title>
</head>
<body>
  <main>
    <h1>T5.2 local form</h1>
    <form id="profile-form" action="/t5.2/submitted" method="get">
      <p>
        <label for="display-name">Display name</label>
        <input id="display-name" name="display-name" type="text" autocomplete="off">
      </p>
      <p>
        <label for="email">Email</label>
        <input id="email" name="email" type="email" autocomplete="off">
      </p>
      <p>
        <label for="note">Note</label>
        <textarea id="note" name="note" rows="4"></textarea>
      </p>
      <button id="profile-submit" type="submit">Submit synthetic profile</button>
    </form>
  </main>
</body>
</html>
"""


SUBMITTED_HTML = """<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>T5.2 submitted</title>
</head>
<body>
  <main data-testid="submitted-marker" data-contract="t5.2-submitted-v1">
    Submission received.
  </main>
</body>
</html>
"""


def generate(output_directory: Path) -> None:
    """Write the form and success page under ``output_directory``."""

    submitted_directory = output_directory / "t5.2"
    submitted_directory.mkdir(parents=True, exist_ok=True)
    (output_directory / "form.html").write_bytes(FORM_HTML.encode("utf-8"))
    (submitted_directory / "submitted").write_bytes(SUBMITTED_HTML.encode("utf-8"))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).resolve().parent / "fixtures",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    generate(args.output.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

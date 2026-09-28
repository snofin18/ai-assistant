"""Generate deterministic fixtures for the Notepad T1.2 benchmark."""

from __future__ import annotations

import argparse
from pathlib import Path


OLD_TEXT = "\u62a5\u8868"
NEW_TEXT = "\u62a5\u544a"

SINGLE_OCCURRENCE = f"\u7b2c\u4e00\u5b63\u5ea6{OLD_TEXT}\n\n\u7b2c\u4e8c\u6bb5\u65e0\u5173\u952e\u8bcd\n"
MULTIPLE_OCCURRENCES = (
    f"{OLD_TEXT}\u603b\u8868\n"
    f"\u5206\u533a\u57df{OLD_TEXT}\n"
    f"\u5b63\u5ea6{OLD_TEXT}\u590d\u76d8\n"
)
CRLF_CJK = (
    f"\u7b2c\u4e00\u884c{OLD_TEXT}\r\n"
    f"\u7b2c\u4e8c\u884c\u65e0\u5173\u952e\u8bcd\r\n"
    f"\u7b2c\u4e09\u884c{OLD_TEXT}\u5b8c\u6210\r\n"
)
NO_MATCH = "\u666e\u901a\u6587\u672c\n\u6ca1\u6709\u76ee\u6807\u8bcd\n"
WRONG_COUNT = f"{OLD_TEXT}\u4e00\n{OLD_TEXT}\u4e8c\n"


def write_text(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8", newline="")


def generate(output_directory: Path) -> None:
    output_directory.mkdir(parents=True, exist_ok=True)
    fixtures = {
        "single_occurrence.txt": (SINGLE_OCCURRENCE, 1),
        "multiple_occurrences.txt": (MULTIPLE_OCCURRENCES, 3),
        "crlf_cjk.txt": (CRLF_CJK, 2),
        "no_match.txt": (NO_MATCH, 0),
        "wrong_count.txt": (WRONG_COUNT, 2),
    }

    for name, (text, expected_count) in fixtures.items():
        actual_count = text.count(OLD_TEXT)
        if actual_count != expected_count:
            raise AssertionError(
                f"{name} expected {expected_count} occurrences, got {actual_count}"
            )
        write_text(output_directory / name, text)

    print(f"fixtures_written={output_directory}")
    print(f"fixture_count={len(fixtures)}")
    print(f"old_text_occurrences={sum(count for _, count in fixtures.values())}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-directory", required=True)
    args = parser.parse_args()
    generate(Path(args.output_directory).resolve())


if __name__ == "__main__":
    main()

"""Generate deterministic fixtures for the Notepad T1.1 benchmark."""

from __future__ import annotations

import argparse
from pathlib import Path


NORMAL_LINES = ["alpha", "beta report", "gamma", "", "delta report", "omega"]
KEYWORD_PARAGRAPHS = [
    "intro",
    "urgent report starts",
    "still urgent",
    "",
    "ordinary",
    "report appears again",
    "",
    "tail",
]


def write_text(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8", newline="")


def generate(output_directory: Path) -> None:
    output_directory.mkdir(parents=True, exist_ok=True)
    write_text(output_directory / "normal_lf.txt", "\n".join(NORMAL_LINES) + "\n")
    write_text(output_directory / "normal_crlf.txt", "\r\n".join(NORMAL_LINES) + "\r\n")
    write_text(output_directory / "normal_cr.txt", "\r".join(NORMAL_LINES) + "\r")
    write_text(
        output_directory / "mixed_eol.txt",
        "alpha\r\nbeta report\ngamma\r\r\ndelta report\nomega\r\n",
    )
    write_text(
        output_directory / "cjk_utf8.txt",
        "\u7b2c\u4e00\u6bb5 \u62a5\u8868\n\u7b2c\u4e8c\u884c\n\n\u7b2c\u4e09\u6bb5 \u62a5\u544a\n",
    )
    write_text(output_directory / "empty.txt", "")
    write_text(output_directory / "one_line_no_keyword.txt", "only one line")
    write_text(
        output_directory / "keyword_multiple_paragraphs.txt",
        "\n".join(KEYWORD_PARAGRAPHS) + "\n",
    )

    large_parts: list[str] = []
    for index in range(35000):
        large_parts.append(f"report paragraph {index:05d}\n")
        large_parts.append(f"detail {index:05d}\n\n")
    large_text = "".join(large_parts)
    write_text(output_directory / "large_1mb.txt", large_text)
    large_bytes = (output_directory / "large_1mb.txt").stat().st_size
    if large_bytes < 1_048_576:
        raise AssertionError(f"large_1mb.txt is smaller than 1 MiB: {large_bytes}")
    print(f"fixtures_written={output_directory}")
    print(f"large_1mb_bytes={large_bytes}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-directory", required=True)
    args = parser.parse_args()
    generate(Path(args.output_directory).resolve())


if __name__ == "__main__":
    main()

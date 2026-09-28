"""Generate deterministic fixtures for the Notepad T1.3 benchmark."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


ASCII_TEXT = "alpha\nbeta\n"
UTF8_CJK_TEXT = "\u7b2c\u4e00\u884c\n\u7b2c\u4e8c\u884c\n"
SPACES_TEXT = "path with spaces\nkept intact\n"
EXISTING_SOURCE_TEXT = "new content for an existing target\n"
RACE_SOURCE_TEXT = "content created during the race window\n"


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="")


def generate(output_directory: Path) -> None:
    output_directory.mkdir(parents=True, exist_ok=True)
    source_directory = output_directory / "source"
    target_directory = output_directory / "targets"
    protected_target_path = target_directory / "existing-target.txt"
    race_target_path = target_directory / "race-target.txt"

    fixtures = {
        source_directory / "ascii.txt": ASCII_TEXT,
        source_directory / "utf8_cjk.txt": UTF8_CJK_TEXT,
        source_directory / "path_with_spaces.txt": SPACES_TEXT,
        source_directory / "empty.txt": "",
        source_directory / "existing-target-source.txt": EXISTING_SOURCE_TEXT,
        source_directory / "race-source.txt": RACE_SOURCE_TEXT,
        protected_target_path: "original target must remain unchanged\n",
        race_target_path: "race target must remain unchanged\n",
    }
    for path, text in fixtures.items():
        write_text(path, text)

    (target_directory / "folder with spaces").mkdir(parents=True, exist_ok=True)
    manifest = {
        "schema_version": 1,
        "source_ascii": "source/ascii.txt",
        "source_utf8_cjk": "source/utf8_cjk.txt",
        "source_path_with_spaces": "source/path_with_spaces.txt",
        "source_empty": "source/empty.txt",
        "source_existing_target": "source/existing-target-source.txt",
        "source_race": "source/race-source.txt",
        "protected_existing_target": "targets/existing-target.txt",
        "protected_race_target": "targets/race-target.txt",
        "space_directory": "targets/folder with spaces",
    }
    manifest_path = output_directory / "manifest.json"
    manifest_path.write_text(
        json.dumps(manifest, indent=2, ensure_ascii=True) + "\n",
        encoding="utf-8",
        newline="",
    )

    print(f"fixtures_written={output_directory}")
    print(f"fixture_count={len(fixtures)}")
    print(f"manifest={manifest_path.name}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-directory", required=True)
    args = parser.parse_args()
    generate(Path(args.output_directory).resolve())


if __name__ == "__main__":
    main()

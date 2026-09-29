/**
 * Commit-message gate.
 *
 * The repository's good commits already look like `feat(task-103): ...`, so the
 * conventional baseline applies unchanged. Two baseline rules are turned off on
 * purpose: `subject-case` and `body-max-line-length` assume English prose,
 * while this repo writes bilingual subjects and bodies.
 *
 * Merge and revert commits stay ignored (`defaultIgnores`), so history does not
 * have to be rewritten for the gate to pass.
 */

export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-enum": [
      2,
      "always",
      [
        "build",
        "chore",
        "ci",
        "docs",
        "feat",
        "fix",
        "perf",
        "refactor",
        "revert",
        "style",
        "test",
      ],
    ],
    "type-case": [2, "always", "lower-case"],
    "scope-case": [2, "always", "lower-case"],
    "subject-empty": [2, "never"],
    "header-max-length": [2, "always", 100],
    "subject-case": [0],
    "body-max-line-length": [0],
  },
};

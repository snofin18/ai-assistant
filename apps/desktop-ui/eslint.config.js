/**
 * ESLint flat config for the desktop UI.
 *
 * `format:check` owns formatting; this config owns correctness rules. AGENTS
 * §5.4 forbids `any` and `console.*`, so both are errors rather than warnings —
 * a warning would be ignored by `--max-warnings 0` anyway.
 */

import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: ["dist/**", "node_modules/**", "src-tauri/target/**", "src-tauri/gen/**"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx,mjs,js}"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: { ...globals.browser, ...globals.node },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "error",
      "no-console": "error",
    },
  },
  {
    // Test files may assert with node:test/assert and vitest matchers.
    files: ["**/*.test.{ts,tsx,mjs}"],
    rules: { "no-console": "off" },
  },
  prettier,
);

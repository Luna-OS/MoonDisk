import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import jsxA11y from "eslint-plugin-jsx-a11y";
import tseslint from "typescript-eslint";
import prettierConfig from "eslint-config-prettier";

// MoonDisk ESLint configuration.
//
// Two rules here are not style preferences, they enforce the security
// boundary described in docs/architecture.md §7 ("Frontend-Architektur"):
// the webview must never talk to the network directly (all network access
// goes through Rust, see docs/safety-model.md) and must never inject raw
// HTML (labels/paths are untrusted input and must stay escaped).
const noDirectNetworkAccess = {
  name: "moondisk/no-direct-network-access",
  rules: {
    "no-restricted-globals": [
      "error",
      {
        name: "fetch",
        message:
          "MoonDisk darf im Frontend keine eigenen Netzwerkaufrufe machen. Nutze einen Tauri-Command (siehe src/lib/ipc.ts).",
      },
      {
        name: "XMLHttpRequest",
        message:
          "MoonDisk darf im Frontend keine eigenen Netzwerkaufrufe machen. Nutze einen Tauri-Command (siehe src/lib/ipc.ts).",
      },
      {
        name: "WebSocket",
        message: "MoonDisk darf im Frontend keine eigenen Netzwerkverbindungen aufbauen.",
      },
    ],
    "no-restricted-syntax": [
      "error",
      {
        selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']",
        message:
          "dangerouslySetInnerHTML is not allowed. Free text such as labels must always stay escaped.",
      },
    ],
  },
};

export default tseslint.config(
  { ignores: ["dist", "src-tauri/target", "src/types/generated", "node_modules"] },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommendedTypeChecked,
      jsxA11y.flatConfigs.recommended,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
  noDirectNetworkAccess,
  {
    // Generated types are not hand-written and not linted for style.
    files: ["src/types/generated/**/*.ts"],
    rules: {
      "@typescript-eslint/consistent-type-imports": "off",
    },
  },
  {
    files: ["*.config.{js,ts}", "vite.config.ts"],
    languageOptions: {
      globals: globals.node,
    },
  },
  prettierConfig,
);

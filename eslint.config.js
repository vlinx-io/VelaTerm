// ESLint flat config with one critical architectural rule:
//   Application code must not import platform APIs (@tauri-apps/* or electron) directly. All platform
//   differences must go through the src/platform/ adapter layer. See §2 and §3.1 of the overall Electron
//   migration plan under docs/optimize/plans.
//
// Deliberately **do not enable any recommended rule set**. The project did not previously use ESLint, so this
// configuration adds only the rule needed to protect the dual-platform foundation without surfacing unrelated
// legacy style issues. The adapter layer and the low-level transport/notification modules (src/platform,
// src/ipc, and src/notify.ts) are where platform differences converge, so they are exempt.

import tsParser from "@typescript-eslint/parser";

// Legacy code contains scattered `// eslint-disable-next-line react-hooks/exhaustive-deps` directives.
// This minimal configuration does not install eslint-plugin-react-hooks. Register no-op rules under the same
// names so ESLint can parse those historical directives without reporting undefined rules. This does not enable
// real Hooks checks; it merely keeps the old directives valid.
const noopRule = { meta: { schema: [] }, create: () => ({}) };
const reactHooksStub = {
  rules: { "exhaustive-deps": noopRule, "rules-of-hooks": noopRule },
};

/** Block static @tauri-apps imports through no-restricted-imports patterns and electron through paths. */
const restrictedImports = {
  paths: [
    {
      name: "electron",
      message: "Do not import electron directly; platform capabilities go through the src/platform/ adapter layer.",
    },
  ],
  patterns: [
    {
      group: ["@tauri-apps/*", "@tauri-apps/**"],
      message: "Do not import @tauri-apps directly; platform capabilities go through the src/platform/ adapter layer.",
    },
  ],
};

/** no-restricted-imports does not cover dynamic import(), so AST selectors in no-restricted-syntax close that gap. */
const restrictedSyntax = [
  {
    selector: "ImportExpression[source.value=/^@tauri-apps\\//]",
    message: "Do not dynamically import @tauri-apps; platform capabilities go through the src/platform/ adapter layer.",
  },
  {
    selector: "ImportExpression[source.value='electron']",
    message: "Do not dynamically import electron; platform capabilities go through the src/platform/ adapter layer.",
  },
];

export default [
  {
    // Ignore nested session worktrees, which contain duplicate source trees.
    ignores: [
      "dist/**",
      "node_modules/**",
      "src-tauri/**",
      "electron-poc/**",
      ".vlx-worktrees/**",
    ],
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    plugins: { "react-hooks": reactHooksStub },
    // Do not report unused eslint-disable directives. The placeholder rules above never fire, so their legacy
    // directives would otherwise be considered unused. This minimal configuration intentionally ignores that
    // distinction to keep its output focused.
    linterOptions: { reportUnusedDisableDirectives: "off" },
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        ecmaFeatures: { jsx: true },
      },
    },
    rules: {
      "no-restricted-imports": ["error", restrictedImports],
      "no-restricted-syntax": ["error", ...restrictedSyntax],
    },
  },
  {
    // Exempt the adapter layer and low-level transport/notification modules, where platform differences converge.
    files: ["src/platform/**/*.{ts,tsx}", "src/ipc/**/*.{ts,tsx}", "src/notify.ts"],
    rules: {
      "no-restricted-imports": "off",
      "no-restricted-syntax": "off",
    },
  },
];

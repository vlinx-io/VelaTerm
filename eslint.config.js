// ESLint flat config with architectural and browser-compatibility rules:
//   Application code must not import platform APIs (@tauri-apps/* or electron) directly. All platform
//   differences must go through the src/platform/ adapter layer. See §2 and §3.1 of the overall Electron
//   migration plan under docs/optimize/plans.
//
// Deliberately **do not enable any recommended rule set**. The project did not previously use ESLint, so this
// configuration protects the dual-platform foundation and plain-HTTP browser access without surfacing unrelated
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

/** Block relative pathname+search URL assembly and empty-string history URLs that break on Tauri's empty-pathname tauri://localhost origin. */
const noRelativeHistoryUrlRule = {
  meta: {
    type: "problem",
    schema: [],
  },
  create(context) {
    const MSG =
      "Use url.href instead of concatenating pathname + search (+ hash). On Tauri's tauri://localhost origin pathname is empty, so clearing query params produces \"\" and makes history.pushState/replaceState a no-op.";
    const isProp = (node, name) =>
      node &&
      node.type === "MemberExpression" &&
      !node.computed &&
      node.property.type === "Identifier" &&
      node.property.name === name;
    const flattenAdd = (node, out = []) => {
      if (node && node.type === "BinaryExpression" && node.operator === "+") {
        flattenAdd(node.left, out);
        flattenAdd(node.right, out);
      } else if (node) {
        out.push(node);
      }
      return out;
    };
    return {
      BinaryExpression(node) {
        if (node.operator !== "+") return;
        if (node.parent && node.parent.type === "BinaryExpression" && node.parent.operator === "+") return;
        const parts = flattenAdd(node);
        if (parts.some((p) => isProp(p, "pathname")) && parts.some((p) => isProp(p, "search"))) {
          context.report({ node, message: MSG });
        }
      },
      TemplateLiteral(node) {
        if (
          node.expressions.some((e) => isProp(e, "pathname")) &&
          node.expressions.some((e) => isProp(e, "search"))
        ) {
          context.report({ node, message: MSG });
        }
      },
      CallExpression(node) {
        const callee = node.callee;
        if (!callee || callee.type !== "MemberExpression") return;
        const method =
          !callee.computed && callee.property.type === "Identifier"
            ? callee.property.name
            : null;
        if (method !== "pushState" && method !== "replaceState") return;
        const target = node.arguments[2];
        if (target && target.type === "Literal" && target.value === "") {
          context.report({
            node: target,
            message:
              "Do not pass an empty string to history.pushState/replaceState; pass a full url.href instead.",
          });
        }
      },
    };
  },
};

const vlxPlugin = {
  rules: { "no-relative-history-url": noRelativeHistoryUrlRule },
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
    // .vlx-worktrees holds git worktrees for other branches: separate checkouts whose code belongs to those
    // branches, not to this one. Linting them reports problems no commit here can fix.
    ignores: [
      "dist/**",
      "node_modules/**",
      "src-tauri/**",
      "electron-poc/**",
      ".vlx-worktrees/**",
      // The Capacitor mobile app is a separate sub-project under apps/mobile with its own
      // TypeScript/Vite pipeline. Its generated iOS and Android trees contain a vendored
      // native-bridge.js whose inline disable directives reference rules not configured here.
      "apps/**",
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
  {
    // Keep this separate from platform exemptions: IPC also runs over plain HTTP on the LAN.
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/genId.ts", "src/**/*.test.{ts,tsx}"],
    plugins: { vlx: vlxPlugin },
    rules: {
      "no-restricted-properties": ["error", {
        property: "randomUUID",
        message: "Use genId() from src/genId.ts; crypto.randomUUID() is unavailable over plain HTTP LAN access.",
      }],
      "vlx/no-relative-history-url": "error",
    },
  },
];

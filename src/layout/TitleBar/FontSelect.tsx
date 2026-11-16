import { useEffect, useState } from "react";
import Select from "../../components/Select";
import { useT } from "../../i18n";
import { platform } from "../../platform";
import type { FontCatalog } from "../../platform/types";

interface FontCatalogState {
  catalog: FontCatalog | null;
  loading: boolean;
}

/** Refresh when settings open or the app regains focus after a font installation. */
export function useFontCatalog(): FontCatalogState {
  const [state, setState] = useState<FontCatalogState>({ catalog: null, loading: true });
  useEffect(() => {
    let revision = 0;
    let alive = true;
    const refresh = async () => {
      const current = ++revision;
      setState({ catalog: null, loading: true });
      try {
        const catalog = await platform.fonts.catalog();
        if (alive && current === revision) setState({ catalog, loading: false });
      } catch {
        if (alive && current === revision) setState({ catalog: null, loading: false });
      }
    };
    void refresh();
    window.addEventListener("focus", refresh);
    return () => {
      alive = false;
      window.removeEventListener("focus", refresh);
    };
  }, []);
  return state;
}

// These are CSS keywords, not an installed-font catalog. Quoted keywords remain literal names.
const GENERIC_FAMILIES = new Set([
  "serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui",
  "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded", "emoji", "math", "fangsong",
]);

/** A successful catalog match is evidence of availability; a miss is only unknown.
 * Keep CSS fallback lists intact, including quoted family names containing commas. */
export function isKnownFontSelection(value: string | null, catalog: FontCatalog | null): boolean {
  if (!value) return true;
  const families = new Set(catalog?.families.map((family) => family.toLowerCase()) ?? []);
  let quote = "";
  let token = "";
  let quoted = false;
  let matched = false;
  const accept = () => {
    const name = token.trim().toLowerCase();
    if (families.has(name) || (!quoted && GENERIC_FAMILIES.has(name))) matched = true;
    token = "";
    quoted = false;
  };
  for (let i = 0; i < value.length; i++) {
    const char = value[i];
    // CSS hexadecimal escapes require full CSS parsing; report unknown rather than guess.
    if (char === "\\") return false;
    if (quote) {
      if (char === quote) quote = "";
      else token += char;
    } else if (char === '"' || char === "'") {
      quote = char;
      quoted = true;
    } else if (char === ",") {
      accept();
    } else {
      token += char;
    }
  }
  if (quote) return false;
  accept();
  return matched;
}

/** Only enumerated families are suggested. Typing filters them, and text matching no family becomes a row
 *  that commits the name as-is, so saved/custom stacks never need a separate editing mode. */
export function FontSelect({ value, onChange, label, fonts }: {
  value: string | null;
  onChange: (value: string | null) => void;
  label: string;
  fonts: FontCatalogState;
}) {
  const t = useT();
  const { catalog, loading } = fonts;
  const families = catalog?.families ?? [];
  const listed = value != null && families.includes(value);
  const options = [
    { value: "", label: t("settings.fontDefault") },
    ...(value != null && !listed ? [{ value, label: value, separatorBefore: true }] : []),
    ...families.map((family, index) => ({ value: family, label: family, separatorBefore: index === 0 })),
  ];
  const message = loading ? t("common.loading")
    : !catalog?.systemFontsAvailable ? t("settings.fontListUnavailable")
    : !isKnownFontSelection(value, catalog) ? t("settings.fontUnconfirmed")
    : null;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4, width: 160 }}>
      <Select
        value={value ?? ""}
        onChange={(next) => onChange(next || null)}
        options={options}
        size="sm"
        width={160}
        menuWidth={280}
        menuPortal
        align="right"
        searchable
        searchPlaceholder={t("settings.fontSearch")}
        customEntry={{
          label: (query) => t("settings.fontUseCustom", query),
          onSelect: (query) => onChange(query.trim() || null),
        }}
        ariaLabel={label}
      />
      {message && <div role="status" style={{ fontSize: 10.5, color: "var(--text-muted)", lineHeight: 1.5 }}>{message}</div>}
    </div>
  );
}

//! Shared settings components: Seg segmented control, Field settings row, and SectionTitle group heading.
//! Extracted from SettingsModal for reuse by the main settings page and category panels (settingsPanels / settingsBehaviorFields).

export function Seg<T extends string>({
  value,
  options,
  disabledOptions = [],
  onChange,
}: {
  value: T;
  options: [T, string][];
  disabledOptions?: T[];
  onChange: (v: T) => void;
}) {
  return (
    <div className="tb-seg" style={{ height: 26 }}>
      {options.map(([v, label]) => {
        const disabled = disabledOptions.includes(v);
        return (
          <button
            key={v}
            className={value === v ? "on" : ""}
            disabled={disabled}
            style={{
              padding: "0 12px",
              fontSize: 11.5,
              cursor: disabled ? "not-allowed" : undefined,
              opacity: disabled ? 0.45 : undefined,
            }}
            onClick={() => {
              if (!disabled) onChange(v);
            }}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}

/** Settings row with a fixed-width left label, right-aligned control, and thin bottom divider; roomier than the original popover on wide layouts. */
export function Field({
  label,
  children,
  className,
}: {
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={className}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 16,
        minHeight: 38,
        padding: "8px 0",
        borderBottom: "1px solid var(--border)",
      }}
    >
      <span
        style={{ width: 150, flex: "none", color: "var(--text-dim)", fontSize: 12.5 }}
      >
        {label}
      </span>
      <div style={{ flex: 1, display: "flex", justifyContent: "flex-end" }}>
        {children}
      </div>
    </div>
  );
}

/** Uppercase heading at the top of a category section, matching the original popover group title. */
export function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        fontSize: 10.5,
        letterSpacing: "1px",
        textTransform: "uppercase",
        color: "var(--text-dim)",
        fontWeight: 600,
        margin: "4px 0 10px",
      }}
    >
      {children}
    </div>
  );
}

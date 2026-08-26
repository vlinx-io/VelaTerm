//! Generic form modal that renders inputs from field definitions for creating groups/sessions, renaming, and similar actions.

import { useState } from "react";
import { normalizeArgDashes } from "../args";
import { useT } from "../i18n";
import { Backdrop } from "./Backdrop";
import Select from "./Select";

export interface FieldDef {
  key: string;
  label: string;
  placeholder?: string;
  required?: boolean;
  autoFocus?: boolean;
  /** When provided, render a select whose option value is submitted; an empty string is a valid Default option. */
  select?: { value: string; label: string }[];
  /** In select mode, append a Custom… option that reveals a text field; values outside the options enter this mode automatically. */
  allowCustom?: boolean;
  /** Field type: text input by default; "checkbox" renders a binary checkable option. */
  type?: "text" | "checkbox";
  /** Value written when a checkbox is checked (default "1"), mapping binary semantics to a concrete string such as "skip". */
  checkedValue?: string;
  /** Optional supporting text below a checkbox. */
  hint?: string;
  /** For text fields, restore `—`/`–` to `--` on submit so macOS Smart Punctuation cannot corrupt launch arguments. */
  normalizeDashes?: boolean;
}

/** Sentinel value for Custom… in a <select>; it cannot collide with a real shell path. */
const CUSTOM_SENTINEL = "\u0000__custom__";

/** Select field with options and an optional Custom… text-entry fallback. */
function SelectField({
  field,
  value,
  onChange,
}: {
  field: FieldDef;
  value: string;
  onChange: (v: string) => void;
}) {
  const t = useT();
  const opts = field.select ?? [];
  const known = opts.some((o) => o.value === value);
  // A non-empty initial value outside the options enters custom mode and populates the text field.
  const [custom, setCustom] = useState(field.allowCustom === true && !known && value !== "");

  return (
    <>
      <Select
        value={custom ? CUSTOM_SENTINEL : value}
        onChange={(v) => {
          if (v === CUSTOM_SENTINEL) {
            setCustom(true);
          } else {
            setCustom(false);
            onChange(v);
          }
        }}
        options={[
          ...opts,
          ...(field.allowCustom
            ? [{ value: CUSTOM_SENTINEL, label: t("form.customOption"), separatorBefore: true }]
            : []),
        ]}
        width="100%"
        ariaLabel={field.label}
      />
      {custom && (
        <input
          className="vlx-input"
          style={{ marginTop: 6 }}
          autoCapitalize="none"
          placeholder={field.placeholder}
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
      )}
    </>
  );
}

export function FormModal({
  title,
  fields,
  initial,
  submitLabel,
  onSubmit,
  onCancel,
}: {
  title: string;
  fields: FieldDef[];
  initial?: Record<string, string>;
  submitLabel?: string;
  onSubmit: (values: Record<string, string>) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const [values, setValues] = useState<Record<string, string>>(() => {
    const v: Record<string, string> = {};
    for (const f of fields) v[f.key] = initial?.[f.key] ?? "";
    return v;
  });

  const canSubmit = fields.every(
    (f) => !f.required || values[f.key].trim().length > 0,
  );

  const submit = () => {
    if (!canSubmit) return;
    // On submit, restore long dashes to `--` in fields marked normalizeDashes, such as launch arguments.
    const out: Record<string, string> = { ...values };
    for (const f of fields) {
      if (f.normalizeDashes) out[f.key] = normalizeArgDashes(out[f.key] ?? "");
    }
    onSubmit(out);
  };

  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 380,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) submit();
          if (e.key === "Escape") onCancel();
        }}
      >
        <div
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 14,
          }}
        >
          {title}
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {fields.map((f) => {
            if (f.type === "checkbox") {
              const on = f.checkedValue ?? "1";
              const checked = values[f.key] === on;
              return (
                <div key={f.key}>
                  <label
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      fontSize: 13,
                      color: "var(--text-primary)",
                      cursor: "pointer",
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={(e) =>
                        setValues((v) => ({
                          ...v,
                          [f.key]: e.target.checked ? on : "",
                        }))
                      }
                    />
                    {f.label}
                  </label>
                  {f.hint && (
                    <div
                      style={{
                        fontSize: 11,
                        color: "var(--text-muted)",
                        lineHeight: 1.5,
                        marginTop: 4,
                        marginLeft: 24,
                      }}
                    >
                      {f.hint}
                    </div>
                  )}
                </div>
              );
            }
            return (
              <label key={f.key} style={{ display: "block" }}>
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-muted)",
                    marginBottom: 4,
                  }}
                >
                  {f.label}
                  {f.required && (
                    <span style={{ color: "var(--status-error)" }}> *</span>
                  )}
                </div>
                {f.select ? (
                  <SelectField
                    field={f}
                    value={values[f.key]}
                    onChange={(v) => setValues((vs) => ({ ...vs, [f.key]: v }))}
                  />
                ) : (
                <input
                  className="vlx-input"
                  autoCapitalize="none"
                  placeholder={f.placeholder}
                  autoFocus={f.autoFocus}
                  value={values[f.key]}
                  onChange={(e) =>
                    setValues((v) => ({ ...v, [f.key]: e.target.value }))
                  }
                />
                )}
              </label>
            );
          })}
        </div>

        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
            marginTop: 18,
          }}
        >
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            onClick={submit}
            disabled={!canSubmit}
          >
            {submitLabel ?? t("common.confirm")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}

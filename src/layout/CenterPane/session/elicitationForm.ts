//! The form an MCP server asks a person to fill in, read from the JSON schema it sent.
//!
//! Elicitation schemas are deliberately flat: one object whose properties are strings, numbers,
//! booleans, or enumerations. Anything richer is shown as free text rather than refused, so a server
//! that stretches the contract still gets an answer.

export type ElicitationFieldKind = "text" | "number" | "integer" | "boolean" | "enum";

export interface ElicitationOption {
  value: string;
  label: string;
}

export interface ElicitationField {
  name: string;
  label: string;
  description?: string;
  kind: ElicitationFieldKind;
  required: boolean;
  options?: ElicitationOption[];
  /** What the field starts as, from the schema's `default`. */
  defaultValue?: string | boolean;
  /** A long-text hint the schema gives: `format: "textarea"` or a generous `maxLength`. */
  multiline?: boolean;
}

/** What a person typed or picked, keyed by field name. Booleans stay booleans; everything else is text. */
export type ElicitationValues = Record<string, string | boolean>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

/** The fields the schema asks for, in the order it lists them. An unusable schema yields no fields. */
export function parseElicitationFields(schema: unknown): ElicitationField[] {
  if (!isRecord(schema) || !isRecord(schema.properties)) return [];
  const required = new Set(
    Array.isArray(schema.required) ? schema.required.filter((n): n is string => typeof n === "string") : [],
  );
  return Object.entries(schema.properties).flatMap(([name, raw]) => {
    if (!isRecord(raw)) return [];
    const label = asString(raw.title) ?? name;
    const description = asString(raw.description);
    const field: ElicitationField = {
      name,
      label,
      description,
      kind: "text",
      required: required.has(name),
    };
    const options = enumOptions(raw);
    if (options) {
      field.kind = "enum";
      field.options = options;
    } else if (raw.type === "boolean") {
      field.kind = "boolean";
    } else if (raw.type === "integer") {
      field.kind = "integer";
    } else if (raw.type === "number") {
      field.kind = "number";
    } else {
      field.multiline =
        raw.format === "textarea" || (typeof raw.maxLength === "number" && raw.maxLength > 200);
    }
    if (typeof raw.default === "boolean" && field.kind === "boolean") field.defaultValue = raw.default;
    else if (typeof raw.default === "string" || typeof raw.default === "number") {
      field.defaultValue = String(raw.default);
    }
    return [field];
  });
}

function enumOptions(raw: Record<string, unknown>): ElicitationOption[] | undefined {
  if (Array.isArray(raw.enum)) {
    const names = Array.isArray(raw.enumNames) ? raw.enumNames : [];
    const options = raw.enum
      .filter((v): v is string | number | boolean => ["string", "number", "boolean"].includes(typeof v))
      .map((v, i) => ({ value: String(v), label: asString(names[i]) ?? String(v) }));
    return options.length > 0 ? options : undefined;
  }
  if (Array.isArray(raw.oneOf)) {
    const options = raw.oneOf.flatMap((entry) => {
      if (!isRecord(entry) || entry.const === undefined) return [];
      const value = String(entry.const);
      return [{ value, label: asString(entry.title) ?? value }];
    });
    return options.length > 0 ? options : undefined;
  }
  return undefined;
}

/** Where every field starts: the schema's defaults, and unchecked for a boolean with none. */
export function initialElicitationValues(fields: ElicitationField[]): ElicitationValues {
  const values: ElicitationValues = {};
  for (const field of fields) {
    if (field.kind === "boolean") values[field.name] = field.defaultValue === true;
    else if (typeof field.defaultValue === "string") values[field.name] = field.defaultValue;
  }
  return values;
}

function filled(field: ElicitationField, value: string | boolean | undefined): boolean {
  if (field.kind === "boolean") return true;
  return typeof value === "string" && value.trim() !== "";
}

/** Whether every required field has something in it and every number reads as one. */
export function isElicitationComplete(fields: ElicitationField[], values: ElicitationValues): boolean {
  return fields.every((field) => {
    const value = values[field.name];
    if (!filled(field, value)) return !field.required;
    if (field.kind === "number" || field.kind === "integer") {
      return numberOf(field, value) !== undefined;
    }
    return true;
  });
}

function numberOf(field: ElicitationField, value: string | boolean | undefined): number | undefined {
  if (typeof value !== "string") return undefined;
  const parsed = Number(value.trim());
  if (!Number.isFinite(parsed)) return undefined;
  if (field.kind === "integer" && !Number.isInteger(parsed)) return undefined;
  return parsed;
}

/** The `content` object the server receives: typed values, blank optional fields left out. */
export function buildElicitationContent(
  fields: ElicitationField[],
  values: ElicitationValues,
): Record<string, unknown> {
  const content: Record<string, unknown> = {};
  for (const field of fields) {
    const value = values[field.name];
    if (field.kind === "boolean") {
      content[field.name] = value === true;
      continue;
    }
    if (!filled(field, value)) continue;
    if (field.kind === "number" || field.kind === "integer") {
      const parsed = numberOf(field, value);
      if (parsed !== undefined) content[field.name] = parsed;
      continue;
    }
    content[field.name] = typeof value === "string" ? value : String(value);
  }
  return content;
}

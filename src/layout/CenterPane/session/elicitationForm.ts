//! The form an MCP server asks a person to fill in, read from the JSON schema it sent.
//!
//! Elicitation schemas are deliberately flat: one object whose properties are strings, numbers,
//! booleans, or enumerations. Anything richer is shown as free text rather than refused, so a server
//! that stretches the contract still gets an answer.

export type ElicitationFieldKind = "text" | "number" | "integer" | "boolean" | "enum" | "multiselect";

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
  defaultValue?: ElicitationValue;
  custom?: boolean;
  url?: string;
  when?: Array<{ key: string; op: "eq" | "neq"; value: unknown }>;
  /** A long-text hint the schema gives: `format: "textarea"` or a generous `maxLength`. */
  multiline?: boolean;
}

/** Input values keyed by field name; booleans and multiselect arrays retain their types. */
export type ElicitationValue = string | boolean | string[];
export type ElicitationValues = Record<string, ElicitationValue>;

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
  const entries = Object.entries(schema.properties);
  const explicitOrder = Array.isArray(schema.propertyOrder)
    ? schema.propertyOrder
    : Array.isArray(schema.propertyOrdering)
      ? schema.propertyOrdering
      : null;
  if (explicitOrder) {
    const order = new Map(explicitOrder.map((key, index) => [key, index]));
    entries.sort(([a], [b]) => (order.get(a) ?? entries.length) - (order.get(b) ?? entries.length));
  } else if (entries.every(([key]) => /^q\d+(?:_other)?$/.test(key))) {
    const rank = (key: string): [number, number] => {
      const m = /^q(\d+)(_other)?$/.exec(key)!;
      return [Number(m[1]), m[2] ? 1 : 0];
    };
    entries.sort(([a], [b]) => {
      const [na, oa] = rank(a);
      const [nb, ob] = rank(b);
      return na !== nb ? na - nb : oa - ob;
    });
  }
  return entries.flatMap(([name, raw]) => {
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
    const options = enumOptions(raw.type === "array" && isRecord(raw.items) ? raw.items : raw);
    if (raw.type === "array" && (options || raw.custom === true)) {
      field.kind = "multiselect";
      field.options = options;
      field.custom = raw.custom === true;
    } else if (options) {
      field.kind = "enum";
      field.options = options;
      field.custom = raw.custom === true;
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
    field.url = asString(raw.url);
    if (Array.isArray(raw.when)) {
      field.when = raw.when.flatMap((condition) => isRecord(condition) && typeof condition.key === "string" && (condition.op === "eq" || condition.op === "neq")
        ? [{ key: condition.key, op: condition.op, value: condition.value }] : []);
    }
    if (Array.isArray(raw.default) && field.kind === "multiselect") field.defaultValue = raw.default.filter((v): v is string => typeof v === "string");
    else if (typeof raw.default === "boolean" && field.kind === "boolean") field.defaultValue = raw.default;
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
  const union = Array.isArray(raw.oneOf) ? raw.oneOf : Array.isArray(raw.anyOf) ? raw.anyOf : null;
  if (union) {
    const options = union.flatMap((entry) => {
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
    if (field.kind === "multiselect") values[field.name] = Array.isArray(field.defaultValue) ? field.defaultValue : [];
    else if (field.kind === "boolean") values[field.name] = field.defaultValue === true;
    else if (typeof field.defaultValue === "string") values[field.name] = field.defaultValue;
  }
  return values;
}

function filled(field: ElicitationField, value: ElicitationValue | undefined): boolean {
  if (field.kind === "multiselect") return Array.isArray(value) && value.length > 0;
  if (field.kind === "boolean") return true;
  return typeof value === "string" && value.trim() !== "";
}

/** Whether every required field has something in it and every number reads as one. */
export function isElicitationComplete(fields: ElicitationField[], values: ElicitationValues): boolean {
  return fields.every((field) => {
    if (!isElicitationVisible(field, fields, values)) return true;
    const value = values[field.name];
    if (field.url) return value === true;
    if (!filled(field, value)) return !field.required;
    if (field.kind === "number" || field.kind === "integer") {
      return numberOf(field, value) !== undefined;
    }
    return true;
  });
}

function numberOf(field: ElicitationField, value: ElicitationValue | undefined): number | undefined {
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
    if (!isElicitationVisible(field, fields, values)) continue;
    const value = values[field.name];
    if (field.kind === "multiselect") {
      if (Array.isArray(value)) content[field.name] = value;
      continue;
    }
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

/** Conditional native fields use typed values and omit hidden answers from submissions. */
export function isElicitationVisible(field: ElicitationField, fields: ElicitationField[], values: ElicitationValues): boolean {
  const visible = (candidate: ElicitationField, ancestors: Set<string>): boolean => {
    if (ancestors.has(candidate.name)) return false;
    const visited = new Set(ancestors).add(candidate.name);
    return (candidate.when ?? []).every((condition) => {
      const source = fields.find((entry) => entry.name === condition.key);
      const raw = source && !visible(source, visited) ? undefined : values[condition.key];
      const value = source && (source.kind === "number" || source.kind === "integer") ? numberOf(source, raw) : raw;
      const equal = Array.isArray(value) ? value.includes(String(condition.value)) : value === condition.value;
      return condition.op === "neq" ? !equal : equal;
    });
  };
  return visible(field, new Set());
}

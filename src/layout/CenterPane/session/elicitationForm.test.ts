import { describe, expect, it } from "vitest";

import {
  buildElicitationContent,
  initialElicitationValues,
  isElicitationComplete,
  parseElicitationFields,
  isElicitationVisible,
} from "./elicitationForm";

const schema = {
  type: "object",
  properties: {
    name: { type: "string", title: "Project name", description: "Shown in the dashboard" },
    size: { type: "integer", default: 3 },
    ratio: { type: "number" },
    region: { type: "string", enum: ["eu", "us"], enumNames: ["Europe", "United States"] },
    tier: { oneOf: [{ const: "a", title: "Tier A" }, { const: "b" }] },
    agree: { type: "boolean", default: true },
    notes: { type: "string", format: "textarea" },
  },
  required: ["name", "region"],
};

describe("elicitation form", () => {
  it("reads every flat field kind the protocol allows, in schema order", () => {
    const fields = parseElicitationFields(schema);
    expect(fields.map((f) => [f.name, f.kind, f.required])).toEqual([
      ["name", "text", true],
      ["size", "integer", false],
      ["ratio", "number", false],
      ["region", "enum", true],
      ["tier", "enum", false],
      ["agree", "boolean", false],
      ["notes", "text", false],
    ]);
    expect(fields[0].label).toBe("Project name");
    expect(fields[3].options).toEqual([
      { value: "eu", label: "Europe" },
      { value: "us", label: "United States" },
    ]);
    expect(fields[4].options).toEqual([
      { value: "a", label: "Tier A" },
      { value: "b", label: "b" },
    ]);
    expect(fields[6].multiline).toBe(true);
    expect(initialElicitationValues(fields)).toEqual({ size: "3", agree: true });
  });

  it("is complete only when required fields are filled and numbers read as numbers", () => {
    const fields = parseElicitationFields(schema);
    const values = initialElicitationValues(fields);
    expect(isElicitationComplete(fields, values)).toBe(false);
    values.name = "vela";
    values.region = "eu";
    expect(isElicitationComplete(fields, values)).toBe(true);
    values.size = "2.5";
    expect(isElicitationComplete(fields, values)).toBe(false);
    values.size = "";
    expect(isElicitationComplete(fields, values)).toBe(true);
  });

  it("sends typed values and leaves blank optional fields out", () => {
    const fields = parseElicitationFields(schema);
    const content = buildElicitationContent(fields, {
      name: " vela ",
      size: "4",
      ratio: "0.5",
      region: "us",
      agree: false,
      notes: "",
    });
    expect(content).toEqual({ name: " vela ", size: 4, ratio: 0.5, region: "us", agree: false });
  });

  it("yields no fields for a schema it cannot read", () => {
    expect(parseElicitationFields(null)).toEqual([]);
    expect(parseElicitationFields({ type: "string" })).toEqual([]);
  });

  it("keeps arrays without choices editable and supports custom multiselect values", () => {
    const fields = parseElicitationFields({ properties: {
      unknown: { type: "array", items: { type: "object" } },
      custom: { type: "array", custom: true, items: { type: "string" } },
    }, required: ["unknown", "custom"] });
    expect(fields.map((field) => field.kind)).toEqual(["text", "multiselect"]);
    const values = { unknown: '[{"name":"sample"}]', custom: ["free choice"] };
    expect(isElicitationComplete(fields, values)).toBe(true);
    expect(buildElicitationContent(fields, values)).toEqual(values);
  });

  it("preserves v2 option values, multiselect defaults and conditional typed answers", () => {
    const fields = parseElicitationFields({ properties: {
      region: { type: "string", oneOf: [{ const: "eu", title: "Europe" }] },
      tags: { type: "array", default: ["one"], items: { oneOf: [{ const: "one", title: "First" }] } },
      count: { type: "integer", default: 2 },
      reason: { type: "string", when: [{ key: "count", op: "eq", value: 2 }] },
    }, required: ["reason"] });
    const values = initialElicitationValues(fields);
    expect(values.tags).toEqual(["one"]);
    expect(isElicitationVisible(fields[3], fields, values)).toBe(true);
    expect(isElicitationComplete(fields, values)).toBe(false);
    values.count = "3";
    values.region = "eu";
    values.reason = "hidden stale input";
    expect(isElicitationComplete(fields, values)).toBe(true);
    expect(buildElicitationContent(fields, values)).toEqual({ tags: ["one"], count: 3, region: "eu" });
  });

  it("requires an external confirmation before submitting", () => {
    const fields = parseElicitationFields({ properties: { confirmed: { type: "boolean", url: "https://example.com" } } });
    const values = initialElicitationValues(fields);
    expect(isElicitationComplete(fields, values)).toBe(false);
    values.confirmed = true;
    expect(isElicitationComplete(fields, values)).toBe(true);
    expect(buildElicitationContent(fields, values)).toEqual({ confirmed: true });
  });

  it("keeps native field order and omits answers behind hidden dependencies", () => {
    const fields = parseElicitationFields({ properties: {
      detail: { type: "string", when: [{ key: "count", op: "eq", value: 2 }] },
      count: { type: "integer", default: 2, when: [{ key: "enabled", op: "eq", value: true }] },
      enabled: { type: "boolean", default: false },
    }, propertyOrder: ["enabled", "count", "detail"], required: ["detail"] });
    const values: ReturnType<typeof initialElicitationValues> = { ...initialElicitationValues(fields), detail: "hidden answer" };
    expect(fields.map((field) => field.name)).toEqual(["enabled", "count", "detail"]);
    expect(isElicitationVisible(fields[2], fields, values)).toBe(false);
    expect(isElicitationComplete(fields, values)).toBe(true);
    expect(buildElicitationContent(fields, values)).toEqual({ enabled: false });
    values.enabled = true;
    expect(buildElicitationContent(fields, values)).toEqual({ enabled: true, count: 2, detail: "hidden answer" });
  });

  it("supports anyOf multiselect items, propertyOrdering, and numeric qN sorting", () => {
    const unordered = parseElicitationFields({
      type: "object",
      properties: {
        q10: { type: "string", title: "Tenth" },
        q2: {
          type: "array",
          title: "Pick many",
          items: {
            anyOf: [
              { const: "1", title: "Alpha" },
              { const: "2", title: "Beta" },
            ],
          },
        },
        q2_other: { type: "string", title: "Other" },
        q1: {
          type: "string",
          title: "First",
          oneOf: [{ const: "1", title: "Only" }],
        },
      },
    });
    expect(unordered.map((f) => [f.name, f.kind])).toEqual([
      ["q1", "enum"],
      ["q2", "multiselect"],
      ["q2_other", "text"],
      ["q10", "text"],
    ]);
    expect(unordered[1].options).toEqual([
      { value: "1", label: "Alpha" },
      { value: "2", label: "Beta" },
    ]);

    const ordered = parseElicitationFields({
      type: "object",
      propertyOrdering: ["second", "first"],
      properties: {
        first: { type: "string" },
        second: { type: "string" },
      },
    });
    expect(ordered.map((f) => f.name)).toEqual(["second", "first"]);
  });
});

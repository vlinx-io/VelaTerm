import { describe, expect, it } from "vitest";

import {
  buildElicitationContent,
  initialElicitationValues,
  isElicitationComplete,
  parseElicitationFields,
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
});

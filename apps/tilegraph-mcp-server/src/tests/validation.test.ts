import { describe, it, expect } from "vitest";
import {
  TagSchema,
  ObjectIdSchema,
  ObjectIdArraySchema,
  RadiusSchema,
  DirectionSchema,
} from "../schemas/validation.js";

describe("TagSchema", () => {
  it("accepts valid engineering tags", () => {
    expect(TagSchema.parse("P-1001")).toBe("P-1001");
    expect(TagSchema.parse("LINE-1001")).toBe("LINE-1001");
    expect(TagSchema.parse("FT_101.A")).toBe("FT_101.A");
    expect(TagSchema.parse("10")).toBe("10");
  });

  it("rejects tags with spaces", () => {
    expect(() => TagSchema.parse("LINE 1001")).toThrow();
  });

  it("rejects empty string", () => {
    expect(() => TagSchema.parse("")).toThrow();
  });

  it("rejects tags longer than 64 chars", () => {
    expect(() => TagSchema.parse("A".repeat(65))).toThrow();
  });

  it("rejects tags with special chars like @", () => {
    expect(() => TagSchema.parse("P@1001")).toThrow();
  });
});

describe("ObjectIdSchema", () => {
  it("accepts valid object_id format", () => {
    const id = "obj_" + "a".repeat(32);
    expect(ObjectIdSchema.parse(id)).toBe(id);
  });

  it("accepts mixed-case hex", () => {
    const id = "obj_" + "abcdef0123456789abcdef0123456789";
    expect(ObjectIdSchema.parse(id)).toBe(id);
  });

  it("rejects missing obj_ prefix", () => {
    expect(() => ObjectIdSchema.parse("abcdef0123456789abcdef0123456789")).toThrow();
  });

  it("rejects wrong hex length (short)", () => {
    expect(() => ObjectIdSchema.parse("obj_abc")).toThrow();
  });

  it("rejects wrong hex length (too long)", () => {
    expect(() => ObjectIdSchema.parse("obj_" + "a".repeat(33))).toThrow();
  });

  it("rejects non-hex chars in id part", () => {
    expect(() => ObjectIdSchema.parse("obj_" + "g".repeat(32))).toThrow();
  });
});

describe("ObjectIdArraySchema", () => {
  const validId = "obj_" + "a".repeat(32);

  it("accepts an array of valid ids", () => {
    expect(ObjectIdArraySchema.parse([validId])).toEqual([validId]);
  });

  it("rejects empty array", () => {
    expect(() => ObjectIdArraySchema.parse([])).toThrow();
  });

  it("rejects arrays exceeding 50 items", () => {
    expect(() => ObjectIdArraySchema.parse(Array(51).fill(validId))).toThrow();
  });

  it("rejects array with invalid id", () => {
    expect(() => ObjectIdArraySchema.parse(["not-an-id"])).toThrow();
  });
});

describe("RadiusSchema", () => {
  it("accepts positive radius with default", () => {
    expect(RadiusSchema.parse(10)).toBe(10);
    expect(RadiusSchema.parse(undefined)).toBe(5.0);
  });

  it("rejects zero radius", () => {
    expect(() => RadiusSchema.parse(0)).toThrow();
  });

  it("rejects negative radius", () => {
    expect(() => RadiusSchema.parse(-1)).toThrow();
  });

  it("rejects radius above 500m", () => {
    expect(() => RadiusSchema.parse(501)).toThrow();
  });
});

describe("DirectionSchema", () => {
  it("accepts valid directions", () => {
    expect(DirectionSchema.parse("upstream")).toBe("upstream");
    expect(DirectionSchema.parse("downstream")).toBe("downstream");
    expect(DirectionSchema.parse("both")).toBe("both");
    expect(DirectionSchema.parse(undefined)).toBe("both");
  });

  it("rejects invalid direction", () => {
    expect(() => DirectionSchema.parse("sideways")).toThrow();
  });
});

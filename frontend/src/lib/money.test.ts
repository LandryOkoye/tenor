import { formatPrice, formatUsdc, fromBaseUnits, toBaseUnits } from "./money";

describe("toBaseUnits", () => {
  it("converts human USDC to 6-decimal base units without floats", () => {
    expect(toBaseUnits("20")).toBe("20000000");
    expect(toBaseUnits("20.5")).toBe("20500000");
    expect(toBaseUnits("0.000001")).toBe("1");
    expect(toBaseUnits("0.1")).toBe("100000");
  });
  it("rejects invalid, zero and over-precise input", () => {
    expect(toBaseUnits("")).toBeNull();
    expect(toBaseUnits("0")).toBeNull();
    expect(toBaseUnits("1.0000001")).toBeNull();
    expect(toBaseUnits("-5")).toBeNull();
    expect(toBaseUnits("abc")).toBeNull();
  });
});

describe("fromBaseUnits / formatUsdc", () => {
  it("round-trips and formats", () => {
    expect(fromBaseUnits("20500000")).toBe(20.5);
    expect(formatUsdc("20500000")).toBe("$20.50");
    expect(formatUsdc("-800000", { sign: true })).toBe("−$0.80");
    expect(formatUsdc("6840000", { sign: true })).toBe("+$6.84");
  });
});

describe("formatPrice", () => {
  it("shows cents and implied chance", () => {
    expect(formatPrice("0.62")).toMatchObject({ cents: "62¢", pct: "62%" });
    expect(formatPrice("0.055")).toMatchObject({ cents: "5.5¢", pct: "6%" });
    expect(formatPrice(null).cents).toBe("–");
  });
});

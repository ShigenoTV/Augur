import { describe, expect, it } from "vitest";
import { formatAge, formatDuration, formatPrice, formatSignedNumber, formatSignedPercent } from "./format";

const nnbsp = " ";

describe("format", () => {
  it("pourcentages signés à la française", () => {
    expect(formatSignedPercent(0.124)).toBe(`+12,4${nnbsp}%`);
    expect(formatSignedPercent(-0.03)).toBe(`−3,0${nnbsp}%`);
    expect(formatSignedPercent(0.0001)).toBe(`±0,0${nnbsp}%`);
  });

  it("nombres signés", () => {
    expect(formatSignedNumber(1234)).toBe(`+1${nnbsp}234`);
    expect(formatSignedNumber(-56.4)).toBe("−56");
  });

  it("prix", () => {
    expect(formatPrice(14, "exalted")).toBe(`14${nnbsp}ex`);
    expect(formatPrice(1.5, "divine")).toBe(`1,5${nnbsp}div`);
  });

  it("âge d'un prix", () => {
    const now = new Date("2026-10-31T15:00:00Z");
    expect(formatAge("2026-10-31T14:59:40Z", now)).toBe("à l'instant");
    expect(formatAge("2026-10-31T14:47:00Z", now)).toBe(`il y a 13${nnbsp}min`);
    expect(formatAge("2026-10-31T12:00:00Z", now)).toBe(`il y a 3${nnbsp}h`);
  });

  it("durées", () => {
    expect(formatDuration(3_200)).toBe(`4${nnbsp}s`);
    expect(formatDuration(65_000)).toBe(`1${nnbsp}min${nnbsp}05`);
  });
});

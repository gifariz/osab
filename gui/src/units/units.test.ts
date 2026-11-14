import { describe, expect, it } from "vitest";

import { QUANTITIES, UNIT_SYSTEMS, fromDisplay, toDisplay, unitLabel, unitOf, unitSystemOf } from "./units";

const close = (a: number, b: number) => expect(Math.abs(a - b)).toBeLessThanOrEqual(1e-12 * Math.max(1, Math.abs(b)));

describe("display units", () => {
  it("every system has a unit for every quantity, and its values convert back exactly", () => {
    for (const s of UNIT_SYSTEMS) {
      for (const q of QUANTITIES) {
        const unit = unitOf(q, s);
        expect(unit.label, `${s} ${q}`).toBeTruthy();
        expect(unit.factor, `${s} ${q}`).toBeGreaterThan(0);
        for (const v of [0, 1, -2.5, 1234.5678, 3e-7]) close(fromDisplay(toDisplay(v, q, s), q, s), v);
      }
    }
  });

  it("SI shows the base units where they agree and the usual SI units elsewhere", () => {
    close(toDisplay(6, "distance", "SI"), 6);
    close(toDisplay(0.3, "sectionLength", "SI"), 300);
    close(toDisplay(0.09, "sectionArea", "SI"), 900); // 0.3 x 0.3 m = 900 cm²
    close(toDisplay(6.75e-4, "length4", "SI"), 67500); // 0.3⁴ / 12 m⁴ = 67,500 cm⁴
    close(toDisplay(0.0123, "displacement", "SI"), 12.3);
    close(toDisplay(25e6, "modulus", "SI"), 25000); // 25,000,000 kN/m² = 25,000 MPa
    close(toDisplay(2, "mass", "SI"), 2000); // 2 t = 2000 kg
    close(toDisplay(1.5, "weightPerLength", "SI"), 1500); // kN/m to N/m
    expect(unitLabel("moment", "SI")).toBe("kN·m");
    expect(unitLabel("forcePerArea", "SI")).toBe("kN/m²");
  });

  it("MKS: kgf = 9.80665 N, tonf = 1000 kgf", () => {
    close(fromDisplay(1, "force", "MKS"), 9.80665);
    close(fromDisplay(1, "forcePerArea", "MKS"), 9.80665e-3);
    close(fromDisplay(1, "modulus", "MKS"), 9806.65); // 1 kgf/mm² in kN/m²
    close(fromDisplay(1, "transStiffness", "MKS"), 9806.65); // 1 tonf/mm in kN/m
    close(fromDisplay(1, "rotationalInertia", "MKS"), 9.80665); // 1 tonf·m·s² in t·m²
    close(toDisplay(0.6, "sectionArea", "MKS"), 600000); // mm²
    expect(unitLabel("moment", "MKS")).toBe("tonf·m");
  });

  it("US: ft = 0.3048 m, in = 0.0254 m, lb = 4.4482216152605 N, kip = 1000 lb", () => {
    close(fromDisplay(1, "distance", "US"), 0.3048);
    close(fromDisplay(1, "sectionLength", "US"), 0.0254);
    close(fromDisplay(1, "force", "US"), 4.4482216152605);
    close(fromDisplay(1, "moment", "US"), 1.3558179483314004); // kip·ft
    close(fromDisplay(1, "forcePerLength", "US"), 14.593902937206364); // kip/ft
    close(fromDisplay(1, "forcePerArea", "US"), 0.04788025898033584); // psf
    close(fromDisplay(1, "modulus", "US"), 6.894757293168361); // psi
    close(fromDisplay(1, "weightPerVolume", "US"), 0.15708746384624552); // pcf
    close(fromDisplay(1, "mass", "US"), 0.014593902937206364); // slug in t
    close(fromDisplay(1, "transStiffness", "US"), 175.12683524647638); // kip/in
    close(fromDisplay(1, "length4", "US"), 4.162314256e-7); // in⁴
  });

  it("temperatures: °F has an offset, a temperature change does not", () => {
    close(fromDisplay(32, "temperature", "US"), 0);
    close(fromDisplay(212, "temperature", "US"), 100);
    close(toDisplay(-40, "temperature", "US"), -40);
    close(fromDisplay(18, "temperatureChange", "US"), 10);
    close(fromDisplay(1, "thermalCoefficient", "US"), 1.8);
  });

  it("a model's saved system falls back to SI", () => {
    expect(unitSystemOf("US")).toBe("US");
    expect(unitSystemOf("MKS")).toBe("MKS");
    expect(unitSystemOf(undefined)).toBe("SI");
    expect(unitSystemOf("kN-m")).toBe("SI");
  });
});

import * as THREE from "three";
import { beforeAll, describe, expect, it } from "vitest";

import { RAMP_STOPS, rampColor } from "../../scene/areas";
import { MaterialPool, type Shared } from "./materials";

// The symbols' textures are drawn on a canvas: a stand-in that takes any drawing call.
beforeAll(() => {
  const g = new Proxy({}, { get: () => () => {} });
  (globalThis as { document?: unknown }).document ??= { createElement: () => ({ getContext: () => g }) };
});

const shared = (): Shared => ({ view: { value: new THREE.Vector2(800, 600) }, swing: { value: 1 }, dashSize: { value: 1 }, gapSize: { value: 1 } });

/** The shader text a built-in material starts from, after its `onBeforeCompile`. */
function compiled(m: THREE.Material, lib: keyof typeof THREE.ShaderLib) {
  const shader = { vertexShader: THREE.ShaderLib[lib].vertexShader, fragmentShader: THREE.ShaderLib[lib].fragmentShader, uniforms: {} as Record<string, unknown> };
  m.onBeforeCompile(shader as unknown as THREE.WebGLProgramParametersWithUniforms, null as unknown as THREE.WebGLRenderer);
  return shader;
}

describe("viewport materials", () => {
  it("add their parts to the installed three.js shaders", () => {
    const pool = new MaterialPool(shared());
    // Lines: dashes per line, and the displacement of each end.
    const line = pool.line(3, 1, true);
    expect(line.vertexShader).toContain("vDash = instanceDash;");
    expect(line.vertexShader).toContain("instanceStart + ( swing - 1.0 ) * instanceMotionStart");
    expect(line.fragmentShader).toContain("vDash > 0.5 && mod(");
    expect(line.uniforms.resolution).toBe(pool.shared.view);
    expect(pool.line(3, 1, true)).toBe(line);
    // Dots: a size per dot.
    expect(compiled(pool.dots(true), "points").vertexShader).toContain("gl_PointSize = size * psize;");
    // Symbols: moved off the joint.
    expect(compiled(pool.glyph("fixed", "#ff0000", 26), "points").vertexShader).toContain("gl_Position.xy += anchor");
    // Faces: displacements, and contours from the corner values.
    const faces = compiled(pool.faces({ shaded: false, transparent: false, contour: true, motion: true }), "basic");
    expect(faces.vertexShader).toContain("transformed += ( swing - 1.0 ) * motion;");
    expect(faces.fragmentShader).toContain("diffuseColor.rgb = owRamp( ct );");
    expect(compiled(pool.faces({ shaded: true, transparent: true, contour: false, motion: true }), "lambert").vertexShader).toContain("motion");
    pool.dispose();
  });

  it("colour contours with the ramp's own stops", () => {
    const faces = compiled(new MaterialPool(shared()).faces({ shaded: false, transparent: false, contour: true, motion: false }), "basic");
    for (const stop of RAMP_STOPS) expect(faces.fragmentShader).toContain(`vec3( ${stop.map((x) => x.toFixed(6)).join(", ")} )`);
    expect(rampColor(0)).toEqual([...RAMP_STOPS[0]]);
    expect(rampColor(1)).toEqual([...RAMP_STOPS[RAMP_STOPS.length - 1]]);
  });
});

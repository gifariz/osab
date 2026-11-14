// The viewport's materials: the usual three.js ones with small additions to their shaders, so that merged
// buffers can carry what separate objects had as material settings: a dashed flag per line, a size per joint
// dot, a displacement per vertex for mode animation (moved by the `swing` uniform), and contour colours worked
// out per pixel. Materials depend only on these settings, so a viewport keeps one of each kind and reuses it
// for every scene: their shader programs are made once.

import * as THREE from "three";
import { LineMaterial } from "three/examples/jsm/lines/LineMaterial.js";

import { RAMP_STOPS } from "../../scene/areas";
import { dotTexture, glyphTexture } from "./icons";
import type { SceneGlyph } from "../../scene/types";

/** Uniforms every material of a viewport shares. */
export interface Shared {
  /** The view's size in CSS pixels (line widths, symbol offsets). */
  view: { value: THREE.Vector2 };
  /** Mode animation: drawn positions are position + (swing - 1) * displacement. */
  swing: { value: number };
  dashSize: { value: number };
  gapSize: { value: number };
}

/** Replaces `find` in a shader (it must be there: a newer three.js with other shader text fails loudly). */
function edit(code: string, find: string, replace: string): string {
  if (!code.includes(find)) throw new Error(`shader text not found: ${find}`);
  return code.replace(find, replace);
}

/**
 * Wide lines with a colour per line, dashes per line (`instanceDash` 1) and, with `motion`, a displacement per
 * end (`instanceMotionStart`, `instanceMotionEnd`).
 */
export function lineMaterial(shared: Shared, width: number, opacity: number, motion: boolean): LineMaterial {
  const m = new LineMaterial({ linewidth: width, vertexColors: true, dashed: true, transparent: opacity < 1, opacity });
  m.uniforms.resolution = shared.view;
  m.uniforms.dashSize = shared.dashSize;
  m.uniforms.gapSize = shared.gapSize;
  m.uniforms.swing = shared.swing;
  let vs = edit(m.vertexShader, "attribute vec3 instanceStart;", "attribute vec3 instanceStart;\n\t\tattribute float instanceDash;\n\t\tvarying float vDash;");
  vs = edit(vs, "void main() {", "void main() {\n\t\t\tvDash = instanceDash;");
  if (motion) {
    vs = edit(vs, "attribute vec3 instanceEnd;", "attribute vec3 instanceEnd;\n\t\tattribute vec3 instanceMotionStart;\n\t\tattribute vec3 instanceMotionEnd;\n\t\tuniform float swing;");
    vs = edit(vs, "vec4( instanceStart, 1.0 )", "vec4( instanceStart + ( swing - 1.0 ) * instanceMotionStart, 1.0 )");
    vs = edit(vs, "vec4( instanceEnd, 1.0 )", "vec4( instanceEnd + ( swing - 1.0 ) * instanceMotionEnd, 1.0 )");
  }
  let fs = "varying float vDash;\n" + m.fragmentShader;
  fs = edit(fs, "if ( vUv.y < - 1.0 || vUv.y > 1.0 ) discard;", "if ( vDash > 0.5 && ( vUv.y < - 1.0 || vUv.y > 1.0 ) ) discard;");
  fs = edit(fs, "if ( mod( vLineDistance + dashOffset, dashSize + gapSize ) > dashSize ) discard;", "if ( vDash > 0.5 && mod( vLineDistance + dashOffset, dashSize + gapSize ) > dashSize ) discard;");
  m.vertexShader = vs;
  m.fragmentShader = fs;
  return m;
}

/** Adds a displacement attribute `motion` moved by `swing` to a built-in material's vertex shader. */
function withMotion(shader: THREE.WebGLProgramParametersWithUniforms, shared: Shared) {
  shader.uniforms.swing = shared.swing;
  shader.vertexShader = edit(shader.vertexShader, "#include <common>", "#include <common>\nattribute vec3 motion;\nuniform float swing;");
  shader.vertexShader = edit(shader.vertexShader, "#include <begin_vertex>", "#include <begin_vertex>\n\ttransformed += ( swing - 1.0 ) * motion;");
}

/** How far joint dots and symbols are pulled towards the camera, in pixels at their depth. */
const PULL_PX = 8;

/**
 * GLSL after `#include <project_vertex>` (with `uniform vec2 view`): the point pulled `PULL_PX` pixels' worth towards
 * the camera along its line of sight, so it stays at the same place on screen: the lines that meet at a joint do not
 * hide its dot or symbol, while objects further in front still do.
 */
const PULL = `
\tfloat owPull = ${PULL_PX.toFixed(1)} * 2.0 / ( projectionMatrix[ 1 ][ 1 ] * view.y );
\tif ( isPerspectiveMatrix( projectionMatrix ) ) mvPosition.xyz *= 1.0 - owPull;
\telse mvPosition.z += owPull;
\tgl_Position = projectionMatrix * mvPosition;`;

/**
 * Round joint dots with a colour and a size (`psize`, CSS pixels; 0: not drawn) per dot: hidden by solid objects in
 * front of them and dimmed by see-through faces, or (`onTop`, the hover highlight) drawn over everything.
 */
export function dotMaterial(shared: Shared, motion: boolean, onTop = false): THREE.PointsMaterial {
  const m = new THREE.PointsMaterial({ vertexColors: true, size: 1, sizeAttenuation: false, map: dotTexture(), transparent: true, alphaTest: 0.4, depthTest: !onTop });
  m.onBeforeCompile = (shader) => {
    shader.uniforms.view = shared.view;
    shader.vertexShader = edit(shader.vertexShader, "#include <common>", "#include <common>\nattribute float psize;\nuniform vec2 view;");
    shader.vertexShader = edit(shader.vertexShader, "#include <project_vertex>", `#include <project_vertex>${PULL}`);
    // `size` is 1 times the pixel ratio: the dot's size in device pixels.
    shader.vertexShader = edit(shader.vertexShader, "gl_PointSize = size;", "gl_PointSize = size * psize;\n\tif ( psize <= 0.0 ) gl_Position = vec4( 0.0, 0.0, 2.0, 1.0 );");
    if (motion) withMotion(shader, shared);
  };
  m.customProgramCacheKey = () => `osab-dots-${motion}`;
  return m;
}

/**
 * Support and spring symbols: fixed-size squares hanging below the joint (springs: to its left), at the joint's
 * depth: hidden by solid objects in front of it and dimmed by see-through faces. Their clear parts are left out
 * (`alphaTest`), so they hide nothing behind them.
 */
export function glyphMaterial(shared: Shared, kind: SceneGlyph["kind"], color: string, px: number): THREE.PointsMaterial {
  const m = new THREE.PointsMaterial({ size: px, sizeAttenuation: false, map: glyphTexture(kind, color), transparent: true, alphaTest: 0.1 });
  // The symbol's middle, in CSS pixels from the joint (screen y up).
  const anchor = kind === "spring" ? new THREE.Vector2(-px / 2, 0) : new THREE.Vector2(0, -px / 2);
  m.onBeforeCompile = (shader) => {
    shader.uniforms.anchor = { value: anchor };
    shader.uniforms.view = shared.view;
    shader.vertexShader = edit(shader.vertexShader, "#include <common>", "#include <common>\nuniform vec2 anchor;\nuniform vec2 view;");
    shader.vertexShader = edit(shader.vertexShader, "#include <project_vertex>", `#include <project_vertex>${PULL}\n\tgl_Position.xy += anchor * 2.0 / view * gl_Position.w;`);
  };
  m.customProgramCacheKey = () => "osab-glyph";
  return m;
}

/** GLSL of the contour colour ramp (the stops of `rampColor`). */
function rampGlsl(): string {
  const v = (c: readonly number[]) => `vec3( ${c.map((x) => x.toFixed(6)).join(", ")} )`;
  const n = RAMP_STOPS.length;
  const pick = (offset: number) =>
    RAMP_STOPS.slice(offset, offset + n - 1)
      .map((c, i) => (i < n - 2 ? `i < ${i}.5 ? ${v(c)} : ` : v(c)))
      .join("");
  return `vec3 owRamp( float t ) {
\tfloat x = clamp( t, 0.0, 1.0 ) * ${(n - 1).toFixed(1)};
\tfloat i = min( floor( x ), ${(n - 2).toFixed(1)} );
\treturn mix( ${pick(0)}, ${pick(1)}, x - i );
}`;
}

export interface FaceKind {
  shaded: boolean;
  transparent: boolean;
  /** Contours coloured per pixel (`cuv`, `cramp`, `ctri` attributes). */
  contour: boolean;
  motion: boolean;
}

/** Faces with an RGBA colour per vertex (or contour colours), drawn on both sides in one pass. */
export function faceMaterial(shared: Shared, k: FaceKind): THREE.Material {
  const common = {
    vertexColors: true,
    transparent: k.transparent,
    side: THREE.DoubleSide,
    depthWrite: !k.transparent,
    // Faces are sorted back to front object by object; one pass keeps that order.
    forceSinglePass: true,
    // Keep lines drawn on the faces (edges, outlines) visible.
    polygonOffset: true,
    polygonOffsetFactor: 1,
    polygonOffsetUnits: 1,
  };
  const m = k.shaded ? new THREE.MeshLambertMaterial(common) : new THREE.MeshBasicMaterial(common);
  if (k.contour || k.motion) {
    m.onBeforeCompile = (shader) => {
      if (k.motion) withMotion(shader, shared);
      if (k.contour) {
        shader.vertexShader = edit(shader.vertexShader, "#include <common>", "#include <common>\nattribute vec2 cuv;\nattribute vec4 cramp;\nattribute float ctri;\nvarying vec2 vCuv;\nvarying vec4 vCramp;\nvarying float vCtri;");
        shader.vertexShader = edit(shader.vertexShader, "#include <begin_vertex>", "#include <begin_vertex>\n\tvCuv = cuv;\n\tvCramp = cramp;\n\tvCtri = ctri;");
        shader.fragmentShader = edit(shader.fragmentShader, "#include <common>", `#include <common>\nvarying vec2 vCuv;\nvarying vec4 vCramp;\nvarying float vCtri;\n${rampGlsl()}`);
        // The element's field: linear over a triangle, bilinear over a quad, from the corners' ramp positions.
        shader.fragmentShader = edit(
          shader.fragmentShader,
          "#include <color_fragment>",
          `#include <color_fragment>
\tfloat cu = vCuv.x;
\tfloat cv = vCuv.y;
\tfloat ct = vCtri > 0.5
\t\t? ( 1.0 - cu - cv ) * vCramp.x + cu * vCramp.y + cv * vCramp.z
\t\t: ( 1.0 - cu ) * ( 1.0 - cv ) * vCramp.x + cu * ( 1.0 - cv ) * vCramp.y + cu * cv * vCramp.z + ( 1.0 - cu ) * cv * vCramp.w;
\tdiffuseColor.rgb = owRamp( ct );`,
        );
      }
    };
  }
  m.customProgramCacheKey = () => `osab-faces-${k.shaded}-${k.contour}-${k.motion}`;
  return m;
}

/** Load and reaction arrow heads: one cone, drawn many times with a colour per copy. */
export function coneMaterial(): THREE.MeshBasicMaterial {
  return new THREE.MeshBasicMaterial({ color: "#ffffff" });
}

/** Materials of a viewport, made once per kind and reused by every scene. */
export class MaterialPool {
  private made = new Map<string, THREE.Material>();
  constructor(readonly shared: Shared) {}

  private get<T extends THREE.Material>(key: string, make: () => T): T {
    let m = this.made.get(key) as T | undefined;
    if (!m) this.made.set(key, (m = make()));
    return m;
  }

  line(width: number, opacity: number, motion: boolean): LineMaterial {
    return this.get(`line|${width}|${opacity}|${motion}`, () => lineMaterial(this.shared, width, opacity, motion));
  }

  dots(motion: boolean, onTop = false): THREE.PointsMaterial {
    return this.get(`dots|${motion}|${onTop}`, () => dotMaterial(this.shared, motion, onTop));
  }

  glyph(kind: SceneGlyph["kind"], color: string, px: number): THREE.PointsMaterial {
    return this.get(`glyph|${kind}|${color}|${px}`, () => glyphMaterial(this.shared, kind, color, px));
  }

  faces(k: FaceKind): THREE.Material {
    return this.get(`faces|${k.shaded}|${k.transparent}|${k.contour}|${k.motion}`, () => faceMaterial(this.shared, k));
  }

  cones(): THREE.MeshBasicMaterial {
    return this.get("cones", coneMaterial);
  }

  /** The hover highlight's faces: see-through over everything, in one colour per vertex, moving with an animation. */
  hoverFaces(): THREE.MeshBasicMaterial {
    return this.get("hover", () => {
      const m = new THREE.MeshBasicMaterial({ vertexColors: true, transparent: true, opacity: 0.25, side: THREE.DoubleSide, depthWrite: false, depthTest: false });
      m.onBeforeCompile = (shader) => withMotion(shader, this.shared);
      m.customProgramCacheKey = () => "osab-hover";
      return m;
    });
  }

  dispose() {
    for (const m of this.made.values()) m.dispose();
    this.made.clear();
  }
}

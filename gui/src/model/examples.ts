// Example models (File > Open Example, and the web demo's welcome): benchmark models with verified results,
// bundled straight from tests/benchmarks so they are always the checked files. They open as if drawn in the
// app: its names and labels (J, C/B/D, F/W, L) and the defaults of drawn objects (`drawn*Defaults`, so a new
// default reaches the examples too; examples.test.ts checks them against objects drawn the same way).

import b05 from "../../../tests/benchmarks/b05_portal_frame/model.json";
import b24 from "../../../tests/benchmarks/b24_slab_on_columns/model.json";
import b76 from "../../../tests/benchmarks/b76_l_floor_offsets/model.json";
import type { Key } from "../i18n";
import { drawnAreaDefaults } from "./areaOps";
import { drawnFrameDefaults, drawnNodeDefaults, newDiaphragms, normalizeModel } from "./ops";
import { withLabels } from "./labels";
import { namesByKind, renameObjects } from "./relabel";
import type { Model } from "./types";

export interface Example {
  id: string;
  /** File name shown in the title bar. */
  file: string;
  title: Key;
  description: Key;
  model: unknown;
}

export const EXAMPLES: Example[] = [
  { id: "b05", file: "B05 portal frame.osab", title: "demo.b05", description: "demo.b05Desc", model: b05 },
  { id: "b24", file: "B24 slab on columns.osab", title: "demo.b24", description: "demo.b24Desc", model: b24 },
  { id: "b76", file: "B76 L-shaped floor.osab", title: "demo.b76", description: "demo.b76Desc", model: b76 },
];

/**
 * An example as it opens: the benchmark model with the app's names and labels, and what objects drawn in the
 * app get where the file gives nothing: automatic end offsets, the drawn insertion points (no stiffness
 * transform), and in a 3D model the new model's semi-rigid diaphragm on floors and story joints (it ties
 * nothing). So the results are the benchmark's.
 */
export function exampleModel(ex: Example): Model {
  const m = normalizeModel(ex.model);
  const at = (id: string): [number, number, number] => {
    const n = m.nodes.find((x) => x.id === id)!;
    return [n.x, n.y, n.z];
  };
  if (m.dimension === "3D" && !m.diaphragms?.length) {
    m.diaphragms = newDiaphragms();
    for (const n of m.nodes) fillMissing(n, drawnNodeDefaults(m, at(n.id)));
  }
  for (const f of m.frames) if (f.section) fillMissing(f, drawnFrameDefaults(m, at(f.nodes[0]), at(f.nodes[1])));
  for (const a of m.areas) if (a.section && !a.opening) fillMissing(a, drawnAreaDefaults(m, a.nodes.map(at)));
  return withLabels(renameObjects(m, namesByKind(m)));
}

/** Sets the fields of `defaults` that `o` does not have. */
function fillMissing<T extends object>(o: T, defaults: Partial<T>) {
  for (const [k, v] of Object.entries(defaults)) if ((o as Record<string, unknown>)[k] === undefined) (o as Record<string, unknown>)[k] = v;
}

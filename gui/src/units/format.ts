// The open model's display units, for React components (pure helpers in ./display).

import { useMemo } from "react";

import { useStore } from "../state/store";
import { unitFormat, type UnitFormat } from "./display";
import { unitSystemOf } from "./units";

export * from "./display";

/** The display units of the open model. */
export function useUnits(): UnitFormat {
  const system = useStore((s) => unitSystemOf(s.model.gui?.units));
  return useMemo(() => unitFormat(system), [system]);
}

// The browser version is the web demo: the L-shaped floor example opens on every visit, the welcome
// is under Help > Web Demo and the notice in the status bar. The desktop app shows none of it.

import { openExample } from "./actions";
import { backend } from "./ipc/backend";

export const isDemo = backend.kind === "browser";

/** Set in browser storage by the click tests, which start from an empty model. */
const START_EMPTY = "osab.startEmpty";

function startEmpty(): boolean {
  try {
    return localStorage.getItem(START_EMPTY) === "1";
  } catch {
    return false;
  }
}

/** Opens the L-shaped floor example when the web demo opens. */
export function exampleOnOpen() {
  if (!isDemo || startEmpty()) return;
  void openExample("b76");
}

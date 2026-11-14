// Objects of a model by id for the scene builders. The models drawn are the store's, which are never changed
// in place, so each list is indexed once and the index is kept as long as the list lives.

import type { Area, Frame, Id, Model, ModelNode } from "../model/types";

const indexes = new WeakMap<object, { length: number; byId: Map<Id, unknown> }>();

/** The list's objects by id (the first one of an id, as `find` gives). */
function index<T extends { id: Id }>(list: T[]): Map<Id, T> {
  const hit = indexes.get(list);
  if (hit && hit.length === list.length) return hit.byId as Map<Id, T>;
  const byId = new Map<Id, T>();
  for (const x of list) if (!byId.has(x.id)) byId.set(x.id, x);
  indexes.set(list, { length: list.length, byId });
  return byId;
}

export const nodeOf = (m: Model, id: Id): ModelNode | undefined => index(m.nodes).get(id);
export const frameOf = (m: Model, id: Id): Frame | undefined => index(m.frames).get(id);
export const areaOf = (m: Model, id: Id): Area | undefined => index(m.areas).get(id);

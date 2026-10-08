import type { Field, NodeDef, Value } from "./bevy";

/** The node catalog Bevy serves once at startup (`bevy.catalog()`). Kept on
 *  `globalThis` so a hot reload, which re-runs this module, keeps it. */
const store: { defs: NodeDef[]; byKind: Map<string, NodeDef> } = ((
  globalThis as Record<string, unknown>
).__nodesCatalog ??= { defs: [], byKind: new Map() }) as never;

export function setCatalog(defs: NodeDef[]) {
  store.defs = defs;
  store.byKind = new Map(defs.map((d) => [d.kind, d]));
}

export function catalogLoaded(): boolean {
  return store.defs.length > 0;
}

export function allDefs(): NodeDef[] {
  return store.defs;
}

export function def(kind: string): NodeDef {
  const d = store.byKind.get(kind);
  if (!d) throw new Error(`unknown node kind ${kind}`);
  return d;
}

/** A node's value for a parameter or inline input, else the default. */
export function valueOf(
  values: Record<string, Value>,
  id: string,
  field: Field,
): Value {
  return values[id] ?? field.default;
}

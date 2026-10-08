import { type Context, type Dispatch, createContext, useContext } from "react";
import { type Doc, type NodeId, emptyDoc } from "./model";

/** The editor's state: the document with its history, and the selection
 *  (not part of undo). */
export interface EditorState {
  doc: Doc;
  past: Doc[];
  future: Doc[];
  selection: NodeId[];
  active: NodeId | null;
  /** The preset the document started from. */
  preset: string | null;
}

export type SelectMode = "set" | "add" | "toggle" | "remove";

export type Action =
  /** Change the document. `record` pushes the previous one onto the undo
   *  history — a drag records its first step only. */
  | {
      type: "edit";
      edit: (doc: Doc) => Doc;
      record: boolean;
      /** Select these afterwards (e.g. the pasted nodes). */
      select?: NodeId[];
    }
  | { type: "select"; ids: NodeId[]; mode: SelectMode; active?: NodeId | null }
  | { type: "load"; doc: Doc; preset: string | null }
  | { type: "undo" }
  | { type: "redo" };

const HISTORY = 100;

export const initialEditor = (): EditorState => ({
  doc: emptyDoc(),
  past: [],
  future: [],
  selection: [],
  active: null,
  preset: null,
});

/** Drop selected ids that no longer exist. */
function prune(
  state: EditorState,
  doc: Doc,
): Pick<EditorState, "selection" | "active"> {
  return {
    selection: state.selection.filter((id) => doc.nodes[id]),
    active:
      state.active !== null && doc.nodes[state.active] ? state.active : null,
  };
}

export function reduce(state: EditorState, action: Action): EditorState {
  switch (action.type) {
    case "edit": {
      const doc = action.edit(state.doc);
      if (doc === state.doc && !action.select) return state;
      const next: EditorState = {
        ...state,
        doc,
        past: action.record
          ? [...state.past, state.doc].slice(-HISTORY)
          : state.past,
        future: action.record ? [] : state.future,
      };
      if (action.select) {
        next.selection = action.select;
        next.active = action.select.at(-1) ?? null;
        return next;
      }
      return { ...next, ...prune(next, doc) };
    }
    case "select": {
      const ids = action.ids;
      let selection: NodeId[];
      if (action.mode === "set") selection = ids;
      else if (action.mode === "add")
        selection = [
          ...state.selection,
          ...ids.filter((id) => !state.selection.includes(id)),
        ];
      else if (action.mode === "remove")
        selection = state.selection.filter((id) => !ids.includes(id));
      else
        selection = [
          ...state.selection.filter((id) => !ids.includes(id)),
          ...ids.filter((id) => !state.selection.includes(id)),
        ];
      const active =
        action.active !== undefined
          ? action.active
          : selection.includes(state.active ?? -1)
            ? state.active
            : (selection.at(-1) ?? null);
      return { ...state, selection, active };
    }
    case "load":
      return {
        ...state,
        doc: action.doc,
        past: [...state.past, state.doc].slice(-HISTORY),
        future: [],
        selection: [],
        active: null,
        preset: action.preset,
      };
    case "undo": {
      const doc = state.past.at(-1);
      if (!doc) return state;
      const next = {
        ...state,
        doc,
        past: state.past.slice(0, -1),
        future: [state.doc, ...state.future],
      };
      return { ...next, ...prune(next, doc) };
    }
    case "redo": {
      const doc = state.future[0];
      if (!doc) return state;
      const next = {
        ...state,
        doc,
        past: [...state.past, state.doc],
        future: state.future.slice(1),
      };
      return { ...next, ...prune(next, doc) };
    }
  }
}

type Editor = { state: EditorState; dispatch: Dispatch<Action> };

/** On `globalThis`: a hot reload re-runs this module, and a fresh context
 *  would remount the whole editor, losing its state. */
export const EditorContext = ((
  globalThis as Record<string, unknown>
).__nodesEditor ??= createContext<Editor | null>(
  null,
)) as Context<Editor | null>;

export function useEditor() {
  const ctx = useContext(EditorContext);
  if (!ctx) throw new Error("useEditor outside the editor");
  return ctx;
}

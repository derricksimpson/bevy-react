import type { Dispatch } from "react";
import { def } from "../catalog";
import type { Action, EditorState } from "./editor";
import type { Interaction } from "./interact";
import {
  type Clip,
  type NodeId,
  copy,
  nodeList,
  paste,
  removeNodes,
} from "./model";

/** The copy buffer, for the session (on `globalThis`: survives hot reloads). */
const clipboard: { clip: Clip | null } = ((
  globalThis as Record<string, unknown>
).__nodesClipboard ??= { clip: null }) as never;

/** Editing commands, shared by the shortcuts, the menus and the context
 *  menu. `frame` fits the view to some nodes (all when none are given). */
export function makeCommands(
  ix: Interaction,
  state: () => EditorState,
  dispatch: Dispatch<Action>,
  frame: (ids?: NodeId[]) => void,
  /** The view's center, viewport px. */
  center: () => { x: number; y: number },
) {
  const selection = () =>
    state().selection.filter((id) => state().doc.nodes[id]);
  const toggle = (key: "mute" | "collapsed" | "hidePreview") => {
    const ids = selection();
    const doc = state().doc;
    const on = ids.some((id) => !doc.nodes[id][key]);
    ix.patch(() => ({ [key]: on }), ids);
  };
  /** Paste a clip offset from where it was, then pick it up (Shift+D). */
  const placeClip = (
    clip: Clip,
    at: { x: number; y: number },
    grab: boolean,
  ) => {
    const doc = state().doc;
    const first = doc.nextId;
    const ids = clip.nodes.map((_, i) => first + i);
    ix.edit((d) => paste(d, clip, at)[0], true, ids);
    if (!grab) return;
    const minX = Math.min(...clip.nodes.map((n) => n.x));
    const minY = Math.min(...clip.nodes.map((n) => n.y));
    ix.startGrab(
      ids,
      new Map(
        clip.nodes.map((n, i) => [
          ids[i],
          { x: n.x - minX + at.x, y: n.y - minY + at.y },
        ]),
      ),
    );
  };

  return {
    undo: () => dispatch({ type: "undo" }),
    redo: () => dispatch({ type: "redo" }),
    delete: () => ix.edit((doc) => removeNodes(doc, selection())),
    duplicate() {
      const clip = copy(state().doc, selection());
      if (!clip.nodes.length) return;
      const x = Math.min(...clip.nodes.map((n) => n.x)) + 30;
      const y = Math.min(...clip.nodes.map((n) => n.y)) + 30;
      placeClip(clip, { x, y }, true);
    },
    copy() {
      const clip = copy(state().doc, selection());
      if (clip.nodes.length) clipboard.clip = clip;
    },
    paste() {
      if (clipboard.clip)
        placeClip(clipboard.clip, ix.toGraph(ix.cursor.current), false);
    },
    selectAll: () =>
      ix.select(
        nodeList(state().doc).map((n) => n.id),
        "set",
      ),
    deselect: () => ix.select([], "set", null),
    grab: () => ix.startGrab(selection()),
    mute: () => toggle("mute"),
    collapse: () => toggle("collapsed"),
    preview: () => toggle("hidePreview"),
    /** Add a node at the view's center, following the cursor until placed. */
    place: (kind: string) => ix.addGrabbed(kind, ix.toGraph(center())),
    frameAll: () => frame(),
    frameSelected: () => frame(selection().length ? selection() : undefined),
    /** Whether the selection has anything a command can act on. */
    hasSelection: () => selection().length > 0,
    canPreview: () =>
      selection().some((id) => def(state().doc.nodes[id].kind).preview),
  };
}

export type Commands = ReturnType<typeof makeCommands>;

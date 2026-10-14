// The Nodes tree: an explorer over the op mirror. Rows expand/
// collapse, click selects (driving the inspector + the on-screen highlight),
// and hovering a row highlights its node in the running UI. Each row leads with
// the React component that emitted the node, where the dev build could
// attribute it (`owners.ts`).

import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { sendHighlight, sendReveal } from "./api";
import { Chip, IconButton } from "./DevtoolsHost";
import { mirror, type MirrorNode } from "./mirror";
import { theme } from "./theme";

/** Re-render whenever the mirror applies a batch. */
export function useMirrorVersion(): number {
  return useSyncExternalStore(mirror.subscribe, mirror.getVersion);
}

/** In "auto" expansion, rows this deep start expanded; deeper ones collapsed. */
const AUTO_EXPAND_DEPTH = 2;

/** The default expansion for rows without an explicit user toggle. */
type Baseline = "auto" | "all" | "none";

/** The newest pick a row has sent: a row remounting later (its parent
 *  re-expanded, the tab shown again) doesn't scroll back to it. */
let revealed = 0;

/** A node to bring into view (pick mode's selection); `seq` tells one pick
 *  of a node from the next. */
export type Reveal = { id: number; seq: number };

/** The tree root `id` belongs to: the nearest `<root>` ancestor, else main (0). */
function rootOf(id: number): number {
  let cursor: number | null = id;
  while (cursor != null && cursor !== 0) {
    const node = mirror.get(cursor);
    if (!node) break;
    if (node.kind === "root") return node.id;
    cursor = node.parent;
  }
  return 0;
}

export function TreeView({
  selected,
  onSelect,
  reveal,
}: {
  selected: number | null;
  onSelect: (id: number | null) => void;
  /** Expand this node's ancestors and scroll its row into view (pick mode's
   *  selection). */
  reveal: Reveal | null;
}) {
  useMirrorVersion();
  // Which root's tree is shown: 0 = the main window tree, else a `<root>` id.
  const [activeRoot, setActiveRoot] = useState(0);
  // Explicit user toggles override the baseline (so a shallow row can be
  // collapsed and a deep one expanded, independent of tree churn).
  const [overrides, setOverrides] = useState<ReadonlyMap<number, boolean>>(
    new Map(),
  );
  const [baseline, setBaseline] = useState<Baseline>("auto");
  const toggle = (id: number, expand: boolean) =>
    setOverrides((prev) => new Map(prev).set(id, expand));
  const setAll = (next: Baseline) => {
    setBaseline(next);
    setOverrides(new Map());
  };

  useEffect(() => {
    if (reveal == null) return;
    // Show the tree the picked node lives in, with its ancestors expanded.
    setActiveRoot(rootOf(reveal.id));
    setOverrides((prev) => {
      const next = new Map(prev);
      let cursor = mirror.get(reveal.id)?.parent ?? null;
      while (cursor != null && cursor !== 0) {
        next.set(cursor, true);
        cursor = mirror.get(cursor)?.parent ?? null;
      }
      return next;
    });
  }, [reveal]);

  const roots = mirror.roots();
  // A previously selected root that unmounted falls back to main.
  const active = roots.some((r) => r.id === activeRoot) ? activeRoot : 0;
  const topLevel =
    active === 0
      ? mirror.root().children.filter((id) => !mirror.isDevtools(id))
      : [active];

  return (
    <node style={{ flexDirection: "column" }}>
      <node
        style={{
          flexDirection: "row",
          flexWrap: "wrap",
          alignItems: "center",
          gap: 4,
          padding: { horizontal: 6, vertical: 4 },
          border: { bottom: 1 },
          borderColor: theme.border,
        }}
      >
        {roots.map((root) => (
          <Chip
            key={root.id}
            label={root.label}
            active={root.id === active}
            onClick={() => setActiveRoot(root.id)}
          />
        ))}
        <node style={{ flexGrow: 1 }} />
        <IconButton
          icon="expandAll"
          label="expand"
          onClick={() => setAll("all")}
        />
        <IconButton
          icon="collapseAll"
          label="collapse"
          onClick={() => setAll("none")}
        />
      </node>
      {topLevel.length === 0 ? (
        <text style={{ color: theme.textDim, fontSize: 12, margin: 10 }}>
          No app nodes yet.
        </text>
      ) : (
        <node style={{ flexDirection: "column", padding: { vertical: 4 } }}>
          {topLevel.map((id) => (
            <TreeRows
              key={id}
              id={id}
              depth={0}
              activeRoot={active}
              selected={selected}
              onSelect={onSelect}
              overrides={overrides}
              baseline={baseline}
              toggle={toggle}
              reveal={reveal}
            />
          ))}
        </node>
      )}
    </node>
  );
}

function TreeRows({
  id,
  depth,
  activeRoot,
  selected,
  onSelect,
  overrides,
  baseline,
  toggle,
  reveal,
  parentOwner,
}: {
  id: number;
  depth: number;
  activeRoot: number;
  selected: number | null;
  onSelect: (id: number | null) => void;
  overrides: ReadonlyMap<number, boolean>;
  baseline: Baseline;
  toggle: (id: number, expand: boolean) => void;
  reveal: Reveal | null;
  /** The owner of the row above, for the boundary rule (see `Row`). */
  parentOwner?: string;
}) {
  const node = mirror.get(id);
  if (!node) return null;
  // A nested `<root>` that isn't the active one renders as a stub: its
  // subtree is a detached tree of its own — switch roots to inspect it.
  const foreignRoot = node.kind === "root" && id !== activeRoot;
  const hasChildren = !foreignRoot && node.children.length > 0;
  const expanded =
    !foreignRoot &&
    (overrides.get(id) ??
      (baseline === "all" ||
        (baseline === "auto" && depth < AUTO_EXPAND_DEPTH)));
  return (
    <node style={{ flexDirection: "column" }}>
      <Row
        node={node}
        depth={depth}
        caret={hasChildren ? expanded : undefined}
        hint={foreignRoot ? "detached" : undefined}
        selected={selected === id}
        reveal={reveal?.id === id ? reveal.seq : undefined}
        parentOwner={parentOwner}
        onCaret={hasChildren ? () => toggle(id, !expanded) : undefined}
        onSelect={() => onSelect(id)}
      />
      {expanded &&
        node.children.map((child) => (
          <TreeRows
            key={child}
            id={child}
            depth={depth + 1}
            activeRoot={activeRoot}
            selected={selected}
            onSelect={onSelect}
            overrides={overrides}
            baseline={baseline}
            toggle={toggle}
            reveal={reveal}
            parentOwner={node.owner}
          />
        ))}
    </node>
  );
}

function Row({
  node,
  depth,
  caret,
  hint,
  selected,
  reveal,
  onCaret,
  onSelect,
  parentOwner,
}: {
  node: MirrorNode;
  depth: number;
  /** Expanded or collapsed; absent on a leaf. */
  caret?: boolean;
  hint?: string;
  selected: boolean;
  /** The pick to bring this row into view for (its `Reveal.seq`). */
  reveal?: number;
  onCaret?: () => void;
  onSelect: () => void;
  parentOwner?: string;
}) {
  // Attribution BOUNDARIES only: a component's whole subtree shares one owner,
  // so naming it on every row would drown the tree — the name instead marks
  // where a component's output begins. "Unattributed" is a value like any
  // other, so a node React could not attribute under one it could shows `<?>`
  // once and its descendants stay blank.
  const showOwner = node.owner !== parentOwner;
  // A picked row reports its own node id, once per pick, for Bevy to scroll
  // into view — JS can't measure where it landed. A host `ref` resolves to
  // the instance (`getPublicInstance`: `{ id }`); `<node>`'s typing has none.
  const revealRef = useMemo(
    () =>
      reveal === undefined
        ? undefined
        : (instance: { id: number } | null) => {
            if (!instance || reveal <= revealed) return;
            revealed = reveal;
            sendReveal(instance.id);
          },
    [reveal],
  );
  return (
    <node
      {...(revealRef && ({ ref: revealRef } as object))}
      style={{
        flexDirection: "row",
        alignItems: "center",
        padding: { left: 8 + depth * 12, top: 1, bottom: 1, right: 4 },
        backgroundColor: selected ? theme.bgActive : "transparent",
      }}
      hoverStyle={{
        backgroundColor: selected ? theme.bgActive : theme.bgAlt,
      }}
      onClick={onSelect}
      onPointerEnter={() => sendHighlight(node.id)}
      onPointerLeave={() => sendHighlight(null)}
    >
      <button
        style={{
          width: 12,
          height: 12,
          alignItems: "center",
          justifyContent: "center",
        }}
        onClick={onCaret ?? onSelect}
      >
        {caret !== undefined && <Chevron open={caret} />}
      </button>
      {showOwner && (
        <text
          style={{
            color: theme.component,
            fontSize: 11,
            fontFamily: theme.mono,
            margin: { right: 6 },
            lineBreak: "noWrap",
          }}
        >
          {node.owner ? `<${node.owner}>` : "<?>"}
        </text>
      )}
      {/* `noWrap`: long labels overflow into the ScrollArea's horizontal
          scroll range instead of wrapping the row. */}
      <text
        style={{
          color: selected ? theme.accent : theme.accentAlt,
          fontSize: 11,
          fontFamily: theme.mono,
          lineBreak: "noWrap",
        }}
      >
        {rowLabel(node)}
      </text>
      {node.text !== undefined && node.kind !== "#text" && (
        <text
          style={{
            color: theme.textDim,
            fontSize: 11,
            fontFamily: theme.mono,
            margin: { left: 6 },
            lineBreak: "noWrap",
          }}
        >
          {truncate(node.text, 28)}
        </text>
      )}
      {hint && (
        <text
          style={{
            color: theme.textDim,
            fontSize: 10,
            margin: { left: 6 },
            lineBreak: "noWrap",
          }}
        >
          {hint}
        </text>
      )}
    </node>
  );
}

export function rowLabel(node: MirrorNode): string {
  if (node.kind === "#text") return `"${truncate(node.text ?? "", 28)}"`;
  // A named element shows its `name` (a `<root>`'s doubles as its devtools
  // root-selector label).
  if (typeof node.props.name === "string" && node.props.name !== "") {
    return `<${node.kind} "${truncate(node.props.name, 24)}">`;
  }
  return `<${node.kind}>`;
}

function truncate(s: string, max: number): string {
  return s.length > max ? `${s.slice(0, max - 1)}...` : s;
}

/** A tree caret: two borders of a small square, turned to point right
 *  (collapsed) or down — drawn, not an icon, so a long tree costs no
 *  texture per row. The nudge centers the point's visual weight. */
function Chevron({ open }: { open: boolean }) {
  return (
    <node
      style={{
        width: 5,
        height: 5,
        border: { right: 1.5, bottom: 1.5 },
        borderColor: theme.textDim,
        transform: {
          rotate: open ? 45 : -45,
          ...(open ? { translateY: -1 } : { translateX: -1 }),
        },
      }}
    />
  );
}

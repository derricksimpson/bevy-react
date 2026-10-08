import { type ReactNode, useEffect, useSyncExternalStore } from "react";
import { bevy } from "../bevy";
import { endTyping } from "../hooks";

/** One floating panel (a menu, a color picker) at window coordinates. */
interface Popup {
  id: number;
  x: number;
  y: number;
  /** Its estimated size, to keep it inside the window. */
  w: number;
  h: number;
  render: (close: () => void) => ReactNode;
  onClose?: () => void;
}

/** The open popups, a stack (a submenu or picker opens above its menu).
 *  On `globalThis` so a hot reload keeps them. */
const store: { list: Popup[]; next: number; listeners: Set<() => void> } = ((
  globalThis as Record<string, unknown>
).__nodesPopups ??= { list: [], next: 1, listeners: new Set() }) as never;

function emit() {
  for (const l of store.listeners) l();
}

export function openPopup(p: Omit<Popup, "id">): () => void {
  const id = store.next++;
  store.list = [...store.list, { ...p, id }];
  emit();
  return () => closePopup(id);
}

function closePopup(id: number) {
  const popup = store.list.find((p) => p.id === id);
  if (!popup) return;
  store.list = store.list.filter((p) => p.id !== id);
  emit();
  popup.onClose?.();
}

export function closeAll() {
  const list = store.list;
  store.list = [];
  emit();
  for (const p of list) p.onClose?.();
}

export const anyPopup = () => store.list.length > 0;

function usePopups(): Popup[] {
  return useSyncExternalStore(
    (l) => {
      store.listeners.add(l);
      return () => store.listeners.delete(l);
    },
    () => store.list,
  );
}

/** Renders the open popups above everything. A press outside them closes
 *  them all; Escape closes the topmost. */
export function PopupLayer({
  width,
  height,
}: {
  width: number;
  height: number;
}) {
  const list = usePopups();
  useEffect(
    () =>
      bevy.on("keyDown", (e) => {
        if (e.key === "Escape" && store.list.length) {
          closePopup(store.list[store.list.length - 1].id);
        }
      }),
    [],
  );
  if (!list.length) return null;
  return (
    <node
      style={{
        positionType: "absolute",
        left: 0,
        top: 0,
        width: "100%",
        height: "100%",
        globalZIndex: 100,
      }}
      onPointerDown={() => {
        endTyping();
        closeAll();
      }}
    >
      {list.map((p) => (
        <node
          key={p.id}
          style={{
            positionType: "absolute",
            left: Math.max(4, Math.min(p.x, width - p.w - 4)),
            top: Math.max(4, Math.min(p.y, height - p.h - 4)),
          }}
          // Swallow presses inside, or the backdrop would close it.
          onPointerDown={() => {}}
        >
          {p.render(() => closePopup(p.id))}
        </node>
      ))}
    </node>
  );
}

import type { PointerEventData } from "bevy-react";
import { EnumField } from "../fields/choice";
import type { Commands } from "../graph/commands";
import { endTyping } from "../hooks";
import { PRESETS } from "../presets/presets";
import { C, CATEGORY, UI } from "../theme";
import { AddMenu, ADD_MENU_HEIGHT } from "./AddMenu";
import { type MenuItem, Menu, MENU_WIDTH, menuHeight } from "./Menu";
import { openPopup } from "./popup";

function MenuButton({
  label,
  items,
}: {
  label: string;
  items: () => MenuItem[];
}) {
  return (
    <node
      style={{
        height: 22,
        padding: { horizontal: 9 },
        borderRadius: 4,
        justifyContent: "center",
      }}
      hoverStyle={{ backgroundColor: "#3a3a3a" }}
      onPointerDown={(e: PointerEventData) => {
        if (e.button !== 0) return;
        endTyping();
        const list = items();
        openPopup({
          x: e.clientX - e.x * 40,
          y: UI.bar - 2,
          w: MENU_WIDTH,
          h: menuHeight(list),
          render: (close) => <Menu items={list} close={close} />,
        });
      }}
    >
      <text style={{ fontSize: UI.font, color: C.text }}>{label}</text>
    </node>
  );
}

/** The top bar: menus, the preset "workspaces", evaluation status and the
 *  resolution. */
export function TopBar({
  commands,
  preset,
  onPreset,
  onNew,
  resolution,
  onResolution,
  pending,
  totalMs,
  sidebar,
  onToggleSidebar,
}: {
  commands: () => Commands | null;
  preset: string | null;
  onPreset: (id: string) => void;
  onNew: () => void;
  resolution: number;
  onResolution: (r: number) => void;
  pending: number;
  totalMs: number;
  sidebar: boolean;
  onToggleSidebar: () => void;
}) {
  const run = (f: (c: Commands) => void) => () => {
    const c = commands();
    if (c) f(c);
  };
  const file = (): MenuItem[] => [
    { label: "New", onSelect: onNew },
    {
      label: "Open Preset",
      items: PRESETS.map((p) => ({
        label: p.label,
        checked: p.id === preset,
        onSelect: () => onPreset(p.id),
      })),
    },
  ];
  const edit = (): MenuItem[] => [
    { label: "Undo", hint: "Ctrl Z", onSelect: run((c) => c.undo()) },
    { label: "Redo", hint: "Ctrl Shift Z", onSelect: run((c) => c.redo()) },
    {
      label: "Duplicate",
      hint: "Shift D",
      onSelect: run((c) => c.duplicate()),
      separator: true,
    },
    { label: "Copy", hint: "Ctrl C", onSelect: run((c) => c.copy()) },
    { label: "Paste", hint: "Ctrl V", onSelect: run((c) => c.paste()) },
    { label: "Delete", hint: "X", onSelect: run((c) => c.delete()) },
    {
      label: "Select All",
      hint: "A",
      onSelect: run((c) => c.selectAll()),
      separator: true,
    },
    {
      label: "Deselect All",
      hint: "Alt A",
      onSelect: run((c) => c.deselect()),
    },
    {
      label: "Mute",
      hint: "M",
      onSelect: run((c) => c.mute()),
      separator: true,
    },
    { label: "Collapse", hint: "H", onSelect: run((c) => c.collapse()) },
  ];
  const view = (): MenuItem[] => [
    { label: "Frame All", hint: "Home", onSelect: run((c) => c.frameAll()) },
    {
      label: "Frame Selected",
      hint: "F",
      onSelect: run((c) => c.frameSelected()),
    },
    {
      label: "Sidebar",
      hint: "N",
      checked: sidebar,
      onSelect: onToggleSidebar,
      separator: true,
    },
  ];

  return (
    <node
      style={{
        height: UI.bar,
        flexDirection: "row",
        alignItems: "center",
        gap: 2,
        padding: { horizontal: 8 },
        backgroundColor: C.bar,
      }}
    >
      <node
        style={{
          flexDirection: "row",
          alignItems: "center",
          gap: 6,
          margin: { right: 8 },
        }}
      >
        <node
          style={{
            width: 9,
            height: 9,
            borderRadius: 2,
            backgroundColor: CATEGORY.texture.header,
          }}
        />
        <node
          style={{
            width: 9,
            height: 9,
            borderRadius: 5,
            backgroundColor: "#c7c729",
          }}
        />
        <text style={{ fontSize: 13, color: C.text, fontWeight: "semibold" }}>
          Nodes
        </text>
      </node>
      <MenuButton label="File" items={file} />
      <MenuButton label="Edit" items={edit} />
      <node
        style={{
          height: 22,
          padding: { horizontal: 9 },
          borderRadius: 4,
          justifyContent: "center",
        }}
        hoverStyle={{ backgroundColor: "#3a3a3a" }}
        onPointerDown={(e: PointerEventData) => {
          if (e.button !== 0) return;
          endTyping();
          openPopup({
            x: e.clientX - e.x * 40,
            y: UI.bar - 2,
            w: MENU_WIDTH,
            h: ADD_MENU_HEIGHT,
            render: (close) => (
              <AddMenu
                close={close}
                onPick={(kind) => commands()?.place(kind)}
              />
            ),
          });
        }}
      >
        <text style={{ fontSize: UI.font, color: C.text }}>Add</text>
      </node>
      <MenuButton label="View" items={view} />
      <node style={{ width: 18 }} />
      {PRESETS.map((p) => (
        <node
          key={p.id}
          style={{
            height: 24,
            padding: { horizontal: 12 },
            justifyContent: "center",
            borderRadius: { top: 5, right: 5, bottom: 0, left: 0 },
            backgroundColor: p.id === preset ? C.pane : "transparent",
            margin: { top: 6 },
          }}
          hoverStyle={
            p.id === preset ? undefined : { backgroundColor: "#2e2e2e" }
          }
          onClick={() => onPreset(p.id)}
        >
          <text
            style={{
              fontSize: UI.font,
              color: p.id === preset ? C.text : C.textDim,
            }}
          >
            {p.label}
          </text>
        </node>
      ))}
      <node style={{ flexGrow: 1 }} />
      <node
        style={{
          flexDirection: "row",
          alignItems: "center",
          gap: 6,
          margin: { right: 12 },
        }}
      >
        <node
          style={{
            width: 7,
            height: 7,
            borderRadius: 4,
            backgroundColor: pending ? "#ffd166" : "#5fbf6a",
          }}
        />
        <text style={{ fontSize: UI.small, color: C.textDim }}>
          {pending
            ? `Evaluating ${pending} node${pending === 1 ? "" : "s"}…`
            : `Graph ${totalMs.toFixed(0)} ms`}
        </text>
      </node>
      <node style={{ width: 92 }}>
        <EnumField
          label="Resolution"
          value={String(resolution)}
          options={[256, 512, 1024].map((r) => ({
            id: String(r),
            label: `${r} px`,
          }))}
          z={1}
          onChange={(v) => onResolution(Number(v))}
        />
      </node>
    </node>
  );
}

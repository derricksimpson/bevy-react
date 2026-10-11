import { useEffect, useState } from "react";
import { read } from "../game/act";
import { countItem, credits } from "../game/items";
import { LOGS, OBJECTIVES } from "../game/lore";
import { objective } from "../game/reduce";
import { getGame, useGame } from "../game/store";
import { useDebug, useKeys } from "../hooks";
import { cue } from "../sound";
import { C, F, T } from "../theme";
import { Holo } from "./Holo";
import { Inventory } from "./menu/Inventory";
import { Logs } from "./menu/Logs";
import { MapTab } from "./menu/MapTab";
import { Objectives } from "./menu/Objectives";
import { Bar, Rule, Shell, Tabs } from "./shared/kit";

const TABS = [
  { id: "inventory", label: "INVENTORY" },
  { id: "map", label: "MAP" },
  { id: "objectives", label: "OBJECTIVES" },
  { id: "logs", label: "LOGS" },
] as const;
type Tab = (typeof TABS)[number]["id"];

/** How long an audio log's transmission plays. */
const PLAY_MS = 4200;

/** The suit's RIG menu, projected in front of the engineer (beat 3): his
 *  vitals and credits, then four pages — the inventory, the deck map, the
 *  objective chain and the logs. Tab / Shift+Tab or a click switch pages,
 *  each change sweeping a scan line across the hologram. */
export function Menu() {
  const active = useGame((g) => g.beat) === 3;
  const [tab, setTab] = useState<Tab>("inventory");
  const [slot, setSlot] = useState(0);
  const [log, setLog] = useState(LOGS[0].id);
  const [playing, setPlaying] = useState(false);

  const pick = (next: Tab) => {
    if (next === tab) return;
    setTab(next);
    cue("tab");
  };
  const openLog = (id: string, play = false) => {
    setLog(id);
    setPlaying(false);
    read(id);
    cue("tick");
    if (play && LOGS.find((l) => l.id === id)?.audio) startPlaying();
  };
  const startPlaying = () => {
    setPlaying(true);
    cue("log");
  };

  useEffect(() => {
    if (!playing) return;
    const t = setTimeout(() => setPlaying(false), PLAY_MS);
    return () => clearTimeout(t);
  }, [playing]);

  useKeys(active, (e) => {
    if (e.code !== "Tab") return;
    const at = TABS.findIndex((t) => t.id === tab);
    const step = e.shiftKey ? TABS.length - 1 : 1;
    pick(TABS[(at + step) % TABS.length].id);
  });
  useDebug("tab", (id) => {
    if (TABS.some((t) => t.id === id)) setTab(id as Tab);
  });
  useDebug("select", (item) => {
    const at = getGame().pack.slots.findIndex((s) => s?.item === item);
    if (at >= 0) {
      setTab("inventory");
      setSlot(at);
    }
  });
  useDebug("log", (id) => {
    if (!LOGS.some((l) => l.id === id)) return;
    setTab("logs");
    openLog(id, true);
  });

  return (
    <Holo name="menu" page={tab} seed={3}>
      <Shell>
        <Header />
        <node
          style={{
            flexDirection: "row",
            justifyContent: "spaceBetween",
            alignItems: "center",
            margin: { top: 12 },
          }}
        >
          <Tabs tabs={TABS} active={tab} onPick={pick} />
          <text style={{ ...T.small, letterSpacing: 2 }}>TAB · SHIFT+TAB</text>
        </node>
        <Rule />
        <node style={{ flexGrow: 1, margin: { top: 16, bottom: 12 } }}>
          {tab === "inventory" && (
            <Inventory active={active} selected={slot} onSelect={setSlot} />
          )}
          {tab === "map" && <MapTab active={active} />}
          {tab === "objectives" && <Objectives />}
          {tab === "logs" && (
            <Logs
              selected={log}
              playing={playing}
              onSelect={(id) => openLog(id)}
              onPlay={startPlaying}
            />
          )}
        </node>
        <Rule />
        <Footer />
      </Shell>
    </Holo>
  );
}

/** Who and how: the suit, its vitals and stasis, credits, power cells. */
function Header() {
  const suit = useGame((g) => g.suit);
  const pack = useGame((g) => g.pack);
  const vitals =
    suit.health > 0.4 ? C.cyan : suit.health > 0.2 ? C.amber : C.red;
  return (
    <node style={{ flexDirection: "row", alignItems: "center", gap: 40 }}>
      <node style={{ flexDirection: "column", gap: 3 }}>
        <text style={{ ...T.header, fontSize: 26, letterSpacing: 6 }}>RIG</text>
        <text style={{ ...T.small, letterSpacing: 2 }}>
          {"T. OKAFOR · HALDEN ICE & ORE"}
        </text>
      </node>
      <Gauge label="VITALS" value={suit.health} color={vitals} />
      <Gauge label="STASIS" value={suit.stasis} color={C.cyan} />
      <node style={{ flexGrow: 1 }} />
      <node style={{ flexDirection: "column", alignItems: "end", gap: 1 }}>
        <text style={{ ...T.small, letterSpacing: 2 }}>POWER CELLS</text>
        <text style={{ ...T.number, fontSize: 24, color: C.green }}>
          {String(countItem(pack, "power-cell"))}
        </text>
      </node>
      <node style={{ flexDirection: "column", alignItems: "end", gap: 1 }}>
        <text style={{ ...T.small, letterSpacing: 2 }}>CREDITS</text>
        <text style={{ ...T.number, fontSize: 30, color: C.amber }}>
          {credits(pack.credits)}
        </text>
      </node>
    </node>
  );
}

function Gauge({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: string;
}) {
  return (
    <node style={{ flexDirection: "column", gap: 6 }}>
      <node style={{ flexDirection: "row", justifyContent: "spaceBetween" }}>
        <text style={{ ...T.small, letterSpacing: 2 }}>{label}</text>
        <text style={{ fontFamily: F.semibold, fontSize: 14, color }}>
          {`${Math.round(value * 100)}%`}
        </text>
      </node>
      <Bar value={value} color={color} width={200} />
    </node>
  );
}

/** The current objective, always in view. */
function Footer() {
  const current = OBJECTIVES[useGame(objective)];
  return (
    <node
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 16,
        margin: { top: 10 },
      }}
    >
      <text style={{ ...T.small, letterSpacing: 3, color: C.amber }}>
        OBJECTIVE
      </text>
      <text style={{ ...T.label, fontSize: 18, color: C.white }}>
        {current.title}
      </text>
      <text
        style={{ ...T.body, fontSize: 16, color: C.cyanDim, flexShrink: 1 }}
      >
        {current.text}
      </text>
    </node>
  );
}

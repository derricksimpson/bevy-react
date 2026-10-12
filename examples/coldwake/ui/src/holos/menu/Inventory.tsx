import { useEffect } from "react";
import { bevy } from "../../bevy";
import { equip, use } from "../../game/act";
import {
  ITEMS,
  WEAPON_AMMO,
  WEAPONS,
  credits,
  sellPrice,
  type ItemId,
  type WeaponId,
} from "../../game/items";
import { clipSize, reserve } from "../../game/reduce";
import { useGame } from "../../game/store";
import { cue } from "../../sound";
import { C, F, T } from "../../theme";
import { HoloButton, Rule } from "../shared/kit";
import { ItemIcon, KIND_LABEL } from "../shared/items";

/** What using (or holding) an item does, in the detail pane. */
function effect(item: ItemId): string {
  switch (item) {
    case "medpack-s":
      return "RESTORES 25% VITALS";
    case "medpack-m":
      return "RESTORES 50% VITALS";
    case "medpack-l":
      return "RESTORES ALL VITALS";
    case "stasis-pack":
      return "REFILLS THE STASIS MODULE";
    case "air-canister":
      return "+30 S OF AIR";
    case "power-cell":
      return "POWERS ONE NODE AT A BENCH";
    case "semiconductor":
      return `SELLS FOR ${credits(sellPrice(item))} CREDITS`;
  }
  const weapon = WEAPONS.find((w) => WEAPON_AMMO[w].ammo === item);
  return weapon
    ? `LOADS THE ${ITEMS[weapon].name.toUpperCase()} · ${WEAPON_AMMO[weapon].perItem} ROUNDS`
    : "";
}

const USABLE = new Set(["health", "stasis", "air", "ammo"]);

/** The suit's inventory: the pack's slots, the selected item up close on
 *  the turntable, and the weapons he carries. */
export function Inventory({
  active,
  selected,
  onSelect,
}: {
  active: boolean;
  selected: number;
  onSelect: (slot: number) => void;
}) {
  const slots = useGame((g) => g.pack.slots);
  const item = slots[selected]?.item;
  const used = slots.filter(Boolean).length;

  // The turntable shows the selection while this hologram is up.
  useEffect(() => {
    if (active && item) bevy.preview.show({ item });
  }, [active, item]);

  return (
    <node style={{ flexDirection: "row", gap: 28, flexGrow: 1 }}>
      {/* Four 120 px slots and their gaps take 516 px; the slack keeps a
          fractional surface scale from wrapping the fourth. */}
      <node
        style={{ flexDirection: "column", gap: 14, width: 520, flexShrink: 0 }}
      >
        <node style={{ flexDirection: "row", justifyContent: "spaceBetween" }}>
          <text style={{ ...T.small, letterSpacing: 3 }}>SUIT INVENTORY</text>
          <text style={{ ...T.small, letterSpacing: 2, color: C.cyan }}>
            {`${used} / ${slots.length}`}
          </text>
        </node>
        <node style={{ flexDirection: "row", flexWrap: "wrap", gap: 12 }}>
          {slots.map((s, i) => (
            <button
              key={i}
              onClick={() => {
                onSelect(i);
                cue("tick");
              }}
              style={{
                width: 120,
                height: 112,
                alignItems: "center",
                justifyContent: "center",
                border: 1,
                borderColor: i === selected ? C.white : C.clear,
                backgroundColor:
                  i === selected ? "rgba(240, 253, 255, 0.1)" : C.clear,
                cursor: "pointer",
              }}
              hoverStyle={{ backgroundColor: "rgba(127, 227, 255, 0.1)" }}
            >
              {s ? (
                <ItemIcon
                  item={s.item}
                  size={96}
                  count={s.count}
                  lit={i === selected}
                />
              ) : (
                <node
                  style={{
                    width: 96,
                    height: 96,
                    border: 1,
                    borderColor: "rgba(127, 227, 255, 0.12)",
                  }}
                />
              )}
            </button>
          ))}
        </node>
        <Weapons />
      </node>
      <Detail slot={selected} live={active} />
    </node>
  );
}

/** The selected slot up close. */
function Detail({ slot, live }: { slot: number; live: boolean }) {
  const s = useGame((g) => g.pack.slots[slot]);
  if (!s) {
    return (
      <node
        style={{ flexGrow: 1, alignItems: "center", justifyContent: "center" }}
      >
        <text style={{ ...T.label, color: C.cyanDim, letterSpacing: 4 }}>
          EMPTY SLOT
        </text>
      </node>
    );
  }
  const item = ITEMS[s.item];
  return (
    <node style={{ flexGrow: 1, flexDirection: "column", gap: 12 }}>
      <node
        style={{
          flexDirection: "row",
          justifyContent: "spaceBetween",
          alignItems: "end",
        }}
      >
        <text style={{ ...T.header, fontSize: 26, color: C.white }}>
          {item.name.toUpperCase()}
        </text>
        <text style={{ ...T.small, letterSpacing: 3 }}>
          {KIND_LABEL[item.kind]}
        </text>
      </node>
      <Rule />
      <node style={{ flexDirection: "row", gap: 22 }}>
        {/* The `preview` camera renders only while a portal shows it: only
            while this hologram is up. Its pixels change every frame, behind
            the hologram's cached layer: `cache: "never"` re-captures. */}
        <node
          style={{
            width: 340,
            height: 280,
            flexShrink: 0,
            border: 1,
            borderColor: "rgba(127, 227, 255, 0.25)",
            backgroundColor: "rgba(2, 10, 16, 1)",
          }}
        >
          {live && (
            <portal
              target="preview"
              style={{ width: "100%", height: "100%", cache: "never" }}
            />
          )}
        </node>
        <node
          style={{
            flexDirection: "column",
            gap: 14,
            flexGrow: 1,
            flexShrink: 1,
          }}
        >
          <text style={T.body}>{item.text}</text>
          <text
            style={{
              ...T.label,
              fontSize: 17,
              color: C.cyanHi,
              lineBreak: "wordBoundary",
            }}
          >
            {effect(s.item)}
          </text>
          <node style={{ flexDirection: "row", gap: 26 }}>
            <node style={{ flexDirection: "column", gap: 2 }}>
              <text style={{ ...T.small, letterSpacing: 2 }}>CARRIED</text>
              <text style={{ ...T.number, fontSize: 26 }}>
                {String(s.count)}
              </text>
            </node>
            <node style={{ flexDirection: "column", gap: 2 }}>
              <text style={{ ...T.small, letterSpacing: 2 }}>VALUE</text>
              <text style={{ ...T.number, fontSize: 26, color: C.amber }}>
                {credits(sellPrice(s.item))}
              </text>
            </node>
          </node>
        </node>
      </node>
      <node style={{ flexDirection: "row", gap: 16, margin: { top: 6 } }}>
        <HoloButton
          label="USE"
          wide
          disabled={!USABLE.has(item.kind)}
          onClick={() => use(slot)}
        />
      </node>
    </node>
  );
}

/** The weapons he carries: the equipped one marked, the others a click
 *  away; the ones he hasn't bought, dark. */
function Weapons() {
  const arms = useGame((g) => g.arms);
  const g = useGame();
  return (
    <node style={{ flexDirection: "column", gap: 8, margin: { top: 4 } }}>
      <text style={{ ...T.small, letterSpacing: 3 }}>WEAPONS</text>
      <node style={{ flexDirection: "row", gap: 10 }}>
        {WEAPONS.map((w: WeaponId) => {
          const owned = arms.owned.includes(w);
          const on = arms.weapon === w;
          return (
            <button
              key={w}
              onClick={() => owned && equip(w)}
              style={{
                width: 165,
                flexDirection: "column",
                gap: 2,
                padding: { horizontal: 10, vertical: 7 },
                border: 1,
                borderColor: on
                  ? C.white
                  : owned
                    ? C.cyanLine
                    : "rgba(127, 227, 255, 0.12)",
                backgroundColor: on ? "rgba(240, 253, 255, 0.08)" : C.clear,
                cursor: owned ? "pointer" : "default",
              }}
              hoverStyle={
                owned
                  ? { backgroundColor: "rgba(127, 227, 255, 0.1)" }
                  : undefined
              }
            >
              <text
                style={{
                  fontFamily: F.semibold,
                  fontSize: 16,
                  letterSpacing: 1.5,
                  color: on ? C.white : owned ? C.cyan : C.cyanDeep,
                  lineBreak: "noWrap",
                }}
              >
                {ITEMS[w].name.toUpperCase()}
              </text>
              <text
                style={{
                  fontFamily: F.regular,
                  fontSize: 13,
                  letterSpacing: 1.5,
                  color: on ? C.green : owned ? C.cyanDim : C.cyanDeep,
                  lineBreak: "noWrap",
                }}
              >
                {!owned
                  ? "NOT OWNED"
                  : `${on ? "EQUIPPED · " : ""}${arms.clip[w]}/${clipSize(g, w)} · ${reserve(g, w)}`}
              </text>
            </button>
          );
        })}
      </node>
    </node>
  );
}

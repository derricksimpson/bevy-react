import { useEffect, useState } from "react";
import { bevy } from "../bevy";
import { buy, retrieve, sell, stow } from "../game/act";
import {
  ITEM_IDS,
  ITEMS,
  PACK_SLOTS,
  countItem,
  credits,
  sellPrice,
  type ItemId,
} from "../game/items";
import { getGame, useGame } from "../game/store";
import { useDebug } from "../hooks";
import { cue } from "../sound";
import { C, T } from "../theme";
import { Holo } from "./Holo";
import { HoloButton, Rule, Shell, Tabs } from "./shared/kit";
import { Detail, ItemRow, List } from "./store/parts";

const TABS = [
  { id: "buy", label: "BUY" },
  { id: "sell", label: "SELL" },
  { id: "safe", label: "SAFE" },
] as const;
type Tab = (typeof TABS)[number]["id"];

/** What the store sells: everything with a price that isn't salvage, and
 *  the weapons he doesn't own yet. */
function catalog(owned: string[]): ItemId[] {
  return ITEM_IDS.filter((id) => {
    const item = ITEMS[id];
    if (item.price === 0 || item.kind === "valuable") return false;
    return item.kind !== "weapon" || !owned.includes(id);
  });
}

/** The store kiosk's hologram (beat 4): buy, sell, and the safe that keeps
 *  what the suit can't carry. Credits up top in amber. */
export function Store() {
  const active = useGame((g) => g.beat) === 4;
  const [tab, setTab] = useState<Tab>("buy");
  const [item, setItem] = useState<ItemId>("medpack-s");
  const [slot, setSlot] = useState(0);
  const pack = useGame((g) => g.pack);

  // The turntable shows what this page is about while the store is up.
  const shown =
    tab === "buy" ? item : tab === "sell" ? pack.slots[slot]?.item : undefined;
  useEffect(() => {
    if (active && shown) bevy.preview.show({ item: shown });
  }, [active, shown]);

  const pick = (next: Tab) => {
    if (next === tab) return;
    setTab(next);
    cue("tab");
  };
  useDebug("store", (id) => {
    if (TABS.some((t) => t.id === id)) setTab(id as Tab);
  });
  useDebug("select", (id) => {
    if (!(id in ITEMS)) return;
    setTab("buy");
    setItem(id as ItemId);
  });
  useDebug("buy", (id) => {
    if (!(id in ITEMS)) return;
    setTab("buy");
    setItem(id as ItemId);
    buy(id as ItemId);
  });
  useDebug("sell", (n) => {
    const at = Number(n);
    if (!getGame().pack.slots[at]) return;
    setTab("sell");
    setSlot(at);
    sell(at);
  });

  return (
    <Holo name="store" page={tab} seed={5}>
      <Shell>
        <node style={{ flexDirection: "row", alignItems: "center", gap: 18 }}>
          <node style={{ flexDirection: "column", gap: 3 }}>
            <text style={{ ...T.header, fontSize: 28, letterSpacing: 8 }}>
              STORE
            </text>
            <text style={{ ...T.small, letterSpacing: 2 }}>
              CONCOURSE C · HALDEN SUPPLY TERMINAL 07
            </text>
          </node>
          <node style={{ flexGrow: 1 }} />
          <node style={{ flexDirection: "column", alignItems: "end", gap: 1 }}>
            <text style={{ ...T.small, letterSpacing: 3 }}>CREDITS</text>
            <text style={{ ...T.number, fontSize: 42, color: C.amber }}>
              {credits(pack.credits)}
            </text>
          </node>
        </node>
        <node
          style={{
            flexDirection: "row",
            justifyContent: "spaceBetween",
            alignItems: "center",
            margin: { top: 10 },
          }}
        >
          <Tabs tabs={TABS} active={tab} onPick={pick} />
          <text style={{ ...T.small, letterSpacing: 2 }}>WHEEL · SCROLL</text>
        </node>
        <Rule />
        <node
          style={{
            flexDirection: "row",
            gap: 30,
            flexGrow: 1,
            margin: { top: 16 },
          }}
        >
          {tab === "buy" && <Buy item={item} live={active} onItem={setItem} />}
          {tab === "sell" && (
            <Sell slot={slot} live={active} onSlot={setSlot} />
          )}
          {tab === "safe" && <Safe />}
        </node>
      </Shell>
    </Holo>
  );
}

function Buy({
  item,
  live,
  onItem,
}: {
  item: ItemId;
  live: boolean;
  onItem: (item: ItemId) => void;
}) {
  const g = useGame();
  const items = catalog(g.arms.owned);
  const price = ITEMS[item].price;
  const afford = price <= g.pack.credits;
  // Just bought: a weapon goes to his hands, not the pack.
  const owned = (g.arms.owned as string[]).includes(item);
  return (
    <>
      <List>
        {items.map((id) => (
          <ItemRow
            key={id}
            item={id}
            on={id === item}
            value={credits(ITEMS[id].price)}
            valueColor={
              ITEMS[id].price <= g.pack.credits ? C.amber : C.amberDim
            }
            onClick={() => {
              onItem(id);
              cue("tick");
            }}
          />
        ))}
      </List>
      <Detail item={item} live={live}>
        <node style={{ flexDirection: "row", gap: 34 }}>
          <Figure
            label="PRICE"
            value={credits(price)}
            color={afford ? C.amber : C.red}
          />
          <Figure
            label="IN PACK"
            value={
              owned
                ? "OWNED"
                : String(
                    ITEMS[item].kind === "weapon" ? 0 : countItem(g.pack, item),
                  )
            }
          />
        </node>
        <node style={{ flexDirection: "row", alignItems: "center", gap: 18 }}>
          {owned ? (
            <text style={{ ...T.small, letterSpacing: 2, color: C.green }}>
              OWNED · Q SWAPS TO IT IN THE WEAPON BEAT
            </text>
          ) : (
            <HoloButton
              label="BUY"
              wide
              tone={C.amber}
              onClick={() => buy(item)}
            />
          )}
          {!owned && !afford && (
            <text style={{ ...T.small, letterSpacing: 2, color: C.red }}>
              INSUFFICIENT CREDITS
            </text>
          )}
        </node>
      </Detail>
    </>
  );
}

function Sell({
  slot,
  live,
  onSlot,
}: {
  slot: number;
  live: boolean;
  onSlot: (slot: number) => void;
}) {
  const slots = useGame((g) => g.pack.slots);
  const s = slots[slot];
  return (
    <>
      <List>
        {slots.map((x, i) =>
          x ? (
            <ItemRow
              key={i}
              item={x.item}
              count={x.count}
              on={i === slot}
              value={credits(sellPrice(x.item))}
              onClick={() => {
                onSlot(i);
                cue("tick");
              }}
            />
          ) : null,
        )}
      </List>
      <Detail item={s?.item} live={live}>
        {s && (
          <>
            <node style={{ flexDirection: "row", gap: 34 }}>
              <Figure
                label="PAYS"
                value={credits(sellPrice(s.item))}
                color={C.amber}
              />
              <Figure label="CARRIED" value={String(s.count)} />
            </node>
            <HoloButton
              label="SELL ONE"
              wide
              tone={C.amber}
              onClick={() => sell(slot)}
            />
          </>
        )}
      </Detail>
    </>
  );
}

/** The pack and the safe side by side: click a slot to move it across. */
function Safe() {
  const pack = useGame((g) => g.pack);
  const used = pack.slots.filter(Boolean).length;
  return (
    <>
      <node style={{ flexDirection: "column", gap: 10 }}>
        <Heading
          text={`SUIT · ${used} / ${PACK_SLOTS}`}
          hint="CLICK TO STORE →"
        />
        <List width={590}>
          {pack.slots.map((x, i) =>
            x ? (
              <ItemRow
                key={i}
                item={x.item}
                count={x.count}
                on={false}
                value="›"
                valueColor={C.cyan}
                onClick={() => stow(i)}
              />
            ) : null,
          )}
        </List>
      </node>
      <node style={{ flexDirection: "column", gap: 10 }}>
        <Heading
          text={`SAFE · ${pack.safe.length} STACKS`}
          hint="← CLICK TO TAKE"
        />
        <List width={590}>
          {pack.safe.map((x, i) => (
            <ItemRow
              key={x.item}
              item={x.item}
              count={x.count}
              on={false}
              value="‹"
              valueColor={C.cyan}
              onClick={() => retrieve(i)}
            />
          ))}
        </List>
      </node>
    </>
  );
}

function Heading({ text, hint }: { text: string; hint: string }) {
  return (
    <node
      style={{
        flexDirection: "row",
        justifyContent: "spaceBetween",
        width: 590,
      }}
    >
      <text style={{ ...T.small, letterSpacing: 3, color: C.cyan }}>
        {text}
      </text>
      <text style={{ ...T.small, letterSpacing: 2 }}>{hint}</text>
    </node>
  );
}

function Figure({
  label,
  value,
  color = C.cyan,
}: {
  label: string;
  value: string;
  color?: string;
}) {
  return (
    <node style={{ flexDirection: "column", gap: 2 }}>
      <text style={{ ...T.small, letterSpacing: 2 }}>{label}</text>
      <text style={{ ...T.number, fontSize: 28, color }}>{value}</text>
    </node>
  );
}

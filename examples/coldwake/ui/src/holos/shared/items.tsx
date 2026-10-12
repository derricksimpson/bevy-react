import { ITEMS, type ItemId, type ItemKind } from "../../game/items";
import { C, F } from "../../theme";

/** How the menus name each kind of item. */
export const KIND_LABEL: Record<ItemKind, string> = {
  health: "MEDICAL",
  stasis: "STASIS",
  air: "LIFE SUPPORT",
  ammo: "AMMUNITION",
  upgrade: "UPGRADE",
  valuable: "VALUABLE",
  weapon: "WEAPON",
};

/** "Arc Cutter Cells" → "ACC": what an icon shows until its snapshot
 *  lands. */
function initials(item: ItemId): string {
  return ITEMS[item].name
    .split(" ")
    .map((w) => w[0])
    .join("")
    .slice(0, 3)
    .toUpperCase();
}

/** An item's icon: its `icon-<id>` snapshot (filmed by `preview/`) over
 *  its initials, in a frame; an optional count in the corner. */
export function ItemIcon({
  item,
  size = 88,
  count,
  lit = false,
}: {
  item: ItemId;
  size?: number;
  count?: number;
  lit?: boolean;
}) {
  return (
    <node
      style={{
        width: size,
        height: size,
        flexShrink: 0,
        alignItems: "center",
        justifyContent: "center",
        border: 1,
        borderColor: lit ? C.white : "rgba(127, 227, 255, 0.28)",
        backgroundColor: "rgba(6, 26, 38, 0.95)",
      }}
    >
      <text
        style={{
          fontFamily: F.display,
          fontSize: size * 0.22,
          color: "rgba(127, 227, 255, 0.3)",
        }}
      >
        {initials(item)}
      </text>
      <portal
        target={`icon-${item}`}
        style={{
          positionType: "absolute",
          left: 0,
          top: 0,
          width: "100%",
          height: "100%",
        }}
      />
      {count !== undefined && count > 1 && (
        <text
          style={{
            positionType: "absolute",
            right: 5,
            bottom: 2,
            fontFamily: F.display,
            fontSize: 15,
            color: lit ? C.white : C.cyan,
          }}
        >
          {count}
        </text>
      )}
    </node>
  );
}

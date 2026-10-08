import type { Choice } from "../bevy";
import { endTyping } from "../hooks";
import { Icon } from "../icons";
import { Menu, menuHeight } from "../menus/Menu";
import { openPopup } from "../menus/popup";
import { C } from "../theme";

/** A dropdown: the current choice; a press opens the list under it. */
export function EnumField({
  label,
  value,
  options,
  z,
  onChange,
}: {
  label: string;
  value: string;
  options: Choice[];
  z: number;
  onChange: (value: string) => void;
}) {
  const current = options.find((o) => o.id === value) ?? options[0];
  const items = options.map((o) => ({
    label: o.label,
    checked: o.id === current.id,
    onSelect: () => onChange(o.id),
  }));
  return (
    <node
      style={{
        flexGrow: 1,
        height: 18 * z,
        borderRadius: 4 * z,
        backgroundColor: "#282828",
        border: 1,
        borderColor: "#3a3a3a",
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "spaceBetween",
        padding: { horizontal: 6 * z },
      }}
      hoverStyle={{ backgroundColor: "#323232" }}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        endTyping();
        openPopup({
          x: e.clientX - 20,
          y: e.clientY + 10,
          w: 190,
          h: menuHeight(items) + 20,
          render: (close) => (
            <Menu title={label} items={items} close={close} width={190} />
          ),
        });
      }}
    >
      <text style={{ fontSize: 11.5 * z, color: C.text, lineBreak: "noWrap" }}>
        {current.label}
      </text>
      <Icon name="chevronDown" size={10 * z} color={C.textDim} />
    </node>
  );
}

/** A checkbox with its label. */
export function BoolField({
  label,
  value,
  z,
  onChange,
}: {
  label: string;
  value: boolean;
  z: number;
  onChange: (value: boolean) => void;
}) {
  return (
    <node
      style={{
        flexGrow: 1,
        flexDirection: "row",
        alignItems: "center",
        gap: 6 * z,
      }}
      onClick={() => onChange(!value)}
    >
      <node
        style={{
          width: 14 * z,
          height: 14 * z,
          borderRadius: 3 * z,
          backgroundColor: value ? C.fill : C.field,
          alignItems: "center",
          justifyContent: "center",
        }}
        hoverStyle={{ backgroundColor: value ? C.fillHover : C.fieldHover }}
      >
        {value && <Icon name="check" size={11 * z} stroke={2} />}
      </node>
      <text style={{ fontSize: 11.5 * z, color: C.text, lineBreak: "noWrap" }}>
        {label}
      </text>
    </node>
  );
}

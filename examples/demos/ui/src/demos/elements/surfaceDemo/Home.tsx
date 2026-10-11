import { BevyStyle } from "bevy-react/jsx";
import { Checkbox } from "@/components";
import { Colors, Fonts, FontSizes } from "@/theme";

type Props = {
  crt: boolean;
  setCrt: (v: boolean) => void;
};

export function Home({ crt, setCrt }: Props) {
  return (
    <node style={home}>
      <text style={brand}>bevy-react OS</text>
      <text style={brandSub}>
        bevy_ui rendered with React, live on a 3D screen
      </text>

      <node style={controls}>
        <Checkbox label="CRT effect" enabled={crt} onChange={setCrt} />
      </node>

      {/* A plain scroll container: the in-world pointer carries the wheel. */}
      <node style={processes} scrollStep={30}>
        {PROCESSES.map(([name, load]) => (
          <node key={name} style={processRow}>
            <text style={processName}>{name}</text>
            <text style={processLoad}>{load}</text>
          </node>
        ))}
      </node>
    </node>
  );
}

// The "running processes": bevy-react's own systems, scrolled with the wheel.
const PROCESSES = [
  ["apply_js_ops", "2.1%"],
  ["ui_layout_system", "4.8%"],
  ["drive_transitions", "0.9%"],
  ["drive_surface_pointer", "0.3%"],
  ["extract_ui_layers", "0.7%"],
  ["ui_layer_capture_pass", "1.6%"],
  ["redistribute_ui_layers", "0.4%"],
  ["apply_scroll", "0.1%"],
  ["collect_wheel_events", "0.1%"],
  ["resolve_filter_chains", "0.2%"],
  ["animation_apply", "0.5%"],
  ["op_next_event", "0.2%"],
] as const;

const home: BevyStyle = {
  flexGrow: 1,
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  gap: 14,
};

const brand: BevyStyle = {
  fontFamily: Fonts.display,
  color: Colors.text,
  fontSize: FontSizes.display,
  fontWeight: "semibold",
  letterSpacing: -0.5,
};

const brandSub: BevyStyle = {
  color: Colors.textBody,
  fontSize: FontSizes.lg,
};

const controls: BevyStyle = {
  flexDirection: "row",
  alignItems: "center",
  gap: 14,
  margin: { top: 10 },
};

const processes: BevyStyle = {
  width: 420,
  height: 150,
  flexDirection: "column",
  overflowY: "scroll",
  transition: { scroll: { duration: 160, easing: "easeOut" } },
  margin: { top: 18 },
  border: 1,
  borderColor: Colors.line,
  borderRadius: 10,
  backgroundColor: Colors.card,
};

const processRow: BevyStyle = {
  flexDirection: "row",
  justifyContent: "spaceBetween",
  flexShrink: 0,
  padding: { horizontal: 14, vertical: 7 },
  border: { bottom: 1 },
  borderColor: Colors.line,
};

const processName: BevyStyle = {
  color: Colors.textBody,
  fontSize: FontSizes.sm,
  fontFamily: Fonts.mono,
};

const processLoad: BevyStyle = {
  color: Colors.cyan,
  fontSize: FontSizes.sm,
  fontFamily: Fonts.mono,
};

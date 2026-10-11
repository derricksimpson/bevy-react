import { useEffect, useState } from "react";
import { LOGS, type Log } from "../../game/lore";
import { useGame } from "../../game/store";
import { C, F, T } from "../../theme";
import { HoloButton, Rule } from "../shared/kit";

/** The logs: text and audio logs found on the deck. The list scrolls under
 *  the wheel; an audio log plays its garbled transmission over a waveform
 *  and shows the transcript. */
export function Logs({
  selected,
  playing,
  onSelect,
  onPlay,
}: {
  selected: string;
  playing: boolean;
  onSelect: (id: string) => void;
  onPlay: () => void;
}) {
  const read = useGame((g) => g.story.read);
  const log = LOGS.find((l) => l.id === selected) ?? LOGS[0];
  return (
    <node style={{ flexDirection: "row", gap: 28, flexGrow: 1 }}>
      <node
        scrollStep={36}
        style={{
          width: 430,
          flexShrink: 0,
          height: 476,
          flexDirection: "column",
          gap: 8,
          overflowY: "scroll",
          transition: { scroll: { duration: 180, easing: "easeOut" } },
        }}
      >
        {LOGS.map((l) => (
          <Row
            key={l.id}
            log={l}
            on={l.id === log.id}
            unread={!read.includes(l.id)}
            onClick={() => onSelect(l.id)}
          />
        ))}
      </node>
      <node style={{ flexDirection: "column", gap: 12, flexGrow: 1 }}>
        <text
          style={{
            ...T.small,
            letterSpacing: 3,
            color: log.audio ? C.amber : C.cyanDim,
          }}
        >
          {log.audio ? "AUDIO LOG" : "TEXT LOG"}
        </text>
        <text style={{ ...T.header, fontSize: 26, color: C.white }}>
          {log.title}
        </text>
        <text style={{ ...T.label, fontSize: 17, color: C.cyanDim }}>
          {`FROM: ${log.from.toUpperCase()}`}
        </text>
        <Rule />
        {log.audio && (
          <node style={{ flexDirection: "row", alignItems: "center", gap: 20 }}>
            <HoloButton label={playing ? "PLAYING" : "PLAY"} onClick={onPlay} />
            <Waveform playing={playing} />
          </node>
        )}
        <text style={{ ...T.body, fontSize: 20, lineHeight: 1.5 }}>
          {log.audio ? `"${log.text}"` : log.text}
        </text>
      </node>
    </node>
  );
}

function Row({
  log,
  on,
  unread,
  onClick,
}: {
  log: Log;
  on: boolean;
  unread: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        flexShrink: 0,
        flexDirection: "row",
        alignItems: "center",
        gap: 14,
        height: 96,
        padding: { horizontal: 14 },
        border: 1,
        borderColor: on ? C.white : "rgba(127, 227, 255, 0.2)",
        backgroundColor: on
          ? "rgba(240, 253, 255, 0.08)"
          : "rgba(4, 18, 28, 0.4)",
        cursor: "pointer",
      }}
      hoverStyle={{ backgroundColor: "rgba(127, 227, 255, 0.1)" }}
    >
      <node
        style={{
          width: 54,
          height: 26,
          alignItems: "center",
          justifyContent: "center",
          border: 1,
          borderColor: log.audio ? C.amber : C.cyanDim,
        }}
      >
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 12,
            letterSpacing: 1.5,
            color: log.audio ? C.amber : C.cyanDim,
          }}
        >
          {log.audio ? "AUDIO" : "TEXT"}
        </text>
      </node>
      <node
        style={{ flexDirection: "column", gap: 2, flexGrow: 1, flexShrink: 1 }}
      >
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 18,
            letterSpacing: 1.5,
            color: on ? C.white : C.cyan,
            lineBreak: "noWrap",
          }}
        >
          {log.title}
        </text>
        <text style={{ ...T.small, lineBreak: "noWrap" }}>
          {log.from.toUpperCase()}
        </text>
      </node>
      {unread && (
        <node
          style={{
            width: 9,
            height: 9,
            borderRadius: 5,
            backgroundColor: C.cyan,
          }}
        />
      )}
    </button>
  );
}

/** Cheap, repeatable noise: 0..1 for bar `i` at `frame`. */
function noise(i: number, frame: number): number {
  const s = Math.sin(i * 12.9898 + frame * 78.233) * 43758.5453;
  return s - Math.floor(s);
}

/** The transmission's waveform: bars that jump while it plays, a flat
 *  line of static when it doesn't. */
function Waveform({ playing }: { playing: boolean }) {
  const [frame, setFrame] = useState(0);
  useEffect(() => {
    if (!playing) return;
    const t = setInterval(() => setFrame((f) => f + 1), 70);
    return () => clearInterval(t);
  }, [playing]);
  const width = 560;
  const height = 96;
  return (
    <canvas
      style={{ width, height }}
      draw={(ctx) => {
        const bars = 56;
        const step = width / bars;
        ctx.fillStyle = playing ? C.cyan : C.cyanDeep;
        ctx.beginPath();
        for (let i = 0; i < bars; i++) {
          // A garbled voice: syllables (the slow envelope) over noise,
          // with dropouts.
          const env = Math.abs(Math.sin(i * 0.23 + frame * 0.41)) * 0.8 + 0.2;
          const drop = noise(i >> 3, frame >> 2) < 0.18 ? 0.1 : 1;
          const h = playing
            ? Math.max(3, noise(i, frame) * env * drop * (height - 8))
            : 2 + noise(i, 0) * 3;
          ctx.rect(i * step + 1, (height - h) / 2, step - 3, h);
        }
        ctx.fill();
      }}
    />
  );
}

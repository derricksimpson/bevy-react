import { useGame } from "../game/store";
import { C } from "../theme";
import { Holo } from "./Holo";
import { useBlink } from "./suit/loops";

/** Segments down the spine; the top ones go out first. */
const SEGMENTS = 10;
const PITCH = 35;
const TOP = 6;

/** A vertebra: a chevron pointing down, `y` its top edge. */
function vertebra(y: number): number[] {
  return [5, y, 43, y, 43, y + 20, 24, y + 30, 5, y + 20];
}

/** The suit's vitals running down the engineer's spine (48 × 360): cyan
 *  while healthy, amber at 40%, red and pulsing at 20%. */
export function Spine() {
  return (
    <Holo name="spine" seed={3}>
      <SpineBar />
    </Holo>
  );
}

function SpineBar() {
  const health = useGame((g) => g.suit.health);
  const color = health > 0.4 ? C.cyan : health > 0.2 ? C.amber : C.red;
  const pulse = useBlink(health <= 0.2, 380, 0.3);
  // In segments, from the bottom: whole ones lit, the next one partly.
  const level = health * SEGMENTS;

  return (
    <svg viewBox="0 0 48 360" style={{ width: 48, height: 360 }}>
      <line
        x1={24}
        y1={2}
        x2={24}
        y2={358}
        stroke={C.cyanDim}
        strokeWidth={1.5}
        opacity={0.55}
      />
      <rect x={14} y={1} width={20} height={2} fill={C.cyanDim} />
      <rect x={14} y={357} width={20} height={2} fill={C.cyanDim} />
      {Array.from({ length: SEGMENTS }, (_, i) => {
        const fromBottom = SEGMENTS - 1 - i;
        if (fromBottom < level) return null;
        return (
          <polygon
            key={`off${i}`}
            points={vertebra(TOP + i * PITCH)}
            fill="none"
            stroke={C.cyanDim}
            strokeWidth={1.2}
            opacity={0.35}
          />
        );
      })}
      <g opacity={{ animated: pulse }}>
        {Array.from({ length: SEGMENTS }, (_, i) => {
          const fromBottom = SEGMENTS - 1 - i;
          const lit = Math.min(1, level - fromBottom);
          if (lit <= 0) return null;
          const y = TOP + i * PITCH;
          return (
            <g key={`on${i}`} opacity={0.3 + 0.7 * lit}>
              <polygon points={vertebra(y)} fill={color} opacity={0.9} />
              <rect x={12} y={y + 5} width={24} height={1.5} fill={C.white} />
            </g>
          );
        })}
      </g>
    </svg>
  );
}

import { C } from "../../theme";

/** The freight lift's stops (Rust's elevator only knows the numbers). */
export const FLOORS = [
  { floor: 1, name: "HYDROPONICS" },
  { floor: 2, name: "CONCOURSE C" },
  { floor: 3, name: "ICE REFINERY" },
  { floor: 4, name: "COMMAND" },
] as const;

/** A solid triangle pointing up or down. */
export function Arrow({
  up,
  size,
  color = C.amber,
}: {
  up: boolean;
  size: number;
  color?: string;
}) {
  return (
    <svg viewBox="0 0 12 10" style={{ width: size, height: (size * 10) / 12 }}>
      <polygon
        points={up ? [6, 0, 12, 10, 0, 10] : [0, 0, 12, 0, 6, 10]}
        fill={color}
      />
    </svg>
  );
}

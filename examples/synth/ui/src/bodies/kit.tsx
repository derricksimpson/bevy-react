import type { ReactNode } from "react";
import type { SharedValue } from "bevy-react";
import type { GraphNode } from "../graph/model";
import { r } from "../theme";

/** What each module shows under its sockets. Everything that moves runs on
 *  the step loop in Bevy (`stepLight`, `Scope`): React renders only when a
 *  knob turns. */

export type BodyProps = {
  node: GraphNode;
  set: (key: string, value: number) => void;
  color: string;
  steps: SharedValue;
};

export const Col = ({
  children,
  gap = 8,
}: {
  children: ReactNode;
  gap?: number;
}) => (
  <node
    style={{
      flexGrow: 1,
      flexDirection: "column",
      justifyContent: "center",
      gap: r(gap),
    }}
  >
    {children}
  </node>
);
export const Row = ({
  children,
  gap = 10,
}: {
  children: ReactNode;
  gap?: number;
}) => (
  <node
    style={{
      flexDirection: "row",
      justifyContent: "center",
      alignItems: "center",
      gap: r(gap),
    }}
  >
    {children}
  </node>
);

export const pct = (v: number) => `${Math.round(v * 100)}`;

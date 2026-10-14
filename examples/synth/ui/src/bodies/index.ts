import type { ReactNode } from "react";
import type { Kind } from "../graph/catalog";
import { Beat, Drum } from "./drums";
import { Delay, Mixer, Output, Reverb } from "./effects";
import type { BodyProps } from "./kit";
import { Clock, Sequencer } from "./rhythm";
import { Envelope, Filter, Glide, Vca } from "./shapers";
import { Lfo, Noise, Oscillator } from "./sources";

export const BODIES: Record<Kind, (p: BodyProps) => ReactNode> = {
  clock: Clock,
  sequencer: Sequencer,
  beat: Beat,
  oscillator: Oscillator,
  noise: Noise,
  lfo: Lfo,
  drum: Drum,
  envelope: Envelope,
  filter: Filter,
  vca: Vca,
  glide: Glide,
  mixer: Mixer,
  delay: Delay,
  reverb: Reverb,
  output: Output,
};

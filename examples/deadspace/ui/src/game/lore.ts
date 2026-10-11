/** The story the showroom hints at. Eurydice Station mines ice on a gas
 *  giant's moon for Halden Ice & Ore; something stopped the shift change on
 *  Concourse C. All names and text here are our own. */

/** The objective chain, in order, and where each leads (the map's beam,
 *  the locator). Which one is current follows from the game (`objective`
 *  in `reduce.ts`): something bought, a node in the cutter's circuit, the
 *  bulkhead open. The last has no end. */
export const OBJECTIVES = [
  {
    to: "store",
    title: "RESUPPLY",
    text: "The maintenance store on Concourse C still has power. Buy what the suit needs.",
  },
  {
    to: "bench",
    title: "UPGRADE THE ARC CUTTER",
    text: "Power a node in the cutter's circuit at the upgrade bench.",
  },
  {
    to: "bulkhead",
    title: "OVERRIDE THE BULKHEAD",
    text: "The way to Concourse D is sealed. Bypass the lock at its hack panel.",
  },
  {
    to: "elevator",
    title: "REACH THE REFINERY",
    text: "Ride the freight elevator down to deck 3. Someone there is still transmitting.",
  },
] as const;

export type Log = {
  id: string;
  title: string;
  from: string;
  /** Audio logs play a garbled transmission (the `log` cue); the text is
   *  their transcript. */
  audio: boolean;
  text: string;
};

export const LOGS: Log[] = [
  {
    id: "shift-roster",
    title: "SHIFT ROSTER 7-C",
    from: "Concourse C Supervisor",
    audio: false,
    text:
      "Night crew short four again. Petrov and the Wan sisters reported to medical with the headaches, " +
      "Abara never clocked in. Ice output down 30% on the week. If Halden asks, it's the drills.",
  },
  {
    id: "maintenance-note",
    title: "WORK ORDER 2291",
    from: "Facilities",
    audio: false,
    text:
      "Bulkhead C-D lock rejecting every badge since 02:14. Manual override sits behind the panel " +
      "left of the frame: three tumblers, rotary. Do NOT force it while the panel is live.",
  },
  {
    id: "store-ledger",
    title: "STORE LEDGER",
    from: "Concourse C Store",
    audio: false,
    text:
      "Every medpack in the district sold in six hours. Air canisters gone by noon. " +
      "Nobody bought ammunition. Nobody needed to, yet.",
  },
  {
    id: "okafor-1",
    title: "MESSAGE FROM DECK 3",
    from: "Dr. Ines Varga",
    audio: true,
    text:
      "Okafor, if this reaches you — don't trust the elevator logs. The refinery isn't empty. " +
      "We sealed ourselves in the control room. Bring cutter cells. Bring all of them.",
  },
  {
    id: "okafor-2",
    title: "HALDEN PRIORITY",
    from: "Halden Ice & Ore",
    audio: true,
    text:
      "Engineer T. Okafor: your contract obliges you to restore Concourse D access. " +
      "Station quarantine is a precaution. Production resumes at the next shift.",
  },
];

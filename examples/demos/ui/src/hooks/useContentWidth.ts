import { Responsiveness } from "../theme";
import { useIsMobile } from "./useIsMobile";
import { useWindowSize } from "./useWindowSize";

/** The content column's width in px — the window less the sidebar and the
 *  column's padding, capped. Computed here rather than with `width: "100%"` +
 *  `maxWidth` because Bevy 0.19 under-measured wrapped text below that pair;
 *  Bevy 0.20 measures it right, so new code can use the pair. */
export function useContentWidth(): number {
  const win = useWindowSize();
  const isMobile = useIsMobile();
  const padding = isMobile
    ? Responsiveness.contentPaddingMobile
    : Responsiveness.contentPadding;
  const available =
    win.width - (isMobile ? 0 : Responsiveness.navWidth) - padding * 2;
  return Math.max(0, Math.min(Responsiveness.contentMaxWidth, available));
}

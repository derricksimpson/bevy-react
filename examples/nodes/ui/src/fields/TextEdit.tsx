import { useEffect, useRef, useState } from "react";
import type { BevyStyle } from "bevy-react";
import { bevy } from "../bevy";
import { startTyping } from "../hooks";
import { C } from "../theme";

/** Typing into a field: an autofocused `<editableText>` with its text
 *  selected. Enter or a press elsewhere commits (`onDone(text)`), Escape
 *  cancels (`onDone(null)`); the field then unmounts it. */
export function TextEdit({
  initial,
  onDone,
  style,
}: {
  initial: string;
  onDone: (text: string | null) => void;
  style?: BevyStyle;
}) {
  const [text, setText] = useState(initial);
  const latest = useRef({ text, onDone, done: false });
  latest.current.text = text;
  latest.current.onDone = onDone;
  const finish = (value: string | null) => {
    if (latest.current.done) return;
    latest.current.done = true;
    latest.current.onDone(value);
  };

  useEffect(() => {
    const stop = startTyping(() => finish(latest.current.text));
    const off = bevy.on("keyDown", (e) => {
      if (e.key === "Enter") finish(latest.current.text);
      else if (e.key === "Escape") finish(null);
    });
    return () => {
      stop();
      off();
    };
    // `finish` reads everything through the ref.
  }, []);

  return (
    <editableText
      autofocus
      value={text}
      onChange={setText}
      onBlur={() => finish(latest.current.text)}
      selectionStart={0}
      selectionEnd={initial.length}
      style={{
        flexGrow: 1,
        color: C.text,
        backgroundColor: C.fieldDark,
        ...style,
      }}
    />
  );
}

// Unit test for the Animations tab's field labels (`boundFields` in
// `js/src/devtools/mirror.ts`): the wire paths in a mirrored node whose
// `{ animated }` wrapper reads a given shared value.
//
// Same esbuild-on-the-fly rig as warnings.test.mjs (extensionless TS imports).
//
// Run: npm test -w bevy-react

import { test } from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { build } from "esbuild";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const entry = join(
  dirname(fileURLToPath(import.meta.url)),
  "../src/devtools/mirror.ts",
);
const bundled = await build({
  entryPoints: [entry],
  bundle: true,
  format: "esm",
  write: false,
  logLevel: "silent",
});
const code = Buffer.from(bundled.outputFiles[0].contents).toString("base64");
const { boundFields } = await import(`data:text/javascript;base64,${code}`);

test("finds every base-style and attribute path binding the value", () => {
  const sv = { id: 3, value: 0 };
  const node = {
    id: 1,
    kind: "node",
    style: {
      opacity: { animated: sv },
      width: { animated: { id: 4, value: 0 } },
      transform: { translateX: { animated: { type: "shared", id: 3 } } },
      filter: [{ name: "blur", params: { radius: { animated: sv, seed: 8 } } }],
    },
    // A wrapper in a style variant never binds (Rust ignores it).
    props: { cx: { animated: sv }, hoverStyle: { opacity: { animated: sv } } },
    children: [],
    parent: null,
    devtools: false,
  };
  assert.deepEqual(boundFields(node, 3), [
    "opacity",
    "transform.translateX",
    "filter[0].params.radius",
    "cx",
  ]);
  assert.deepEqual(boundFields(node, 4), ["width"]);
});

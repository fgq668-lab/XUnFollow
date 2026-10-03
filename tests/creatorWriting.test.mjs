import test from "node:test";
import assert from "node:assert/strict";
import { selectedThemes, publishPreview, wordingHints } from "../src/creatorWriting.ts";
test("multiple presets and custom themes are deduplicated and bounded", () => {
  assert.deepEqual(selectedThemes(["暴论", "创业"], "创业，搞笑\n互关、擦边"), ["暴论", "创业", "搞笑", "互关", "擦边"]);
  assert.throws(() => selectedThemes([], ""));
  assert.throws(() => selectedThemes([], "字".repeat(25)));
  assert.throws(() => selectedThemes(Array.from({ length: 13 }, (_, i) => `主题${i}`), ""));
});
test("paste preview keeps native paragraphs and unique source links", () => {
  const url = "https://x.com/test/status/1";
  const d = { publishBody: "代码能跑。\n\n先别问为什么。", sources: [{ url }, { url }] };
  assert.equal(publishPreview(d, false), d.publishBody);
  assert.equal(publishPreview(d, true), `${d.publishBody}\n\n来源：\n${url}`);
  assert.equal(publishPreview({ ...d, publishBody: url }, true), url);
});
test("template hints are transparent wording advice, not an AI detection score", () => {
  assert.deepEqual(wordingHints("代码能跑。先别问为什么。"), []);
  assert.match(wordingHints("总的来说，AI赋能未来。值得深思。")[0], /总结稿/);
});

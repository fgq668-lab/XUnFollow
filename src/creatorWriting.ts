export function selectedThemes(presets: string[], custom: string): string[] {
  const values = [...new Set([...presets, ...custom.split(/[,，、;；\n]/)].map(x => x.trim()).filter(Boolean))];
  if (!values.length || values.length > 12 || values.some(x => [...x].length > 24)) {
    throw new Error("请选择或填写1–12个创作主题，每个最多24字");
  }
  return values;
}
/** Body has already been laid out by the native formatter; never recompute it in the browser. */
export function publishPreview(draft: { publishBody: string; sources: { url: string }[] }, includeSources: boolean): string {
  const urls = includeSources ? [...new Set(draft.sources.map(s => s.url).filter(url => !draft.publishBody.includes(url)))] : [];
  return draft.publishBody + (urls.length ? `\n\n来源：\n${urls.join("\n")}` : "");
}
/** Wording hints, not an AI detector. They never block copy or alter the user's text. */
export function wordingHints(body: string): string[] {
  const patterns = ["赋能", "值得深思", "总的来说", "归根结底", "不禁感叹", "让我们", "未来属于", "你有没有发现", "在这个时代", "随着时代", "不可否认"];
  const found = patterns.filter(s => body.includes(s));
  if (found.length) return [`有些词像总结稿：${found.slice(0, 4).join("、")}。可以换成具体的小事，或直接删掉结尾总结。`];
  return [];
}

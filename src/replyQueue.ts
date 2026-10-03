import type { ReplyPost } from "./types";

export function isChineseText(text: string): boolean {
  const words = text.split(/\s+/).filter(word => !/^(https?:\/\/|www\.|@)/i.test(word)).join(" ");
  const han = words.match(/[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/g)?.length ?? 0;
  const letters = words.match(/\p{L}/gu)?.length ?? 0;
  return han >= 4 && han * 2 >= letters;
}

export function replyQueue(posts: ReplyPost[], filter: string, query: string): ReplyPost[] {
  const term = query.trim().toLowerCase();
  return posts.filter(p => {
    const pending = p.status === "pending" || p.status === "copied";
    const match = filter === "pending" ? pending : filter === "ready" ? pending && !!p.draft : filter === "all" || p.status === filter;
    return match && (!term || `${p.username} ${p.postText} ${p.draft}`.toLowerCase().includes(term));
  }).sort((a, b) => Number(isChineseText(b.postText)) - Number(isChineseText(a.postText)));
}

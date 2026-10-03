import { test } from "node:test";
import assert from "node:assert/strict";
import { isChineseText, replyQueue } from "../src/replyQueue.ts";

const post = (postId, postText, status = "pending") => ({postId, postText, status, username:"friend", draft:"草稿"});
test("Chinese queue comes first, keeps stable order and preserves English backlog", () => {
  const posts = [post("en", "Building a new AI app today"), post("zh1", "今天做了一个 AI 小工具，继续一点点往前走"), post("zh2", "正在继续开发本地应用，慢慢把功能补齐")];
  assert.deepEqual(replyQueue(posts, "pending", "").map(p => p.postId), ["zh1", "zh2", "en"]);
  assert.equal(posts[0].postId, "en");
  assert.equal(isChineseText("An English post with a 中文 tag"), false);
  assert.equal(isChineseText("今天继续开发小工具 https://example.com/a-long-english-address"), true);
});
test("Replied and skipped posts never join the pending or ready queue", () => {
  const posts = [post("done", "今天做了一个小工具并已经回复", "replied"), post("skip", "这条中文帖子已经跳过处理", "skipped"), post("copied", "这条中文帖子刚刚复制草稿", "copied")];
  for (const filter of ["pending", "ready"]) assert.deepEqual(replyQueue(posts, filter, "").map(p => p.postId), ["copied"]);
  assert.deepEqual(replyQueue(posts, "replied", "").map(p => p.postId), ["done"]);
  assert.equal(replyQueue(posts, "all", "").length, 3);
});

import { invoke } from "@tauri-apps/api/core";
import type { Bootstrap, Candidate, CostEstimate, DecisionStatus, HistoryEntry, ReplyConfig, ReplyPersona, WorkbenchSnapshot } from "./types";

const native = "__TAURI_INTERNALS__" in window;

const demoCandidates: Candidate[] = [
  ["2001", "OooCoder", "OooCoder | AI Builder", 149, 985],
  ["2002", "zorvia", "Zorvia", 1248, 406],
  ["2003", "tinytools", "Tiny Tools", 4431, 219],
  ["2004", "makerlee", "Maker Lee", 92, 607],
  ["2005", "buildwithchen", "Chen", 823, 1102],
  ["2006", "product_sailor", "Product Sailor", 2021, 501],
  ["2007", "shipfast", "Ship Fast", 774, 421],
  ["2008", "openbuild", "Open Build", 344, 700],
  ["2009", "indiehacker", "Indie Hacker", 622, 603],
  ["2010", "frontendfox", "Frontend Fox", 515, 416],
  ["2011", "designloop", "Design Loop", 234, 911],
  ["2012", "launchdaily", "Launch Daily", 1202, 320],
].map(([stableXId, username, name, followersCount, followingCount]) => ({
  stableXId: String(stableXId),
  username: String(username),
  name: String(name),
  followersCount: Number(followersCount),
  followingCount: Number(followingCount),
  xUrl: `https://x.com/${username}`,
}));

const demoBootstrap: Bootstrap = {
  account: { username: "demo_account", name: "示例账号" },
  candidates: demoCandidates,
  decisions: {},
  history: [],
  summary: {
    capturedAt: new Date().toISOString(),
    followersCount: 9563,
    followingCount: 10546,
    nonFollowbackCount: demoCandidates.length,
    confirmedCostUsd: "0.000000",
    maximumPossibleCostUsd: "0.000000",
    complete: true,
  },
  dailyGoal: 10,
  batchSize: 10,
  apiKeyConfigured: false,
  syncRunning: false,
};
let demoState: Bootstrap = structuredClone(demoBootstrap);
const demoEvents: Array<{ stableXId: string; previous?: Bootstrap["decisions"][string] }> = [];
const localDay = () => new Intl.DateTimeFormat("en-CA", { timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone }).format(new Date());

export async function bootstrap(): Promise<Bootstrap> {
  if (native) return invoke<Bootstrap>("bootstrap");
  const snapshot = structuredClone(demoState);
  snapshot.history = Object.entries(snapshot.decisions)
    .map(([stableXId, decision]): HistoryEntry | undefined => {
      const candidate = snapshot.candidates.find((item) => item.stableXId === stableXId);
      return candidate ? { stableXId, username: candidate.username, name: candidate.name, ...decision } : undefined;
    })
    .filter((item): item is HistoryEntry => Boolean(item))
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
  return snapshot;
}

export async function saveApiKey(apiKey: string): Promise<void> {
  if (!native) { demoWorkbench.twitterKeyConfigured = Boolean(apiKey.trim()); return; }
  await invoke("save_api_key", { apiKey });
}

export async function estimateCost(handle: string, hardCapUsd: string): Promise<CostEstimate> {
  if (!native) {
    return { estimatedUsd: "0.15", hardCapUsd, assumption: `@${handle} 的示例预估；实际按 Provider 返回数量结算。` };
  }
  return invoke<CostEstimate>("estimate_cost", { handle, hardCapUsd });
}

export async function startScan(handle: string, hardCapUsd: string): Promise<void> {
  if (!native) return;
  await invoke("start_scan", { handle, hardCapUsd });
}

export async function resumeScanOnce(handle: string, hardCapUsd: string): Promise<void> {
  if (!native) return;
  await invoke("resume_scan_once", { handle, hardCapUsd, acknowledgeAmbiguousRetry: true });
}

export async function recordDecision(stableXId: string, status: Exclude<DecisionStatus, "pending">): Promise<void> {
  if (!native) {
    demoEvents.push({ stableXId, previous: demoState.decisions[stableXId] });
    demoState.decisions[stableXId] = { status, actionDay: localDay(), updatedAt: new Date().toISOString() };
    return;
  }
  await invoke("record_decision", { stableXId, status });
}

export async function undoLast(): Promise<void> {
  if (!native) {
    const event = demoEvents.pop();
    if (!event) return;
    if (event.previous) demoState.decisions[event.stableXId] = event.previous;
    else delete demoState.decisions[event.stableXId];
    return;
  }
  await invoke("undo_last");
}

export async function continueBatch(): Promise<void> {
  if (!native) {
    const complete = Object.values(demoState.decisions).filter((item) => item.actionDay === localDay()).length;
    if (complete < demoState.dailyGoal) throw new Error("请先完成当前这一批");
    demoState.dailyGoal += demoState.batchSize;
    return;
  }
  await invoke("continue_batch");
}

export async function importDecisions(decisions: Bootstrap["decisions"]): Promise<number> {
  if (!native) {
    let accepted = 0;
    for (const candidate of demoState.candidates) {
      const decision = decisions[candidate.stableXId];
      if (decision) { demoState.decisions[candidate.stableXId] = decision; accepted += 1; }
    }
    return accepted;
  }
  return invoke<number>("import_decisions", { decisions });
}

export async function openExternalProfile(url: string): Promise<void> {
  if (!native) {
    window.open(url, "xunfollow_profile", "noopener,noreferrer");
    return;
  }
  await invoke("open_external_profile", { url });
}

let demoWorkbench: WorkbenchSnapshot = {
  persona: { identity: "", topics: "", voice: "", language: "zh", avoid: "" },
  twitterKeyConfigured: false, deepseekKeyConfigured: false, posts: [], running: false, todayRepliedCount: 0,
};

export async function workbenchSnapshot(): Promise<WorkbenchSnapshot> {
  return native ? invoke<WorkbenchSnapshot>("workbench_snapshot") : structuredClone(demoWorkbench);
}

export async function saveReplyPersona(persona: ReplyPersona): Promise<void> {
  if (native) await invoke("save_reply_persona", { persona });
  else demoWorkbench.persona = persona;
}

export async function saveDeepseekKey(apiKey: string): Promise<void> {
  if (native) await invoke("save_deepseek_key", { apiKey });
  else demoWorkbench.deepseekKeyConfigured = Boolean(apiKey.trim());
}

export async function startReplyRun(config: ReplyConfig): Promise<void> {
  if (native) await invoke("start_reply_run", { config });
  else {
    demoWorkbench.run = { id: 1, day: localDay(), phase: "done", targetCount: config.targetCount, maxPages: 1, pagesDone: 1,
      xCapUsd: config.xCapUsd, aiCapUsd: config.aiCapUsd, xSpentUsd: "0.000000", aiSpentUsd: "0.000000", xUncertainUsd: "0.000000", aiUncertainUsd: "0.000000", aiReservedUsd: "0.000000", xEstimatedUsd: (Math.min(30, Math.max(1, Math.ceil(config.targetCount * 3 / 20))) * 0.003).toFixed(6), aiEstimatedUsd: "0.000000",
      candidateCount: 0, selectedCount: 0, draftedCount: 0, repliedCount: 0, needsExplicitRetry: false };
  }
}

export async function resumeReplyRun(): Promise<void> {
  if (native) await invoke("resume_reply_run", { acknowledgeUncertainCost: true });
}

export async function stopReplyRun(): Promise<void> {
  if (native) await invoke("stop_reply_run");
  else demoWorkbench.running = false;
}

export async function updateReplyPost(postId: string, draft?: string, status?: string): Promise<void> {
  if (native) await invoke("update_reply_post", { postId, draft: draft ?? null, status: status ?? null });
  else {
    const post = demoWorkbench.posts.find((item) => item.postId === postId);
    if (post) { if (draft !== undefined) post.draft = draft; if (status) post.status = status as typeof post.status; }
  }
}

export async function regenerateReplyPost(postId: string): Promise<void> {
  if (native) await invoke("regenerate_reply_post", { postId });
}

export async function increaseReplyAiCap(newCapUsd: string): Promise<void> {
  if (native) await invoke("increase_reply_ai_cap", { newCapUsd });
}

export async function createArticleDraft(topic: string, aiCapUsd: string): Promise<string> {
  if (native) return invoke<string>("create_article_draft", { topic, aiCapUsd });
  const body = `关于${topic}：在这里编辑你的文章草稿。`;
  demoWorkbench.article = { id: 1, topic, body, costUsd: "0.000000", capUsd: aiCapUsd };
  return body;
}

export async function saveArticleDraft(topic: string, body: string): Promise<void> {
  if (native) await invoke("save_article_draft", { topic, body });
  else demoWorkbench.article = { id: 1, topic, body, costUsd: demoWorkbench.article?.costUsd ?? "0.000000", capUsd: demoWorkbench.article?.capUsd ?? "0.000000" };
}

import { invoke } from "@tauri-apps/api/core";
import type { Bootstrap, Candidate, CostEstimate, DecisionStatus, HistoryEntry, ReplyConfig, ReplyPersona, WorkbenchSnapshot, WorkbenchPreferences } from "./types";

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
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  await invoke("open_external_profile", { url });
}

const creatorDefaults = { creatorPrompt: "像时间线上随口发一条，只说一个小细节，讲完就停，不总结不上价值。不虚构亲身经历或事实。按不同话题变换语气，正文纯文本、段间空一行。", creatorVoiceSamples: "", creatorDailyTarget: 5, creatorDayStart: "09:00", creatorDayEnd: "22:00", remindersEnabled: false, reminderSound: true };
let demoWorkbench: WorkbenchSnapshot = {
  persona: { identity: "", topics: "", voice: "", language: "zh", avoid: "" },
  twitterKeyConfigured: false, deepseekKeyConfigured: false, posts: [], running: false, todayRepliedCount: 0,
  totalRepliedCount: 0, pendingCount: 0, mutualCount: 0,
  defaultReplyPrompt: "只写一句约30字的自然口语，以轻轻鼓励为主，不分析、不表态、不感叹、不尬夸，不虚构经历。口头禅最多自然用一个，不要每条重复。只输出可粘贴的回复。",
  preferences: { ...creatorDefaults, replyPrompt: "只写一句约30字的自然口语，以轻轻鼓励为主，不分析、不表态、不感叹、不尬夸，不虚构经历。口头禅最多自然用一个，不要每条重复。只输出可粘贴的回复。", catchphrases: "哈、呗、慢慢来", model: "deepseek-flash", dailyTarget: 200, ownUsername: "demo_account", defaultXCapUsd: "0.25", defaultAiCapUsd: "0.10", openBatchSize: 3 },
};

import type { CreatorConfig, CreatorSnapshot } from "./types";
export const isNative = native;
export async function creatorSnapshot(): Promise<CreatorSnapshot> {
  if(native) return invoke("creator_snapshot");
  return { drafts: [], running: false, todayGoal: demoWorkbench.preferences.creatorDailyTarget, todayPublished: 0, totalPublished: 0, overdueCount: 0, preferences: structuredClone(demoWorkbench.preferences), defaultPrompt: creatorDefaults.creatorPrompt };
}
async function creatorCommand<T=void>(name:string,args?:Record<string,unknown>):Promise<T> {
  if(!native) throw new Error("浏览器仅演示界面；请在桌面应用使用创作功能，不会请求真实 API");
  return invoke<T>(name,args);
}
export const saveCreatorGoal=(target:number)=>creatorCommand("save_creator_goal",{target});
export const startCreatorRun=(config:CreatorConfig)=>creatorCommand("start_creator_run",{config});
export const restartCreatorRun=(config:CreatorConfig,expectedIds:number[])=>creatorCommand("restart_creator_run",{config,expectedIds});
export const referenceSnapshot=async()=>native?invoke<import("./types").ReferenceSnapshot>("creator_reference_snapshot"):{references:[],activeHandles:[],reading:false};
export const saveCreatorReferenceSelection=(handles:string[])=>creatorCommand("save_creator_reference_selection",{handles});
export const readCreatorReference=(username:string,cap:string,acknowledgeUncertainCost:boolean)=>creatorCommand<import("./types").BloggerReference>("read_creator_reference",{username,cap,acknowledgeUncertainCost});
export const saveCreatorReferenceGuide=(username:string,guide:string)=>creatorCommand("save_creator_reference_guide",{username,guide});
export const startScreenshotRun=(imageData:string,notes:string,originUrl:string,count:number,aiCapUsd:string)=>creatorCommand("start_screenshot_run",{imageData,notes,originUrl,count,aiCapUsd});
export const resumeCreatorRun=(acknowledgeUncertainCost:boolean)=>creatorCommand("resume_creator_run",{acknowledgeUncertainCost});
export const stopCreatorRun=()=>creatorCommand("stop_creator_run");
export const endCreatorRun=()=>creatorCommand("end_creator_run");
export const raiseCreatorCap=(kind:string,cap:string)=>creatorCommand("raise_creator_cap",{kind,cap});
export const editCreatorDraft=(id:number,title:string,body:string,scheduledAt?:string)=>creatorCommand("edit_creator_draft",{id,title,body,scheduledAt:scheduledAt??null});
export const setCreatorStatus=(id:number,status:string)=>creatorCommand("set_creator_status",{id,status});
export const approveCreatorRun=()=>creatorCommand<number>("approve_creator_run");
export const planCreator=(replanAll:boolean)=>creatorCommand("plan_creator",{replanAll});
export const snoozeCreator=(id:number)=>creatorCommand("snooze_creator",{id});
export const copyCreatorDraft=(id:number,includeSources:boolean)=>creatorCommand("copy_creator_draft",{id,includeSources});
export const rewriteCreatorDraft=(id:number,aiCapUsd:string)=>creatorCommand("rewrite_creator_draft",{id,aiCapUsd});
export const undoCreatorRewrite=(id:number)=>creatorCommand("undo_creator_rewrite",{id});
export const creatorCalendar=()=>creatorCommand<string>("creator_calendar");
export const testCreatorNotification=()=>creatorCommand("test_creator_notification");
export async function copySupportAddress(chain:string,address:string):Promise<void>{if(native) await invoke("copy_support_address",{chain});else await navigator.clipboard.writeText(address);}

export async function saveWorkbenchPreferences(preferences: WorkbenchPreferences): Promise<void> {
  if (native) await invoke("save_workbench_preferences", { preferences });
  else demoWorkbench.preferences = preferences;
}
export async function saveReplyGoal(target: number): Promise<void> {
  if (native) await invoke("save_reply_goal", { target });
  else demoWorkbench.preferences.dailyTarget = target;
}
export async function saveWorkbenchSettings(preferences: WorkbenchPreferences, persona: ReplyPersona, twitterKey: string, deepseekKey: string): Promise<void> {
  if (native) await invoke("save_workbench_settings", { preferences, persona, twitterKey, deepseekKey });
  else { await saveWorkbenchPreferences(preferences); await saveReplyPersona(persona); if (twitterKey) await saveApiKey(twitterKey); if (deepseekKey) await saveDeepseekKey(deepseekKey); }
}
export async function clearMutualCache(): Promise<void> {
  if (native) await invoke("clear_mutual_cache");
  else { demoWorkbench.mutualCount = 0; demoWorkbench.mutualCapturedAt = undefined; }
}
export async function increaseReplyXCap(newCapUsd: string): Promise<void> {
  if (native) await invoke("increase_reply_x_cap", { newCapUsd });
}
export async function copyReplyDraft(postId: string): Promise<void> {
  if (native) await invoke("copy_reply_draft", { postId });
  else { const post = demoWorkbench.posts.find(p => p.postId === postId); if (!post?.draft) throw new Error("没有草稿"); await navigator.clipboard.writeText(post.draft); if (post.status === "pending") post.status = "copied"; }
}
export async function openReplyPosts(postIds: string[]): Promise<number> {
  if (native) return invoke<number>("open_reply_posts", { postIds });
  if (postIds.length < 1 || postIds.length > 5) throw new Error("一次打开1–5条");
  const posts = postIds.map(id => demoWorkbench.posts.find(p => p.postId === id)!);
  if (posts.length === 1 && posts[0].draft) await copyReplyDraft(posts[0].postId);
  posts.forEach(post => { window.open(post.postUrl, "_blank", "noopener,noreferrer"); post.openedAt = new Date().toISOString(); });
  return posts.length;
}

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
    if (post) { if (draft !== undefined) post.draft = draft; if (status) { const wasReplied = post.status === "replied"; post.status = status as typeof post.status; if (status === "replied" && !wasReplied) { post.repliedAt = new Date().toISOString(); demoWorkbench.todayRepliedCount++; demoWorkbench.totalRepliedCount++; } else if (status !== "replied" && wasReplied) { demoWorkbench.todayRepliedCount--; demoWorkbench.totalRepliedCount--; post.repliedAt = undefined; } } }
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

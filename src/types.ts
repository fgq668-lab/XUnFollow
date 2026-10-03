export type DecisionStatus = "pending" | "unfollowed" | "keep" | "later" | "changed";

export interface Candidate {
  stableXId: string;
  username: string;
  name: string;
  profileImageUrl?: string;
  followersCount: number;
  followingCount: number;
  xUrl: string;
}

export interface Decision {
  status: Exclude<DecisionStatus, "pending">;
  actionDay: string;
  updatedAt: string;
}

export interface HistoryEntry {
  stableXId: string;
  username: string;
  name: string;
  status: Exclude<DecisionStatus, "pending">;
  actionDay: string;
  updatedAt: string;
}

export interface ScanSummary {
  capturedAt?: string;
  followersCount: number;
  followingCount: number;
  nonFollowbackCount: number;
  confirmedCostUsd: string;
  maximumPossibleCostUsd: string;
  complete: boolean;
}

export interface Bootstrap {
  account?: { username: string; name?: string };
  candidates: Candidate[];
  decisions: Record<string, Decision>;
  history: HistoryEntry[];
  summary?: ScanSummary;
  dailyGoal: number;
  batchSize: number;
  apiKeyConfigured: boolean;
  syncRunning: boolean;
  pendingScan?: {
    handle: string;
    hardCapUsd: string;
    needsExplicitRetry: boolean;
    followerPages: number;
    followingPages: number;
    followerIdsLoaded: number;
    followingProfilesLoaded: number;
    followersTotal: number;
    followingTotal: number;
    followerComplete: boolean;
    followingComplete: boolean;
  };
}

export interface CostEstimate {
  estimatedUsd: string;
  hardCapUsd: string;
  assumption: string;
}

export interface ReplyPersona {
  identity: string;
  topics: string;
  voice: string;
  language: "zh" | "en" | "auto";
  avoid: string;
}

export interface ReplyConfig {
  keywords: string[];
  targetCount: number;
  language: "zh" | "en" | "all";
  lookbackHours: number;
  sortMode: "latest" | "hot" | "recommended";
  xCapUsd: string;
  aiCapUsd: string;
  ownUsername: string;
  scope: "mutual" | "keywords";
}
export interface WorkbenchPreferences {
  replyPrompt: string;
  catchphrases: string;
  model: "deepseek-flash" | "deepseek-v4-pro";
  dailyTarget: number;
  ownUsername: string;
  defaultXCapUsd: string;
  defaultAiCapUsd: string;
  openBatchSize: number;
  creatorPrompt: string;
  creatorVoiceSamples: string;
  creatorDailyTarget: number;
  creatorDayStart: string;
  creatorDayEnd: string;
  remindersEnabled: boolean;
  reminderSound: boolean;
}

export interface CreatorConfig { topic: string; materials: string; count: number; hot: boolean; keywords: string[]; kinds: string[]; minLikes: number; xCapUsd: string; aiCapUsd: string; screenshot?: boolean; originUrl?: string; dailyTarget?: number; topics?: string[]; rewriteTarget?: number; referenceHandles?: string[]; }
export interface ReferencePost { id:string; text:string; url:string; createdAt:string; likes:number; replies:number; views:number; }
export interface BloggerReference { username:string; capturedAt:string; posts:ReferencePost[]; medianLength:number; multilineCount:number; filteredCount:number; guide:string; }
export interface ReferenceSnapshot { references:BloggerReference[]; activeHandles:string[]; reading:boolean; lastRead?:{username:string; phase:string; spentMicros:number; uncertainMicros:number; error?:string}; }
export interface CreatorSource { id: string; author: string; text: string; url: string; createdAt: string; likes: number; replies: number; }
export interface CreatorDraft { id: number; runId: number; title: string; body: string; publishBody: string; topic: string; canUndoRewrite: boolean; kind: string; sources: CreatorSource[]; imageIdea: string; status: "review" | "ready" | "published" | "skipped"; scheduledAt?: string; notifiedAt?: string; publishedAt?: string; screenshot: boolean; }
export interface CreatorRun { id: number; phase: string; config: CreatorConfig; preferences: WorkbenchPreferences; generatedCount: number; sourcesFound: number; pagesDone: number; xSpentMicros: number; aiSpentMicros: number; xUncertainMicros: number; aiUncertainMicros: number; xCapMicros: number; aiCapMicros: number; error?: string; }
export interface CreatorSnapshot { run?: CreatorRun; drafts: CreatorDraft[]; running: boolean; todayGoal: number; todayPublished: number; totalPublished: number; overdueCount: number; preferences: WorkbenchPreferences; defaultPrompt: string; }

export interface ReplyRun {
  id: number;
  day: string;
  phase: "searching" | "generating" | "paused" | "done";
  targetCount: number;
  maxPages: number;
  pagesDone: number;
  xCapUsd: string;
  aiCapUsd: string;
  xSpentUsd: string;
  aiSpentUsd: string;
  xUncertainUsd: string;
  aiUncertainUsd: string;
  aiReservedUsd: string;
  xEstimatedUsd: string;
  aiEstimatedUsd: string;
  candidateCount: number;
  selectedCount: number;
  draftedCount: number;
  repliedCount: number;
  error?: string;
  needsExplicitRetry: boolean;
}

export interface ReplyPost {
  postId: string;
  username: string;
  postText: string;
  postUrl: string;
  createdAt: string;
  reason: string;
  draft: string;
  status: "pending" | "copied" | "replied" | "skipped";
  generationError?: string;
  runId: number;
  repliedAt?: string;
  openedAt?: string;
}

export interface WorkbenchSnapshot {
  persona: ReplyPersona;
  twitterKeyConfigured: boolean;
  deepseekKeyConfigured: boolean;
  run?: ReplyRun;
  posts: ReplyPost[];
  article?: { id: number; topic: string; body: string; costUsd: string; capUsd: string };
  running: boolean;
  todayRepliedCount: number;
  totalRepliedCount: number;
  pendingCount: number;
  preferences: WorkbenchPreferences;
  defaultReplyPrompt: string;
  mutualCount: number;
  mutualCapturedAt?: string;
  networkProgress?: string;
}

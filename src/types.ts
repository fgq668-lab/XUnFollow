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
}

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
}

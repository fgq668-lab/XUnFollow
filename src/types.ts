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
  pendingScan?: {
    handle: string;
    hardCapUsd: string;
    needsExplicitRetry: boolean;
  };
}

export interface CostEstimate {
  estimatedUsd: string;
  hardCapUsd: string;
  assumption: string;
}

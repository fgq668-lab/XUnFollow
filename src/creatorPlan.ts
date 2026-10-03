/** Existing inventory can be reused, even when it was scheduled for a later day. */
export function dailyPreparation(goal: number, published: number, pending: number) {
  const valid = Number.isInteger(goal) && goal >= 1 && goal <= 200;
  const missing = valid ? Math.max(0, goal - published - pending) : 0;
  return { valid, missing, count: Math.min(50, missing) };
}
export const beijingDay = (value = new Date().toISOString()) =>
  new Date(new Date(value).getTime() + 8 * 3600_000).toISOString().slice(0, 10);

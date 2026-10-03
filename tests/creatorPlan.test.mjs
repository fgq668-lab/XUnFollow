import test from "node:test";
import assert from "node:assert/strict";
import { dailyPreparation, beijingDay } from "../src/creatorPlan.ts";
test("daily target is also the preparation quantity, capped at 50 per run", () => {
  assert.deepEqual(dailyPreparation(50, 0, 0), { valid: true, missing: 50, count: 50 });
  assert.equal(dailyPreparation(50, 1, 20).count, 29);
  assert.equal(dailyPreparation(20, 1, 19).count, 0);
  assert.equal(dailyPreparation(5, 10, 2).count, 0);
  assert.deepEqual(dailyPreparation(200, 0, 0), { valid: true, missing: 200, count: 50 });
  assert.equal(dailyPreparation(0, 0, 0).valid, false);
  assert.equal(dailyPreparation(1.5, 0, 0).valid, false);
});
test("today's planned count uses Beijing dates regardless of local timezone", () => {
  assert.equal(beijingDay("2026-10-03T16:05:00Z"), "2026-10-04");
  assert.equal(beijingDay("2026-10-03T15:59:00Z"), "2026-10-03");
});

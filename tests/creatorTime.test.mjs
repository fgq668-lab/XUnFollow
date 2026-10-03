import test from "node:test";
import assert from "node:assert/strict";
import {beijingInput,fromBeijingInput,beijingDisplay} from "../src/creatorTime.ts";
test("Beijing date conversions stay independent of machine timezone",()=>{
  assert.equal(beijingInput("2026-10-03T16:05:00Z"),"2026-10-04T00:05");
  assert.equal(fromBeijingInput("2026-10-04T00:05"),"2026-10-03T16:05:00.000Z");
  assert.match(beijingDisplay("2026-10-03T16:05:00Z"),/10\/04.*00:05/);
  assert.equal(fromBeijingInput(""),undefined);
  assert.throws(()=>fromBeijingInput("nonsense"));
});

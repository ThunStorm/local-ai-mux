import assert from "node:assert/strict";
import { test } from "node:test";
import { cacheUsage } from "../src/cache-usage.ts";
const report = { usage_source: "provider_reported", usage_scope: "last_turn", usage_status: "provider_reported", reported_input_tokens: 100, reported_cached_input_tokens: 30 };

test("cache share uses a trusted input subset and distinguishes missing from reported zero", () => {
  assert.deepEqual(cacheUsage(report), { reported: true, input: 100, cached: 30, percent: 30 });
  assert.equal(cacheUsage({ ...report, reported_cached_input_tokens: 0 }).percent, 0);
  assert.equal(cacheUsage({ ...report, reported_input_tokens: 0, reported_cached_input_tokens: 0 }).percent, null);
  assert.equal(cacheUsage({ ...report, reported_cached_input_tokens: null }).cached, null);
  assert.equal(cacheUsage({ ...report, reported_cached_input_tokens: 101 }).percent, null);
});
test("legacy and invalid provenance cannot become a fabricated cache hit", () => {
  assert.deepEqual(cacheUsage(null, 100), { reported: false, input: null, cached: null, percent: null });
  const { usage_status, reported_input_tokens, reported_cached_input_tokens, ...legacy } = report;
  assert.deepEqual(cacheUsage(legacy, 100), { reported: true, input: 100, cached: null, percent: null });
  for (const change of [{ usage_source: "unverified" }, { usage_scope: "session_total" }, { usage_status: "unknown" }]) assert.equal(cacheUsage({ ...report, ...change }).cached, null);
  for (const value of [-1, 1.5, Infinity, NaN, Number.MAX_SAFE_INTEGER + 1]) {
    assert.equal(cacheUsage({ ...report, reported_cached_input_tokens: value }).cached, null);
    assert.equal(cacheUsage({ ...report, reported_input_tokens: value }).percent, null);
  }
});

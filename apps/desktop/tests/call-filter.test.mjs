import assert from 'node:assert/strict';
import { test } from 'node:test';
import { localTimeRange, exportError } from '../src/call-filter.ts';
test('time bounds use local time, allow open ends and reject invalid/reversed periods', () => {
  assert.deepEqual(localTimeRange('', ''), {started_from:null, started_before:null});
  const from = '2026-10-21T09:00', before = '2026-10-21T10:00';
  assert.deepEqual(localTimeRange(from,before), {started_from:new Date(from).getTime()/1000,started_before:new Date(before).getTime()/1000});
  assert.equal(localTimeRange(from,'' ).started_before,null);
  for (const [a,b] of [[from,from],[before,from],['2026-02-30T10:00',''],['invalid',''],['1960-01-01T00:00','']]) assert.equal(localTimeRange(a,b),null);
});
test('export errors do not reflect arbitrary backend strings into the UI',()=>{
 assert.match(exportError('EXPORT_TOO_LARGE',true),/10,000/);
 assert.doesNotMatch(exportError('PRIVATE_SENTINEL',true),/PRIVATE_SENTINEL/);
});

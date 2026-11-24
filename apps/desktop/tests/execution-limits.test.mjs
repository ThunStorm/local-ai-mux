import assert from 'node:assert/strict';
import {test} from 'node:test';
import {parseCliIdleSeconds} from '../src/execution-limits.ts';
test('CLI idle range accepts 1200 seconds and rejects invalid configurations',()=>{
 for(const value of ['30','180','600','601','1200']) assert.equal(parseCliIdleSeconds(value),Number(value));
 for(const value of ['', ' ', '0', '29','1201','1200.5','NaN','Infinity']) assert.equal(parseCliIdleSeconds(value),null);
});

import assert from "node:assert/strict";
import { test } from "node:test";
import { buildChatPayload, eventBelongsToRequest, errorText, DEFAULT_PARAMETERS } from "../src/chat-state.ts";
const range = (min,max) => ({ support:"SUPPORTED",min,max });
const unknown = { support:"UNKNOWN",min:0,max:null };
const model = { id:"fixture/model",provider:"fixture",stream:true,chat:{history:"SUPPORTED",system:"SUPPORTED",temperature:range(0,2),top_p:range(0,1),max_tokens:range(1,4096),source:"offline fixture"} };
const completed = { id:"1",user:"前问",answer:"前答",status:"completed",model:model.id };
test("chat payload preserves successful roles and omits unused parameters", () => {
  const payload=buildChatPayload(model,[completed,{...completed,id:"2",status:"stopped",answer:"部分答案"},{...completed,id:"3",status:"failed"}],"续问","multi","中文",DEFAULT_PARAMETERS,true);
  assert.deepEqual(payload.messages,[{role:"system",content:"中文"},{role:"user",content:"前问"},{role:"assistant",content:"前答"},{role:"user",content:"续问"}]);
  assert.ok(!Object.hasOwn(payload,"temperature")); assert.ok(!Object.hasOwn(payload,"top_p")); assert.ok(!Object.hasOwn(payload,"max_tokens"));
});
test("single-turn sends only current user and rejects hidden system/history capability", () => {
  const single={...model,chat:{...model.chat,history:"UNSUPPORTED",system:"UNSUPPORTED"}};
  assert.deepEqual(buildChatPayload(single,[completed],"当前","single","",DEFAULT_PARAMETERS,true).messages,[{role:"user",content:"当前"}]);
  assert.throws(()=>buildChatPayload(single,[],"x","multi","",DEFAULT_PARAMETERS,true));
  assert.throws(()=>buildChatPayload(single,[],"x","single","隐藏提示",DEFAULT_PARAMETERS,true));
});
test("unknown, nonfinite, conflicting and out-of-range overrides are rejected",()=>{
  const payload=buildChatPayload(model,[],"x","multi","",{temperature:.7,top_p:null,max_tokens:1024},false);
  assert.equal(payload.temperature,.7);assert.equal(payload.max_tokens,1024);assert.equal(payload.stream,false);
  for(const parameters of [{temperature:NaN,top_p:null,max_tokens:null},{temperature:.7,top_p:.5,max_tokens:null},{temperature:2.1,top_p:null,max_tokens:null},{temperature:null,top_p:null,max_tokens:4097},{temperature:null,top_p:null,max_tokens:1.5}]) assert.throws(()=>buildChatPayload(model,[],"x","multi","",parameters,true));
  assert.throws(()=>buildChatPayload({...model,chat:{...model.chat,temperature:unknown}},[],"x","multi","",{temperature:.7,top_p:null,max_tokens:null},true));
});
test("model contexts never cross and UTF-8 byte limit never truncates",()=>{
  assert.throws(()=>buildChatPayload(model,[{...completed,model:"other/model"}],"x","multi","",DEFAULT_PARAMETERS,true));
  const text="中".repeat(350000);
  assert.throws(()=>buildChatPayload(model,[],text,"single","",DEFAULT_PARAMETERS,true),/1 MiB/);
  assert.equal(text.length,350000);
  assert.throws(()=>buildChatPayload(model,[],"   ","single","",DEFAULT_PARAMETERS,true));
});
test("late, duplicate and mismatched-model events do not belong to active request",()=>{
  const event={requestId:"current",model:model.id,sequence:2};
  assert.equal(eventBelongsToRequest(event,"current",model.id,1),true);
  assert.equal(eventBelongsToRequest(event,"old",model.id,1),false);
  assert.equal(eventBelongsToRequest(event,"current","other/model",1),false);
  assert.equal(eventBelongsToRequest(event,"current",model.id,2),false);
  assert.equal(eventBelongsToRequest({...event,sequence:Infinity},"current",model.id,1),false);
});

test("provider cooldown is distinct from a missing model",()=>{
  assert.match(errorText("LMX_PROVIDER_COOLDOWN",true),/冷却/);
  assert.ok(!errorText("LMX_PROVIDER_COOLDOWN",true).includes("已不可用"));
  assert.match(errorText("LMX_PROVIDER_UNAVAILABLE",true),/上游连接/);
});

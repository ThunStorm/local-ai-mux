import OpenAI from "openai";
import { VERSION } from "openai/version";
import assert from "node:assert/strict";

const client = new OpenAI({ apiKey: process.env.LOCALMUX_TEST_API_KEY, baseURL: process.env.LOCALMUX_TEST_BASE_URL });
const models = await client.models.list();
assert.ok(models.data.some(model => model.id === "mock/echo"));
const response = await client.chat.completions.create({ model: "mock/echo", messages: [{ role: "user", content: "ts-smoke" }] });
assert.equal(response.choices[0].message.content, "ts-smoke");
const stream = await client.chat.completions.create({ model: "mock/echo", messages: [{ role: "user", content: "ts-stream" }], stream: true });
let text = "";
for await (const chunk of stream) text += chunk.choices[0].delta.content ?? "";
assert.equal(text, "ts-stream");
const result = await client.responses.create({ model: "mock/echo", input: "ts-responses" });
assert.equal(result.output_text, "ts-responses");
const events = await client.responses.create({ model: "mock/echo", input: "ts-responses-stream", stream: true });
let responseText = "";
for await (const event of events) if (event.type === "response.output_text.delta") responseText += event.delta;
assert.equal(responseText, "ts-responses-stream");
console.log("TypeScript SDK: models, chat, Responses, streams passed");

const negotiated = await client.chat.completions.create({ model: "mock/echo", messages: [{ role: "system", content: "generic rules" }, { role: "user", content: "generic question" }], response_format: { type: "text" } });
assert.equal(negotiated.usage, null);
assert.equal(negotiated.choices[0].finish_reason, null);
assert.equal(negotiated.localmux.usage_status, "unknown");
const usageStream = await client.chat.completions.create({ model: "mock/echo", messages: [{ role: "user", content: "usage unknown" }], stream: true, stream_options: { include_usage: true } });
let last;
for await (const chunk of usageStream) last = chunk;
assert.equal(last.usage, null);
assert.equal(last.localmux.usage_status, "unknown");
try {
  await client.chat.completions.create({ model: "mock/echo", messages: [{ role: "user", content: "generic question" }], max_tokens: 8, max_completion_tokens: 8 });
  assert.fail("conflicting limits must fail");
} catch (error) {
  assert.equal(error.status, 400);
  assert.equal(error.error.param, "max_completion_tokens");
  assert.equal(error.error.retryable, false);
}
console.log("TypeScript SDK: capability negotiation, unknown metadata and usage stream passed");

const privateChat = await client.chat.completions.create({model:"mock/echo",messages:[{role:"user",content:"ts-no-store"}],store:false});
assert.equal(privateChat.choices[0].message.content,"ts-no-store");
const privateResponse = await client.responses.create({model:"mock/echo",input:"ts-response-no-store",store:false});
assert.equal(privateResponse.output_text,"ts-response-no-store");
console.log("TypeScript SDK: Chat and Responses store:false accepted");

const strict = client.withOptions({maxRetries:0});
const base = process.env.LOCALMUX_TEST_BASE_URL.replace(/\/v1$/,'');
const health = async () => (await fetch(base + '/api/health')).json();
const catalog = await (await fetch(base + '/api/chat/models',{headers:{Authorization:`Bearer ${process.env.LOCALMUX_TEST_API_KEY}`}})).json();
assert.equal(catalog.schema_version,2);
assert.equal((await health()).execution_limits.request_timeout_seconds,1200);
const {data:correlated,response:raw} = await strict.chat.completions.create({model:'mock/echo',messages:[{role:'user',content:'correlation'}],store:false}).withResponse();
assert.equal(raw.headers.get('x-request-id'),correlated.localmux.request_id);
for(const [model,finish,reason] of [['fixture/length','length','max_output_tokens'],['fixture/cancel',null,'cancelled']]){
 const chat = await strict.chat.completions.create({model,messages:[{role:'user',content:'synthetic'}]});
 assert.equal(chat.choices[0].finish_reason,finish);assert.equal(chat.usage.total_tokens,5);
 assert.equal(chat.usage.prompt_tokens_details.cached_tokens,1);assert.equal(chat.localmux.usage.cached_input_tokens,1);
 assert.equal(chat.localmux.actual_model,'synthetic-upstream');assert.equal(chat.localmux.actual_model_source,'provider_reported');
 const chunks=[];for await(const chunk of await strict.chat.completions.create({model,messages:[{role:'user',content:'synthetic'}],stream:true,stream_options:{include_usage:true}}))chunks.push(chunk);
 assert.equal(chunks.at(-1).usage.total_tokens,5);assert.equal(chunks.at(-1).usage.prompt_tokens_details.cached_tokens,1);assert.equal(chunks.at(-1).localmux.usage.cached_input_tokens,1);assert.equal(chunks.at(-2).choices[0].finish_reason,finish);
 for(const streaming of [false,true]){
  let result = await strict.responses.create({model,input:'synthetic',stream:streaming});
  if(streaming){const events=[];for await(const event of result)events.push(event);assert.equal(events.at(-1).type,'response.incomplete');result=events.at(-1).response;}
  assert.equal(result.status,'incomplete');assert.equal(result.incomplete_details.reason,reason);assert.equal(result.usage.total_tokens,5);assert.equal(result.usage.input_tokens_details.cached_tokens,1);assert.equal(result.localmux.usage.cached_input_tokens,1);
 }
}
for(const endpoint of ['chat','responses']){
 const stream=endpoint==='chat'?await strict.chat.completions.create({model:'fixture/wait',messages:[{role:'user',content:'synthetic'}],stream:true,store:false}):await strict.responses.create({model:'fixture/wait',input:'synthetic',stream:true,store:false});
 for await(const event of stream){if(endpoint==='chat'&&event.choices[0]?.delta.content||endpoint==='responses'&&event.type==='response.output_text.delta'){stream.controller.abort();break;}}
 for(let i=0;i<100 && (await health()).active_tasks!==0;i++)await new Promise(resolve=>setTimeout(resolve,200));
 assert.equal((await health()).active_tasks,0);
}
for(const [status,code] of [[429,'LMX_RATE_LIMITED'],[503,'LMX_PROVIDER_COOLDOWN']]){
 try { await strict.chat.completions.create({model:'fixture/error',messages:[{role:'user',content:'synthetic'}]});assert.fail('fixture error must fail'); }
 catch(error){assert.equal(error.status,status);assert.equal(error.error.code,code);assert.equal(error.headers.get('x-request-id'),error.error.request_id);}
}
console.log(`TypeScript OpenAI ${VERSION}: 25 contract cases passed / 0 failed (baseline + schema/limits/correlation + length/cancel/cache/SSE + disconnect + 400/429/503)`);

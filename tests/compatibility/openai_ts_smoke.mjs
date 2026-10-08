import OpenAI from "openai";
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

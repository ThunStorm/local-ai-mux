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

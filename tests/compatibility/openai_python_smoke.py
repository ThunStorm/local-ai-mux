"""Run against the synthetic compat server, never a real provider."""
import os
from openai import OpenAI

client = OpenAI(api_key=os.environ["LOCALMUX_TEST_API_KEY"], base_url=os.environ["LOCALMUX_TEST_BASE_URL"])
assert any(model.id == "mock/echo" for model in client.models.list().data)
response = client.chat.completions.create(model="mock/echo", messages=[{"role": "user", "content": "python-smoke"}])
assert response.choices[0].message.content == "python-smoke"
chunks = client.chat.completions.create(model="mock/echo", messages=[{"role": "user", "content": "python-stream"}], stream=True)
assert "".join(chunk.choices[0].delta.content or "" for chunk in chunks) == "python-stream"
response = client.responses.create(model="mock/echo", input="python-responses")
assert response.output_text == "python-responses"
events = client.responses.create(model="mock/echo", input="python-responses-stream", stream=True)
assert "".join(event.delta for event in events if event.type == "response.output_text.delta") == "python-responses-stream"
print("Python SDK: models, chat, Responses, streams passed")

response = client.chat.completions.create(model="mock/echo", messages=[{"role":"system","content":"generic rules"},{"role":"user","content":"generic question"}], response_format={"type":"text"})
assert response.usage is None
assert response.choices[0].finish_reason is None
assert response.localmux["usage_status"] == "unknown"
chunks = list(client.chat.completions.create(model="mock/echo",messages=[{"role":"user","content":"usage unknown"}],stream=True,stream_options={"include_usage":True}))
assert chunks[-1].usage is None
assert chunks[-1].localmux["usage_status"] == "unknown"
from openai import BadRequestError
try:
    client.chat.completions.create(model="mock/echo",messages=[{"role":"user","content":"generic question"}],max_tokens=8,max_completion_tokens=8)
    raise AssertionError("conflicting limits must fail")
except BadRequestError as error:
    assert error.body["param"] == "max_completion_tokens"
    assert error.body["retryable"] is False
print("Python SDK: capability negotiation, unknown metadata and usage stream passed")

private_chat = client.chat.completions.create(model="mock/echo", messages=[{"role":"user","content":"python-no-store"}], store=False)
assert private_chat.choices[0].message.content == "python-no-store"
private_response = client.responses.create(model="mock/echo", input="python-response-no-store", store=False)
assert private_response.output_text == "python-response-no-store"
print("Python SDK: Chat and Responses store:false accepted")

# Isolated fixture matrix: no real credentials, CLI or subscription inference.
import json, time, urllib.request, openai
client = client.with_options(max_retries=0)
base = os.environ["LOCALMUX_TEST_BASE_URL"].removesuffix("/v1")
def health():
    with urllib.request.urlopen(base + "/api/health") as response:
        return json.load(response)
with urllib.request.urlopen(urllib.request.Request(base + "/api/chat/models", headers={"Authorization": "Bearer " + os.environ["LOCALMUX_TEST_API_KEY"]})) as response:
    assert json.load(response)["schema_version"] == 2
assert health()["execution_limits"]["request_timeout_seconds"] == 1200
raw = client.chat.completions.with_raw_response.create(model="mock/echo", messages=[{"role":"user","content":"correlation"}], store=False)
assert raw.headers["x-request-id"] == raw.parse().localmux["request_id"]
for model, finish, reason in [("fixture/length", "length", "max_output_tokens"), ("fixture/cancel", None, "cancelled")]:
    chat = client.chat.completions.create(model=model, messages=[{"role":"user","content":"synthetic"}])
    assert chat.choices[0].finish_reason == finish and chat.usage.total_tokens == 5
    assert chat.usage.prompt_tokens_details.cached_tokens == chat.localmux["usage"]["cached_input_tokens"] == 1
    assert chat.localmux["actual_model"] == "synthetic-upstream" and chat.localmux["actual_model_source"] == "provider_reported"
    chunks = list(client.chat.completions.create(model=model, messages=[{"role":"user","content":"synthetic"}], stream=True, stream_options={"include_usage":True}))
    assert chunks[-1].usage.total_tokens == 5 and chunks[-2].choices[0].finish_reason == finish
    assert chunks[-1].usage.prompt_tokens_details.cached_tokens == chunks[-1].localmux["usage"]["cached_input_tokens"] == 1
    for stream in [False, True]:
        response = client.responses.create(model=model, input="synthetic", stream=stream)
        if stream:
            events = list(response)
            assert events[-1].type == "response.incomplete"
            response = events[-1].response
        assert response.status == "incomplete" and response.incomplete_details.reason == reason
        assert response.usage.total_tokens == 5
        assert response.usage.input_tokens_details.cached_tokens == response.localmux["usage"]["cached_input_tokens"] == 1
for endpoint in ["chat", "responses"]:
    stream = client.chat.completions.create(model="fixture/wait", messages=[{"role":"user","content":"synthetic"}], stream=True, store=False) if endpoint == "chat" else client.responses.create(model="fixture/wait", input="synthetic", stream=True, store=False)
    for event in stream:
        if endpoint == "chat" and event.choices and event.choices[0].delta.content or endpoint == "responses" and event.type == "response.output_text.delta":
            break
    stream.close()
    for _ in range(100):
        if health()["active_tasks"] == 0: break
        time.sleep(.2)
    assert health()["active_tasks"] == 0, f"{endpoint} disconnect did not release active task"
for status, code in [(429,"LMX_RATE_LIMITED"),(503,"LMX_PROVIDER_COOLDOWN")]:
    try:
        client.chat.completions.create(model="fixture/error", messages=[{"role":"user","content":"synthetic"}])
        raise AssertionError("fixture error must fail")
    except openai.APIStatusError as error:
        assert error.status_code == status and error.body["code"] == code
        assert error.response.headers["x-request-id"] == error.body["request_id"]
print(f"Python OpenAI {openai.__version__}: 25 contract cases passed / 0 failed (baseline + schema/limits/correlation + length/cancel/cache/SSE + disconnect + 400/429/503)")

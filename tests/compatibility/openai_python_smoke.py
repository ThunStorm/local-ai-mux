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

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

"""Offline public-protocol fixture. Never contacts Codex or reads user configuration."""
import json
import sys
from pathlib import Path
SCHEMA_CASE = "complete"
if '--version' in sys.argv:
    print('codex-cli 0.160.0')
    sys.exit(0)
if 'generate-json-schema' in sys.argv:
    if SCHEMA_CASE == 'unavailable':
        sys.exit(2)
    output = Path(sys.argv[sys.argv.index('--out') + 1]) / 'v2'
    output.mkdir()
    instructions = {'type': 'object', 'properties': {
        'baseInstructions': {'type': ['string', 'null']},
        'developerInstructions': {'type': ['string', 'null']},
    }}
    if SCHEMA_CASE == 'missing_system':
        del instructions['properties']['baseInstructions']
    if SCHEMA_CASE == 'missing_developer':
        del instructions['properties']['developerInstructions']
    for name in ('ThreadStartParams', 'ThreadResumeParams'):
        value = dict(instructions)
        if SCHEMA_CASE == 'missing_resume' and name == 'ThreadResumeParams':
            value = {'type': 'object', 'properties': {}}
        text = json.dumps(value)
        if SCHEMA_CASE == 'malformed': text = '{invalid'
        if SCHEMA_CASE == 'oversized': text = ' ' * (1024 * 1024 + 1)
        (output / (name + '.json')).write_text(text)
    effort = {'type': 'object', 'properties': {
        'effort': {'anyOf': [{'$ref': '#/definitions/ReasoningEffort'}, {'type': 'null'}]},
    }, 'definitions': {'ReasoningEffort': {'type': 'string', 'minLength': 1}}}
    if SCHEMA_CASE == 'no_effort': effort['properties'] = {}
    if SCHEMA_CASE == 'restricted_effort': effort['definitions']['ReasoningEffort']['enum'] = ['low']
    if SCHEMA_CASE == 'cyclic': effort['definitions']['ReasoningEffort'] = {'$ref': '#/definitions/ReasoningEffort'}
    (output / 'TurnStartParams.json').write_text(json.dumps(effort))
    sys.exit(0)
def emit(value):
    print(json.dumps(value), flush=True)
for line in sys.stdin:
    req = json.loads(line)
    method, params = req.get('method'), req.get('params', {})
    result = {}
    if method == 'initialize': result = {'userAgent': 'fixture'}
    elif method == 'initialized': continue
    elif method == 'account/gatewayOAuth/read': result = {'required': False}
    elif method == 'account/read': result = {'account': {'type': 'chatgpt'}, 'requiresOpenaiAuth': True}
    elif method == 'account/rateLimits/read': result = {'ordinaryUsageAllowed': True, 'rateLimitsByLimitId': {'codex': {'primary': {'usedPercent': 25, 'windowDurationMins': 300, 'resetsAt': 1700000000}}}}
    elif method == 'config/read': result = {'layers': [{'config': {'mcp_servers': {'synthetic': {'command': 'must-not-run'}}}}]}
    elif method == 'model/list': result = {'data': [{'model': 'fixture-model', 'hidden': False, 'inputModalities': ['text', 'image'], 'defaultReasoningEffort': 'low', 'supportedReasoningEfforts': [{'reasoningEffort': 'low'}, {'reasoningEffort': 'high'}]}], 'nextCursor': None}
    elif method in ('thread/start', 'thread/resume'):
        assert params['sandbox'] == 'read-only'
        assert params['approvalPolicy'] == 'never'
        assert params['config']['features']['shell_tool'] is False
        assert params['config']['features']['apps'] is False
        assert params['config']['mcp_servers']['synthetic']['enabled'] is False
        assert params['config']['web_search'] == 'disabled'
        if SCHEMA_CASE in ('unavailable', 'malformed', 'oversized', 'missing_resume'):
            assert 'baseInstructions' not in params and 'developerInstructions' not in params
        if SCHEMA_CASE == 'missing_system': assert 'baseInstructions' not in params
        if SCHEMA_CASE == 'missing_developer': assert 'developerInstructions' not in params
        system, developer = params.get('baseInstructions'), params.get('developerInstructions')
        result = {'thread': {'id': 'thread-fixture'}, 'model': 'fixture-model'}
    elif method == 'turn/start':
        assert params['sandboxPolicy']['type'] == 'readOnly'
        assert params['approvalPolicy'] == 'never'
        prompt = params['input'][0]['text']
        if SCHEMA_CASE == 'restricted_effort': assert params.get('effort') in (None, 'low')
        if prompt == 'negotiation-fixture':
            assert system == 'one\n\n--- next instruction ---\n\ntwo'
            assert developer == 'developer rule'
            assert params['effort'] == 'high'
        emit({'id': req['id'], 'result': {'turn': {'id': 'turn-fixture'}}})
        base = {'threadId': 'thread-fixture', 'turnId': 'turn-fixture'}
        if prompt == 'error-fixture':
            emit({'method': 'error', 'params': dict(base, willRetry=True, error={'message': 'private upstream detail', 'codexErrorInfo': {'responseStreamDisconnected': {'httpStatusCode': None}}})})
            continue
        emit({'method': 'item/agentMessage/delta', 'params': dict(base, delta='fixture reply')})
        emit({'method': 'item/completed', 'params': dict(base, item={'type': 'agentMessage', 'text': 'fixture reply'})})
        for _ in range(2):
            emit({'method': 'thread/tokenUsage/updated', 'params': dict(base, tokenUsage={'last': {'inputTokens': 10, 'outputTokens': 5, 'totalTokens': 15, 'reasoningOutputTokens': 2, 'cachedInputTokens': 3}, 'total': {'inputTokens': 1000, 'outputTokens': 500, 'totalTokens': 1500}})})
        emit({'method': 'turn/completed', 'params': dict(base, turn={'status': 'completed'})})
        continue
    else: raise AssertionError(method)
    emit({'id': req['id'], 'result': result})

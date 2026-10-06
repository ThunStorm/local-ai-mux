"""Offline public-protocol fixture. Never contacts Codex or reads user configuration."""
import json
import sys
if '--version' in sys.argv:
    print('codex-cli 0.160.0')
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
        system, developer = params['baseInstructions'], params['developerInstructions']
        result = {'thread': {'id': 'thread-fixture'}, 'model': 'fixture-model'}
    elif method == 'turn/start':
        assert params['sandboxPolicy']['type'] == 'readOnly'
        assert params['approvalPolicy'] == 'never'
        prompt = params['input'][0]['text']
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

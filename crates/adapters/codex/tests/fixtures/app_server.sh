#!/bin/sh
if [ "$1" = "--version" ]; then printf "%s\n" "codex-cli 0.155.0-alpha.16.4"; exit 0; fi
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*) printf '%s\n' '{"id":1,"result":{"userAgent":"fixture"}}' ;;
    *'"method":"account/gatewayOAuth/read"'*) printf '%s\n' '{"id":2,"result":{"required":false}}' ;;
    *'"method":"account/read"'*) printf '%s\n' '{"id":3,"result":{"account":{"type":"chatgpt"},"requiresOpenaiAuth":true}}' ;;
    *'"method":"account/rateLimits/read"'*) printf '%s\n' '{"id":3,"result":{"ordinaryUsageAllowed":true,"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":1700000000}}}}}' ;;
    *'"method":"model/list"'*) printf '%s\n' '{"id":4,"result":{"data":[{"model":"fixture-model","hidden":false}],"nextCursor":null}}' ;;
    *'"method":"thread/start"'*) printf '%s\n' '{"id":3,"result":{"thread":{"id":"thread-fixture"}}}' ;;
    *'"method":"thread/resume"'*) printf '%s\n' '{"id":3,"result":{"thread":{"id":"thread-fixture"}}}' ;;
    *'"method":"turn/start"'*)
      printf '%s\n' '{"id":4,"result":{"turn":{"id":"turn-fixture"}}}'
      printf '%s\n' '{"method":"item/agentMessage/delta","params":{"threadId":"thread-fixture","turnId":"turn-fixture","delta":"fixture reply"}}'
      printf '%s\n' '{"method":"item/completed","params":{"threadId":"thread-fixture","turnId":"turn-fixture","item":{"type":"agentMessage","text":"fixture reply"}}}'
      printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thread-fixture","turnId":"turn-fixture","turn":{"status":"completed"}}}' ;;
  esac
done

#!/bin/sh
if [ "$1" = "--version" ]; then echo "fixture 1.0"; exit 0; fi
if [ "$1" = "--help" ]; then echo "--system-prompt --tools --strict-mcp-config --setting-sources"; exit 0; fi
case " $* " in
  *' --list-models '*) printf '%s\n' '{"models":[{"id":"current-model"},{"id":"efficient"}]}' ; exit 0 ;;
esac
case "$*" in *"--permission-mode dont_ask"*) ;; *) exit 3 ;; esac
case "$*" in *"--strict-mcp-config"*) ;; *) exit 3 ;; esac
case "$*" in *"--max-turns 1"*) ;; *) exit 3 ;; esac
case "$*" in *"--setting-sources "*) ;; *) exit 3 ;; esac
# For a synthetic instruction request, assert both controls and ordered content reach the CLI.
case "$*" in *"--system-prompt "*) case "$*" in *"one"*"two"*) ;; *) exit 3 ;; esac ;; esac
IFS= read -r prompt || true
[ -n "$prompt" ] || exit 4
if [ -n "${LOCALMUX_TEST_QODER_DISPATCH_LOG:-}" ]; then
  printf '%s\n' "$prompt" >> "$LOCALMUX_TEST_QODER_DISPATCH_LOG"
fi
case "$prompt" in
  first-payload-timeout|timeout-a)
    printf '%s\n' '{"type":"result","is_error":true,"result":"Model stream timed out before first payload after 120s without stream data (total 122s)"}'
    exit 1 ;;
  first-payload-stderr)
    printf '%s\n' 'Error: Model stream timed out before first payload after 120s without stream data (total 122s)' >&2
    exit 1 ;;
  null-error)
    printf '%s\n' '{"type":"result","error":null,"result":"ok"}'
    exit 0 ;;
  already-done|rejected-b)
    printf '%s\n' '{"type":"result","result":"{\"section_facts\":[],\"places\":[],\"warnings\":[]}"}'
    exit 0 ;;
esac
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"first "}}}'
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"second"}}}'
printf '%s\n' '{"type":"result","result":"first second","session_id":"qoder-fixture-session"}'

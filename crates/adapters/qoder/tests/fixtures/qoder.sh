#!/bin/sh
case " $* " in
  *' --list-models '*) printf '%s\n' '{"models":[{"id":"current-model"},{"id":"efficient"}]}' ; exit 0 ;;
esac
case "$*" in *"--permission-mode dont_ask"*) ;; *) exit 3 ;; esac
case "$*" in *"--strict-mcp-config"*) ;; *) exit 3 ;; esac
case "$*" in *"--max-turns 1"*) ;; *) exit 3 ;; esac
IFS= read -r prompt || true
[ -n "$prompt" ] || exit 4
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"first "}}}'
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"second"}}}'
printf '%s\n' '{"type":"result","result":"first second","session_id":"qoder-fixture-session"}'

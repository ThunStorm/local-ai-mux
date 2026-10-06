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
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"first "}}}'
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"second"}}}'
printf '%s\n' '{"type":"result","result":"first second","session_id":"qoder-fixture-session"}'

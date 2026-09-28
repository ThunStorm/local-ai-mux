#!/bin/sh
if [ "$1" = "config" ] && [ "$2" = "get" ] && [ "$3" = "model" ]; then printf '%s\n' 'fixture-model'; exit 0; fi
case "$*" in *"--permission-mode dontAsk"*) ;; *) exit 3 ;; esac
case "$*" in *"--include-partial-messages"*) ;; *) exit 3 ;; esac
case "$*" in *"--strict-mcp-config"*) ;; *) exit 3 ;; esac
IFS= read -r prompt || true
[ -n "$prompt" ] || exit 4
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"buddy "}}}'
printf '%s\n' '{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"reply"}}}'
printf '%s\n' '{"type":"result","result":"buddy reply","session_id":"buddy-fixture-session"}'

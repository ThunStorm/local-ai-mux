#!/bin/sh
if [ "$1" = "--version" ]; then echo "fixture 1.0"; exit 0; fi
if [ "$1" = "--help" ]; then echo "--system-prompt --tools --strict-mcp-config --setting-sources"; echo "--model <model> Currently supported: (fixture-a, fixture-b)"; exit 0; fi
if [ "$1" = "config" ] && [ "$2" = "get" ] && [ "$3" = "model" ]; then printf '\n'; exit 0; fi
exit 3

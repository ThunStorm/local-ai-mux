#!/bin/sh
exec python3 "$(dirname "$0")/app_server.py" "$@"

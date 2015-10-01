#!/bin/sh
cd "$(dirname "$0")/.."
chmod +x scripts/*.sh
exec scripts/checked.sh scripts/miau-build.sh

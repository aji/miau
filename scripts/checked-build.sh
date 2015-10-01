#!/bin/sh
chdir "$(dirname "$0")/.."
exec scripts/checked.sh scripts/build.sh

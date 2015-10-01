#!/bin/sh

RUNAS='ec2-user'
ROOT=/home/$RUNAS/build/miau
SCRIPTDIR=$(dirname $(readlink -f "$1"))

if [ "$(id -u)" = '0' ]; then
  echo "Dropping privileges"
  sudo -u "$RUNAS" "$@"
fi

cd "$ROOT"
exec $SCRIPTDIR"$@" 2>&1 >>build.log

#!/bin/sh

RUNAS='ec2-user'
ROOT=/home/$RUNAS/build/miau

if [ "$(id -u)" = '0' ]; then
  echo "Dropping privileges"
  sudo -u "$RUNAS" -i "$@"
fi

cd "$ROOT"
exec "$@" 2>&1 >>build.log

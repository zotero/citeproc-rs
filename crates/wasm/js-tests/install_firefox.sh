#!/usr/bin/env bash

# Installs firefox using zotero-standalone-build. (Not system-wide.)

CALLDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $CALLDIR

set -uo pipefail

ZSB=zotero-standalone-build
echo "fetching zotero-standalone-build" >/dev/stderr
if [ ! -d $ZSB/.git ]; then
  git clone https://github.com/zotero/$ZSB
else
  (cd $ZSB && git pull)
fi

# grab the variables
. $ZSB/config.sh

UNAME=$(uname)
if [ "$UNAME" == "Linux" ]; then PLAT=l;
elif [ "$UNAME" == "Darwin" ]; then PLAT=m;
else PLAT=w
fi

# the whole CALLDIR thing at the top of fetch_xulrunner.sh isn't used throughout
cd $ZSB
echo "running fetch_xulrunner.sh" >/dev/stderr
bash -euo pipefail ./fetch_xulrunner.sh -p $PLAT &>/dev/null
cd $CALLDIR

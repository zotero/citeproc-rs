#!/bin/bash

# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at http://mozilla.org/MPL/2.0/.
#
# Copyright © 2019 Corporation for Digital Scholarship

# mpl.sh

# This adds the MPL 2.0 license notice to a file. If the file is tracked
# by git, the date is set to when it was first committed; otherwise, it is the
# current year. The templates are in notice.* files.

SCRIPT=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )

copyright_year () {
  git log --follow --oneline --pretty="format:%ci" "$1" | cut -d- -f1 | tail -n 1
}

FILE="$1"
YEAR=$(copyright_year "$FILE")

if [[ "$YEAR" -eq "" ]]; then
  YEAR=$(date +"%Y")
fi

NOTICE="$SCRIPT/notice.${FILE##*.}"

cat <(sed "s/COPYRIGHT_YEAR/$YEAR/g" "$NOTICE") "$FILE" | sponge "$FILE"

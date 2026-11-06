#!/usr/bin/env bash
# Survarium 0.26g on the Skhid server, for Linux. This file belongs in the
# game's folder, beside the folder named game. In a terminal there:
#
#     bash Skhid.sh             starts the game
#     bash Skhid.sh launcher    starts the Skhid launcher, which looks for
#                               newer Skhid files first (see LINUX.txt)
#     SKHID_CRASH_DUMP=full bash Skhid.sh launcher
#                               the same, a crash writing the game's whole
#                               memory, and the crash files listed at the end
#
# It only runs game/binaries/x86/play_skhid.sh, which does the work.
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd) || exit 1
script=$here/game/binaries/x86/play_skhid.sh
if [[ ! -f $script ]]; then
    echo "play_skhid.sh was not found in $here/game/binaries/x86" >&2
    echo "Unpack the Skhid files into the game's folder, the one that holds the" >&2
    echo "folder named game, and run Skhid.sh from there." >&2
    exit 1
fi
exec bash "$script" "$@"

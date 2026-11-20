#!/bin/bash
# Survarium 0.26g on the Skhid server, for macOS: double-click in Finder (it
# opens Terminal) to start the Skhid launcher. The same as bash Skhid.sh launcher.
cd -- "$(dirname -- "$0")" || exit 1
exec bash Skhid.sh launcher

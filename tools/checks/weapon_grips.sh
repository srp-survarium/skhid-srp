#!/usr/bin/env bash
# The grips' pack and texts are what their sources make (scripts/weapon-grips): the pack built anew
# from the kept 0.69d0 donor folder and the kept icons render_icons.py drew (icons-2, @2's) has the
# manifest's hash, grips.tsv and the items' rows are its,
# and the twelve localization tables hold its texts. Needs the pack weapon-modules as built (its
# models are checked to be there).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
common=$(dirname "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)")
donor="$common/private/weapon-grips/donor"
icons="$common/private/weapon-grips/icons-2"
modules=$(sed -n 's/^pack\tweapon-modules@\([^\t]*\)\t.*/weapon-modules-\1/p' "$root/release/content.tsv")
export SCOPE_CHECK_FLAKE="git+file://$root?dir=srp-026g"
cd "$root"
nix shell --impure --expr '
  let pkgs = import (builtins.getFlake (builtins.getEnv "SCOPE_CHECK_FLAKE")).inputs.nixpkgs
    { system = builtins.currentSystem; };
  in pkgs.python3.withPackages (p: [ p.numpy p.pillow ])
' --command bash -c '
  set -euo pipefail
  python3 -I srp-026g/scripts/weapon-grips/make_pack.py --donor "$1" --modules "$2" --icons "$3" --out "$CHECK_WORK/pack" --check
  python3 -I srp-026g/scripts/weapon-grips/texts.py "$1" --check
' weapon-grips "$donor" "$common/private/content-packs/$modules" "$icons"

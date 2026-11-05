#!/usr/bin/env bash
# client2.sh [exe] [account index 2|3]: start one more local client in its run.py prefix
S=${CR_WORK:?CR_WORK: a scratch folder with env.sh (DISPLAY, PATH, PULSE_SINK)}
. $(dirname $0)/lib.sh
N=${2:-2}
EXE=${1:-$(nix shell nixpkgs#python3 -c python3 -c "import json;print(json.load(open('$CFG'))['accounts'][$N-1]['executable'])")}
NAME=$(nix shell nixpkgs#python3 -c python3 -c "import json;print(json.load(open('$CFG'))['accounts'][$N-1]['name'])")
PW=$(nix shell nixpkgs#python3 -c python3 -c "import json;print(json.load(open('$CFG'))['accounts'][$N-1]['password'])")
cd $G
WINEPREFIX=$CD/runtime/client-$N WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml,qwave=;winemenubuilder.exe=d;d3d11,dxgi=n,b" \
  setsid nohup $R/private/wine-keyboard/runtime/bin/wine $EXE -account_name=$NAME -account_password=$PW -localization=english \
  -server=127.0.0.1:$(nix shell nixpkgs#python3 -c python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['ports'][0])" "$CFG") -mt_allocator=768 -minimum_resources_memory_size=768 -splash_screen > $CD/runtime/client-$N.extra.log 2>&1 < /dev/null &
echo started $EXE as $NAME

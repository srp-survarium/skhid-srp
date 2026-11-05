#!/usr/bin/env bash
# fire.sh <tag> [cycles]: the PvB match panel is set; press Play, wait for the match,
# look up at the sky, fire 9 Vepr-12 buckshot shells + reload, [cycles] times, recording audio.
S=${CR_WORK:?CR_WORK: a scratch folder with env.sh (DISPLAY, PATH, PULSE_SINK)}
. $(dirname $0)/lib.sh
TAG=$1 CYC=${2:-12}; OUT=$S/runs/$TAG; mkdir -p $OUT
W=$(wins | head -1)
before=$(ls $G/SRP-crash-*.txt 2>/dev/null | sort)
setsid nohup nix shell nixpkgs#pulseaudio -c parecord --device=srp_repro.monitor --file-format=wav $OUT/audio.wav > /dev/null 2>&1 < /dev/null &
rclick 638 24 1 3
sleep 45
magick import -display $DISPLAY -window root $OUT/01-match.png
focus; xdotool mousemove_relative -- 0 -350; sleep 0.5; xdotool mousemove_relative -- -60 -60; sleep 0.5; xdotool mousemove_relative -- 0 -90; sleep 0.5
magick import -display $DISPLAY -window root $OUT/02-aim.png
start=$(date +%s)
for c in $(seq 1 $CYC); do
  focus
  for i in 1 2 3 4 5 6 7 8 9; do xdotool mousedown 1; sleep 0.07; xdotool mouseup 1; sleep 0.09; done
  xdotool keydown r; sleep 0.1; xdotool keyup r; sleep 3.6
  [ $c = 4 ] && magick import -display $DISPLAY -window root $OUT/03-firing.png
done
echo "fired $CYC cycles in $(( $(date +%s) - start )) s" > $OUT/result.txt
sleep 5
magick import -display $DISPLAY -window root $OUT/04-after.png
sleep 10
pkill -f "^parecord --device=srp_repro"
cp $(gamelog 1) $OUT/
for f in $(comm -13 <(echo "$before") <(ls $G/SRP-crash-*.txt 2>/dev/null | sort)); do cp $f $OUT/; echo "crash: $f" >> $OUT/result.txt; grep -m3 "^Exception\|^Address\|Description" $f >> $OUT/result.txt; done
pgrep -f '[s]urvarium_skhid.*\.exe' >/dev/null && echo "game running" >> $OUT/result.txt
cat $OUT/result.txt

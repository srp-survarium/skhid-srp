#!/usr/bin/env bash
# c5b.sh <match hex> <end epoch s (float)> <exe> <tag> [lead s]: kill Repro2's game, start it again
# with <exe>, and click Enter <lead> s before the match's time is up: the room seats it back in the
# match at its sign-in, and the match ends while it loads, so it gets 0x97 with no match loaded.
S=${CR_WORK:?CR_WORK: a scratch folder with env.sh}
. $(dirname $0)/lib.sh
M=$1 END=$2 EXE=$3 OUT=$S/runs/$4 LEAD=${5:-2}; mkdir -p $OUT
# Repro2's window: the one whose process runs with -account_name=Repro2
for w in $(wins); do p=$(xdotool getwindowpid $w); tr '\0' ' ' < /proc/$p/cmdline | grep -q "account_name=Repro2" && W2=$w; done
before=$(ls $G/SRP-crash-*.txt | sort)
OLDPID=$(xdotool getwindowpid $W2); kill -9 $OLDPID; echo "killed Repro2's game $(date +%T)" > $OUT/result.txt
sleep 3
$(dirname $0)/client2.sh $EXE 2
W=; until [ -n "$W" ]; do sleep 1; for w in $(wins); do p=$(xdotool getwindowpid $w 2>/dev/null); [ -n "$p" ] && [ "$p" != "$OLDPID" ] && tr "\0" " " < /proc/$p/cmdline 2>/dev/null | grep -q "account_name=Repro2" && W=$w; done; done
echo "window $W $(date +%T)" >> $OUT/result.txt
until awk -v n=$(date +%s.%N) -v e=$END -v l=$LEAD "BEGIN{exit !(n >= e - l)}"; do sleep 0.1; done
n0=$(grep -c "Repro2 goes to match $M" $CD/runtime/server.log)
rclick 1111 549 1 0.2
echo "Enter clicked $(date +%T.%N)" >> $OUT/result.txt
if [ -n "${FREEZE:-}" ]; then
  # a slow machine: the game is stopped from the moment the lobby sends it to the
  # match (before it reaches the match server) until the lobby has sent it the results
  P=$(xdotool getwindowpid $W)
  until [ $(grep -c "Repro2 goes to match $M" $CD/runtime/server.log) -gt $n0 ]; do sleep 0.05; done
  kill -STOP $P; echo "game stopped $(date +%T.%N)" >> $OUT/result.txt
  until grep -q "Repro2 is shown the results of match $M\|Repro2 gets the notices of match $M" $CD/runtime/server.log; do sleep 0.2; done
  sleep 1; kill -CONT $P; echo "game continued $(date +%T.%N)" >> $OUT/result.txt
fi
for i in $(seq 1 30); do sleep 1; magick import -display $DISPLAY -window root -resize 640x360 $OUT/t$(printf %02d $i).png; done
grep "$M\|Repro2" $CD/runtime/server.log | grep -v ping | tail -14 | cut -c1-200 >> $OUT/result.txt
for f in $(comm -13 <(echo "$before") <(ls $G/SRP-crash-*.txt | sort)); do cp $f $OUT/; echo "crash: $f" >> $OUT/result.txt; grep -m2 "^Exception\|^Address" $f >> $OUT/result.txt; done
cat $OUT/result.txt

#!/usr/bin/env bash
# c5next.sh <exe> <tag> [lead s]: wait for the room's next match to begin play, then run
# c5b.sh (FREEZE as set) so that the restarted game signs in <lead> s before its time is up
# (time up = "play begins" + 902.0 s here).
S=${CR_WORK:?CR_WORK: a scratch folder with env.sh}
. $(dirname $0)/lib.sh
n0=$(grep -c "play begins at" $CD/runtime/server.log)
until [ $(grep -c "play begins at" $CD/runtime/server.log) -gt $n0 ]; do sleep 2; done
L=$(grep "play begins at" $CD/runtime/server.log | tail -1)
M=$(echo "$L" | grep -o "Match 0x[0-9a-f]*" | cut -d' ' -f2); T=$(echo "$L" | cut -c1-12)
END=$(awk -v z=$(date -d "2026-11-05 $T UTC" +%s.%N) 'BEGIN{printf "%.3f", z + 902.0}')
echo "match $M play began $T -> time up at $END ($(date -d @${END%.*} +%T))"
until awk -v n=$(date +%s) -v e=$END -v l=${3:-45} 'BEGIN{exit !(n >= e - l - 40)}'; do sleep 2; done
$(dirname $0)/c5b.sh $M $END $1 $2 ${3:-45}

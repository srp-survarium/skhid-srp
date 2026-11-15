#!/usr/bin/env bash
# The stock assets the battery swing is authored and previewed on, out of a
# 0.26g resources.db: extract.sh <resources.db> <vostok-port> <out folder>
set -euo pipefail
db=$1 porter=$2 out=$3
mkdir -p "$out"
K=animations/single/weapons/key_objects
C=models/character/human/black_market
for p in \
  $K/player/nonadditive/stand/3rd_view/on_site/hands_only/idle.b-spline \
  $K/player/nonadditive/stand/3rd_view/on_site/hands_only/idle.bones_names \
  $K/player/nonadditive/stand/3rd_view/on_site/hands_only/hidetest.b-spline \
  $K/player/nonadditive/stand/3rd_view/on_site/hands_only/hidetest.bones_names \
  $K/item/stand/3rd_view/idle_electrobox.b-spline $K/item/stand/3rd_view/idle_electrobox.bones_names \
  $K/item/stand/3rd_view/hidetest_electrobox.b-spline $K/item/stand/3rd_view/hidetest_electrobox.bones_names \
  $K/player/stand/3rd_view/hands_only/idle_look.b-spline $K/player/stand/3rd_view/hands_only/idle_look.bones_names \
  animations/skeletons/scavengers_01.skeleton animations/skeletons/weapons/electrobox.skeleton \
  models/weapons/electrobox.skinned_model/render/bind_pose \
  models/weapons/electrobox.skinned_model/render/electrobox_s/converted_model \
  models/character/human/base/base_character_head.skinned_model/render/bind_pose \
  models/character/human/base/base_character_head.skinned_model/render/black_market_01_head_s/converted_model; do
  "$porter" extract --db "$db" --path "$p" --output "$out/${p//\//_}" >/dev/null
done
for m in hand_hud:hand_s torso_s1:torso_s legs_s1:legs_s boot:boot_s hand:hand_s helmet:helmet_s; do
  model=black_market_01_${m%%:*} surface=black_market_01_${m##*:}
  for p in $C/$model.skinned_model/render/bind_pose $C/$model.skinned_model/render/$surface/converted_model; do
    "$porter" extract --db "$db" --path "$p" --output "$out/${p//\//_}" >/dev/null
  done
done

#!/usr/bin/env bash
# pair.sh <before.png> <before label> <after.png> <after label> <out.png>
. $CR_WORK/env.sh
font=${CR_FONT:-$(fc-match -f "%{file}" sans)}
lbl() { magick "$1" -resize 960x540 -gravity north -background black -splice 0x44 -fill white -font $font -pointsize 24 -annotate +0+8 "$2" png:-; }
magick <(lbl "$1" "$2") <(lbl "$3" "$4") +append "$5"

# Medical card checks

`check.sh <lobby_menu.swf> [work directory]` runs the movie's real item-card,
comparison and indicator classes in CPU-only Ruffle on a separate virtual display.
Use the project's Nix developer shell (or set `PYTHON` to its Python interpreter).
It checks six complete rows, healing percentages, explicit medkit pain relief 0%,
weight, activation comparison colors, repeated comparisons and card bounds.

The runner adapts only its copied test movie with `ruffle_movie.py`. Stock
Scaleform placements differ from the standard SWF form Ruffle accepts: their
visibility flag has no following byte, some bitmap placements omit the image
class name, and some place a symbol by its class name alone. Without the adapter
Ruffle loses named card sections, bar sprites and the profile's CLIK controls.
The release movie is never modified by this test fixture step.

`check_content.py <before> <after>` takes directories of extracted gameplay files
from two manifest builds. It verifies the displayed healing and pain percentages
against each drug's actual influences and duration, preserves weight and use times,
and confirms that only the item cards' property lists changed. Extract through
`srp-game-data get`, mounting the stock database and every manifest patch in order.

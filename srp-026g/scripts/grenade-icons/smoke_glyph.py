#!/usr/bin/env python3
"""smoke_glyph.py: the smoke grenade's marker glyph (player_icons' SmokeMark), a white sketch of the
RDG-M as the game's own art draws it, made the way the stock frag glyph (player_icons shape 32) is a
sketch of the RGD-5: the body solid, the fuse head and the lever separate pieces with a cut between,
tilted and sized as the frag glyph (docs/notes/grenade-icons.md).

    python3 -I smoke_glyph.py [--svg <preview.svg>]

Prints SmokeMark's polygons (ActionScript, the icon's pixels). Needs shapely.

The RDG-M's shape is measured from its HUD icon (rdg-m's ui_desc/icon 3, 6: hudItems' cell 15 of
the stock hud movie, the same render as the lobby's AmmoIcons one), turned upright (it leans 12.5
degrees): 79.5 px from the fuse's tip to the bottom, the fuse head 6 px wide to 15 px, a neck 10 px
wide to 27 px, the shoulder to 33 px, the body 27.5 px wide to 63 px, the domed bottom to 79.5 px,
and the lever's wire from the fuse's foot down the shoulder's left side. The pin's ring is left out,
as the frag glyph leaves out the RGD-5's.
"""
import argparse
import math

from shapely import affinity
from shapely.geometry import LineString, Point, Polygon, box
from shapely.ops import unary_union

# The frag glyph (shape 32, twips): its bounds' centre and height, its tilt (its fuse rod's line,
# -98, 163: 31 degrees off the vertical, the top to the right), its rod's width (40 twips) and its
# cuts (about 20 twips).
FRAG_CENTRE = (-8.0, -641.5)
FRAG_HEIGHT = 617.0
TILT = math.degrees(math.atan2(98, 163))
STROKE = 40.0
CUT = 22.0

TWIPS = 20.0


def rdg_m(scale):
    """The upright RDG-M in twips (the icon's pixels x `scale`): body, fuse head, lever."""
    s = lambda v: v * scale
    centre = 15.5
    head = unary_union([box(s(16.5), s(2.5), s(22.5), s(14.5)), Point(s(19.5), s(2.5)).buffer(s(3.0))])
    neck = box(s(centre - 5), s(14.5) + CUT, s(centre + 5), s(28))
    shoulder = Polygon([(s(centre - 5), s(27)), (s(centre + 5), s(27)), (s(29.25), s(33.5)), (s(1.75), s(33.5))])
    body = box(s(1.75), s(33), s(29.25), s(63))
    dome = affinity.scale(Point(s(centre), s(63)).buffer(1.0), s(13.75), s(16.5), origin=(s(centre), s(63)))
    body = unary_union([neck, shoulder, body, dome]).buffer(s(1.2)).buffer(-s(1.2))
    # the lever: the wire from the fuse's foot down the shoulder's left, outside the body by a cut
    wire = LineString([(s(12.5), s(14.5)), (s(5.0), s(23.0)), (s(0.4), s(30.5)), (s(-3.7), s(37.5))])
    lever = wire.buffer(STROKE / 2, cap_style='round', join_style='round')
    body = body.difference(lever.buffer(CUT))
    head = head.difference(lever.buffer(CUT))
    return body, head, lever


def glyph():
    # the frag glyph's height for the tilted RDG-M: scale found by a first placement
    scale = 7.7
    body, head, lever = rdg_m(scale)
    whole = affinity.rotate(unary_union([body, head, lever]), TILT, origin=(0, 0))
    scale *= FRAG_HEIGHT / (whole.bounds[3] - whole.bounds[1])
    body, head, lever = (affinity.rotate(g, TILT, origin=(0, 0)) for g in rdg_m(scale))
    minx, miny, maxx, maxy = unary_union([body, head, lever]).bounds
    dx, dy = FRAG_CENTRE[0] - (minx + maxx) / 2, FRAG_CENTRE[1] - (miny + maxy) / 2
    return [affinity.translate(g, dx, dy).simplify(3.0) for g in (body, head, lever)]


def ring(polygon):
    coords = list(polygon.exterior.coords)[:-1]
    return [round(v / TWIPS, 2) for xy in coords for v in xy]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--svg')
    args = parser.parse_args()
    body, head, lever = glyph()
    inner = body.buffer(-TWIPS)  # the toxic smoke's green: the body inside its 1 px white edge
    for name, polygon in (('BODY', body), ('BODY_INSIDE', inner), ('HEAD', head), ('LEVER', lever)):
        print(f'      public static const {name}:Array = {ring(polygon)};'.replace(', ', ','))
    if args.svg:
        paths = ''.join(f'<path d="M{" L".join(f"{x} {y}" for x, y in list(g.exterior.coords))}Z" fill="#fff"/>'
                        for g in (body, head, lever))
        with open(args.svg, 'w') as out:
            out.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="500" height="660" viewBox="-260 -970 500 660">'
                      f'<rect x="-260" y="-970" width="500" height="660" fill="#30373B"/>{paths}</svg>')


if __name__ == '__main__':
    main()

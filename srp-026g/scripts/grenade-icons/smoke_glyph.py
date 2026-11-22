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
    """The upright RDG-M in twips (the icon's pixels x `scale`): body, fuse head, lever, bottom cap."""
    s = lambda v: v * scale
    centre = 15.5
    head = unary_union([box(s(16.5), s(2.5), s(22.5), s(14.5)), Point(s(19.5), s(2.5)).buffer(s(3.0))])
    neck = box(s(centre - 5), s(14.5) + CUT, s(centre + 5), s(28))
    shoulder = Polygon([(s(centre - 5), s(27)), (s(centre + 5), s(27)), (s(29.25), s(33.5)), (s(1.75), s(33.5))])
    body = box(s(1.75), s(33), s(29.25), s(62))
    # the base: the body ends in a rim a little wider than it (62 to 67 px, 29.5 px across), and under
    # it the bottom cap, a dome 25 px across and 12.5 px tall, apart from the rim by a cut
    rim = box(s(0.75), s(61.5), s(30.25), s(67))
    body = unary_union([neck, shoulder, body, rim]).buffer(s(1.2)).buffer(-s(1.2))
    dome = affinity.scale(Point(s(centre), s(66.5)).buffer(1.0), s(12.6), s(13.0), origin=(s(centre), s(66.5)))
    cap = dome.intersection(box(s(-5), s(67) + CUT, s(35), s(85)))
    # the lever: the wire from the fuse's foot down the shoulder's left, outside the body by a cut
    wire = LineString([(s(12.5), s(14.5)), (s(5.0), s(23.0)), (s(0.4), s(30.5)), (s(-3.7), s(37.5))])
    lever = wire.buffer(STROKE / 2, cap_style='round', join_style='round')
    body = body.difference(lever.buffer(CUT))
    head = head.difference(lever.buffer(CUT))
    return body, head, lever, cap


PARTS = ('BODY', 'HEAD', 'LEVER', 'CAP')


def scale_to_frag():
    """The twips an icon pixel: the tilted RDG-M as tall as the frag glyph."""
    scale = 7.7
    whole = affinity.rotate(unary_union(rdg_m(scale)), TILT, origin=(0, 0))
    return scale * FRAG_HEIGHT / (whole.bounds[3] - whole.bounds[1])


def glyph():
    """The parts placed as the frag glyph is: tilted, centred where it is in the ring."""
    parts = [affinity.rotate(g, TILT, origin=(0, 0)) for g in rdg_m(scale_to_frag())]
    minx, miny, maxx, maxy = unary_union(parts).bounds
    dx, dy = FRAG_CENTRE[0] - (minx + maxx) / 2, FRAG_CENTRE[1] - (miny + maxy) / 2
    return [affinity.translate(g, dx, dy).simplify(3.0) for g in parts]


def ring(polygon):
    coords = list(polygon.exterior.coords)[:-1]
    return [round(v / TWIPS, 2) for xy in coords for v in xy]


def path(polygon, style):
    return f'<path d="M{" L".join(f"{x:.1f} {y:.1f}" for x, y in polygon.exterior.coords)}Z" {style}/>'


def overlay(icon, out, bottom=False):
    """The upright sketch's outline (red) over the upright icon at the same scale (the icon's pixels
    x the sketch's scale; `icon` the icon turned upright, 4 px a pixel); `bottom`: its base only."""
    scale = scale_to_frag()
    parts = rdg_m(scale)
    width, height = 31 * scale, 79.5 * scale
    box_ = (-4 * scale, 55 * scale, 39 * scale, 28 * scale) if bottom else (-8 * scale, -2 * scale, 47 * scale, 84 * scale)
    stroke = f'fill="none" stroke="#FF3B30" stroke-width="{scale * (0.12 if bottom else 0.25):.1f}"'
    with open(out, 'w') as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
                f'width="{box_[2] * 10 / scale:.0f}" height="{box_[3] * 10 / scale:.0f}" viewBox="{" ".join(f"{v:.1f}" for v in box_)}">'
                f'<rect x="{box_[0]}" y="{box_[1]}" width="{box_[2]}" height="{box_[3]}" fill="#30373B"/>'
                f'<image xlink:href="{icon}" x="0" y="0" width="{width:.1f}" height="{height:.1f}" preserveAspectRatio="none"/>'
                + ''.join(path(g, stroke) for g in parts) + '</svg>')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--svg', help='a preview of the placed glyph')
    parser.add_argument('--overlay', nargs=2, metavar=('ICON', 'OUT'),
                        help='the upright sketch over the upright icon (and OUT with -bottom: its base)')
    args = parser.parse_args()
    parts = dict(zip(PARTS, glyph()))
    # the toxic smoke's green: the body and the cap inside their 1 px white edge
    inside = {'BODY_INSIDE': parts['BODY'].buffer(-TWIPS), 'CAP_INSIDE': parts['CAP'].buffer(-TWIPS)}
    for name, polygon in (*parts.items(), *inside.items()):
        print(f'      public static const {name}:Array = {ring(polygon)};'.replace(', ', ','))
    if args.svg:
        with open(args.svg, 'w') as out:
            out.write('<svg xmlns="http://www.w3.org/2000/svg" width="500" height="660" viewBox="-260 -970 500 660">'
                      '<rect x="-260" y="-970" width="500" height="660" fill="#30373B"/>'
                      + ''.join(path(g, 'fill="#fff"') for g in parts.values()) + '</svg>')
    if args.overlay:
        icon, out = args.overlay
        overlay(icon, out)
        overlay(icon, out.replace('.svg', '-bottom.svg'), bottom=True)


if __name__ == '__main__':
    main()

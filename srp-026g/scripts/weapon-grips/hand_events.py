#!/usr/bin/env python3
"""hand_events.py: ../content-edit/weapon-grips-hand-events-1.json, the left hand's events of 0.69d0's
own clips for a gun whose 0.26g clips never send the hand to its idle place.

usage: python3 -I hand_events.py <players' files> <0.69d0 files>
       (both at their game paths: animations/.../<clip>.b-spline)

A grip moves the left hand by the gun's idle hand locator (client patch 161 writes the hand IK's
idle record). The hand rests there only when a clip's `left_hand_ik` event sent it there (value
4); a gun whose clips leave it on the gun's `left_hand_cont` bone (value 2, set by its show clip)
would hold thin air beside a grip. Of the guns 0.69d0 fits a grip to, that is the UMP45
(docs/notes/attachments.md, "The left hand"). 0.69d0's own clips of the same names have the event,
so each clip of CLIPS gets the `left_hand_ik` channel 0.69d0's has, knot for knot, and nothing
else of it: the right hand and the fingers stay as 0.26g has them. A clip that has the channel
already, or whose 0.69d0 clip has none or sets it past our clip's end (the two builds' clips are
not all as long: the UMP45's shoot is 10 frames here, 15 there, its event at 0), stops this.
"""
import json
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

CHANNEL = 'left_hand_ik'
# gun: (its hands clips' folder, the clips of the states it is simply held in and comes back to)
CLIPS = {
    'ump45': ('animations/single/weapons/ump_45/player/stand/3rd_view/on_site/hands_only', ('idle', 'shoot', 'reload')),
}
LATER_EVENT = 12  # an event channel's header in 0.69d0 (0.26g: 16, content_edit.clip_parse)


def later_channels(b):
    """(length in frames, channels) of a clip of a later build: clip_parse's layout with 12-byte
    event headers."""
    bones, events, _ = ce.CLIP_HEAD.unpack_from(b, 0)
    heads_at = ce.CLIP_HEAD.size + bones * ce.CLIP_BONE
    off = tracks_at = heads_at + LATER_EVENT * events
    for _ in range(bones * 9):
        off += 4 + 8 * struct.unpack_from('<I', b, off)[0]
    end = ce.clip_end(bytes(b[tracks_at:off]))
    channels = []
    for i in range(events):
        knots, domains, kind = struct.unpack_from('<HBB', b, heads_at + LATER_EVENT * i + 8)
        values = list(struct.unpack_from('<%df' % knots, b, off))
        off += 4 * knots
        stop = b.index(b'\0', off + domains)
        channels.append({'name': bytes(b[off + domains:stop]).decode(), 'type': kind, 'knots': values,
                         'domains': list(b[off:off + domains])})
        off += (domains + stop - off - domains + 1 + 3) // 4 * 4
    if off != len(b):
        raise ValueError('a clip ends at %d, its file at %d' % (off, len(b)))
    return end, channels


def main():
    ours, theirs = Path(sys.argv[1]), Path(sys.argv[2])
    edits = []
    for gun, (folder, clips) in CLIPS.items():
        for clip in clips:
            path = f'{folder}/{clip}.b-spline'
            _, _, tracks, channels, _ = ce.clip_parse((ours / path).read_bytes())
            end, later = later_channels((theirs / path).read_bytes())
            new = [c for c in later if c['name'] == CHANNEL]
            if any(c['name'] == CHANNEL for c in channels) or len(new) != 1 \
                    or new[0]['knots'][-1] >= min(end, ce.clip_end(tracks)):
                sys.exit(f'{path}: ours has {[c["name"] for c in channels]} over {ce.clip_end(tracks):g} frames, '
                         f'0.69d0\'s {later} over {end:g}')
            edits.append({'file': path, 'from': channels, 'add': new[0]})
            print(f'{gun} {clip}: {CHANNEL} {list(zip(new[0]["knots"], new[0]["domains"]))}')
    out = HERE.parent / 'content-edit' / 'weapon-grips-hand-events-1.json'
    out.write_text(json.dumps({
        '_about': "The UMP45's left hand rests on its idle hand locator, where a grip can move it (docs/notes/"
                  "attachments-grips.md): its idle, shoot and reload hands clips get the left_hand_ik channel "
                  "0.69d0's own clips of the same names have (value 4 the idle locator, 2 the gun's left_hand_cont "
                  "bone during the reload). In 0.26g the show clip leaves the hand on that bone for good; in the "
                  "gun's own clips the bone sits on the idle locator (0.3 mm), so the hand without a grip is where "
                  "it was. Made by scripts/weapon-grips/hand_events.py.",
        'add_event_channel': edits}, indent=1) + '\n')


if __name__ == '__main__':
    main()

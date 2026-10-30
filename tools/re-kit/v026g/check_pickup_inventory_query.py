#!/usr/bin/env python3
"""Run original inventory query translation with weapon-only profile slots.

Dictionary lookup returns synthetic ammo configs; resource submission stops
at the original query call. All slot walks, config-type branching, variant
construction and request assembly execute. This does not load weapon assets.
"""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STACK, config_bytes
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP,
                              UC_X86_REG_EIP)

AMMO = [8, 9, 11, 12, 19, 20, 21, 22]


def translate(data, kit, weapon_slot, missing=(), extra=None, extra_type=0):
    g = Game(data)
    profile, recipe, variant, query, cook = (g.block(n) for n in [0x4634, 0x1C, 48, 0x200, 4])
    dictionary, entry, wrapper, config = (g.block(n) for n in [4, 0x120, 0x10C, 0x200])
    g.call(0x2F73F0, ecx=profile)
    g.emu.mem_write(profile + 0x57, b'\x03')
    g.emu.mem_write(profile + 0x1564, b'\x45')
    for k in range(3):
        for s in range(23):
            keep = k == kit and (s == weapon_slot or s in AMMO) and s not in missing
            descriptor = (k * 23 + s) | ((100 + s) << 16) if keep else 0xFFFF
            g.word(profile + 0x98 + k * 0x704 + s * 4, descriptor)
            item = profile + 0x1568 + (k * 23 + s) * 0xB4
            g.emu.mem_write(item + 4, struct.pack('<H', s))
    if extra is not None:
        g.word(profile + 0x98 + kit * 0x704 + extra * 4, (kit * 23 + extra) | (10 << 16))
    # Same synthetic ammo type at all dictionary entries; default helper
    # branch builds class48, as opposed to armor class49/type2.
    g.emu.mem_write(config, config_bytes({'data': {'type': 0}}, config))
    g.word(wrapper + 0x108, config)
    g.word(entry + 0x114, wrapper)
    g.word(entry + 0x118, config)  # unused metadata on type0 branch
    other_entry, other_wrapper, other_config, metadata = (g.block(n) for n in [0x120, 0x10C, 0x200, 4])
    g.emu.mem_write(other_config, config_bytes({'data': {'type': extra_type}}, other_config))
    g.word(other_wrapper + 0x108, other_config)
    g.word(other_entry + 0x114, other_wrapper)
    g.word(other_entry + 0x118, metadata)
    words = [profile, kit, dictionary, 0, 0x12345678, 0x87654321, 0]
    for n, word in enumerate(words):
        g.word(recipe + n * 4, word)
    g.call(0x205C60)
    g.word(variant + 8, recipe)
    g.word(variant + 0x2C, g.emu.reg_read(UC_X86_REG_EAX))
    g.word(query + 0x108, variant)
    lookups, submitted = [], []

    def endpoint(emu, at, _size, _user):
        esp = emu.reg_read(UC_X86_REG_ESP)
        if at == 0x1F0090:
            assert emu.reg_read(UC_X86_REG_EAX) == dictionary
            item_id = g.read(esp + 4)
            lookups.append(item_id)
            emu.reg_write(UC_X86_REG_EAX, other_entry if item_id == extra else entry)
            emu.reg_write(UC_X86_REG_EIP, g.read(esp))
            emu.reg_write(UC_X86_REG_ESP, esp + 8)
        elif at == 0xE5B60:
            requests = emu.reg_read(UC_X86_REG_EAX)
            count = emu.reg_read(UC_X86_REG_ECX)
            users = g.read(esp + 12)
            for n in range(count):
                class_id = g.read(requests + n * 8 + 4)
                user = g.read(users + n * 4)
                payload = bytes(g.emu.mem_read(user + 8, 28)) if user else None
                submitted.append((class_id, payload))
            emu.emu_stop()

    g.emu.hook_add(UC_HOOK_CODE, endpoint)
    g.call(0x2C7250, (query,), ecx=cook)
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0xE5B60
    classes = [class_id for class_id, _ in submitted]
    expected_ammo = [slot for slot in AMMO if slot not in missing]
    if extra is not None:
        expected_ammo.append(extra)
    assert lookups == expected_ammo
    # Native class0 fallback is only used when the entire request list is empty.
    expected_classes = [0x46] + [0x48] * (len(expected_ammo) - int(extra is not None))
    if extra is not None:
        expected_classes.append(0x49 if extra_type == 2 else 0x48)
    assert classes == expected_classes, classes
    weapon = struct.unpack('<7I', submitted[0][1])
    selected_item = profile + 0x1568 + (kit * 23 + weapon_slot) * 0xB4
    assert weapon[:6] == (selected_item + 0x14, selected_item + 0x60,
                         dictionary, words[4], weapon_slot, words[5]), weapon
    assert weapon[6] & 255 == 0  # native flag is one byte; padding is not initialized
    assert words[3] == 0  # no physics-world pointer used by these requests
    if extra_type == 2:
        # The negative control reaches armor's physics-world-dependent recipe.
        armor = struct.unpack('<7I', submitted[-1][1])
        assert armor[5:] == (words[3], words[4])
    return len(submitted)


def run(data):
    for kit in range(3):
        for slot in [7, 10]:
            assert translate(data, kit, slot) == 9
            assert translate(data, kit, slot, missing=(9, 20)) == 7
    # Negative control: a retained unrelated item really submits extra work.
    assert translate(data, 1, 7, extra=13) == 10
    assert translate(data, 1, 7, extra=0, extra_type=2) == 10
    print("native weapon-only inventory query: all kits/both weapons, original "
          "ammo order and holes, weapon attachments/world/seed, null physics "
          "context and extra-item/armor negative controls passed")


if __name__ == '__main__':
    run(Path(sys.argv[1]).read_bytes())

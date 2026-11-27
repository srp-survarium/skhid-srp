#!/usr/bin/env python3
"""Run patch 161 (the Rust patch attachments, rust/patches/src/attachments.rs
and attachments.toml over vostok::attachments: a weapon's fitted modules;
../docs/notes/attachments.md, "The client's runtime") in the default build
and the original.

    python check_attachments_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the game's own code runs between the sites, on made-up
objects. The modules table is the weapon-modules crate's checked-in sample
(../crates/weapon-modules/tests/sample-v4.srpm), and what must stand where is
that crate's own `resolve`, pinned beside it (sample-v4.loadouts.tsv, kept
by the crate's test): nothing here works a loadout out again.

- the table's way in: the items dictionary cook's query at its site
  (0x2C8963) asks for six resources, its five and the table as raw data;
  its callback's entry (0x2C8AA0) reads the sixth result through the game's
  pinned pointer (0x3DD990, 0x3C1020), which is left unpinned with its
  reference given back; a query of five, a missing file, another version's
  table: no table, the last with a line;
- the scope's dict id: a config that names a scope by data/srp_scope asks
  for that item's config first, class 4Dh, without any
  addons/rifle_scope_dict_id (the cook's site 0x24AA15), and the callback
  takes its first result as the scope (the site 0x24AD13, on at 0x24AD47, the cook's slot
  and the scope's out slot set as the skipped instructions set them); a
  scope the dictionary lacks, or none: the game's own checks;
- the cook (on_weapon_config_loaded 0x24A920 from its entry to its call of
  weapon_cook 0x24A780, then from the call's return, 0x24AC30): a config
  that names a gun with something fitted asks for the bare body in place of
  its model and for each part's model, class 15, after its own, inside the
  alloca; one that names nothing, nothing fitted or a refused fit, the
  original's requests;
- the callback: its entry (0x24ACF0) and after load_weapon (0x24B0E1): each
  part's model retained once; the barrel, scope and aim records and the left
  hand's idle and lobby finger sets as the loadout says, the other sets as
  they were; a model with fewer bones than the table's gun: the records as
  they were, the models given back, a line;
- weapon::draw's end (0x245017): each part added at the frame the game's own
  calculate_locator 0x243BC0 gives its record, then updated, drawn as the
  weapon's model is (the two bytes); the weapon's model out of the scene:
  removed; culled (0x245353) and hidden (0x244017): removed;
- weapon::activate (0x244649): the left hand's idle and lobby records in
  both views; the game's refill of the fingers (0x298664): the two sets
  again; the shell effect (0x2443EE): the frame the scope had;
- delete_resource (0x24B190): the parts out of the scene, their references
  given back once, the weapon forgotten;
- a weapon without any of it, and every site in the original: as the game;
- patch 218, the player's weapon view: on_configs_loaded's start (0x1EA210)
  registers its five variables at their defaults; set by their own setters as user.cfg's
  lines are (0xECB70, 0xECA30), the camera's lens
  scales by them (a rifle's hip and aimed angles, a fitted magnified
  scope's), the checkbox gives no lens at all (the world's projection);
  the sight distance at 0, 50 and 100 % moves the hands' alignment
  (process_hands_ik 0x295AA5 through calculate_aim_alignment to 0x295AB8)
  to what the original gives with the aim_offset 0 %, half and all of the
  way to the row's aim_offset_026g (100 %: bit for bit the default at 0;
  revision 73's at 100), and not for a weapon the camera did not sample.
"""
import math
import struct
import sys
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

SAMPLE = Path(__file__).resolve().parent.parent / "crates" / "weapon-modules" / "tests"
TABLE_PATH = b"resources/gameplay/weapon_modules.srpm"

DICT_QUERY, DICT_QUERY_END = 0x2C8963, 0x2C896C     # translate_query: the site, its query_resources call
DICT_CONFIGS, DICT_CONFIGS_END = 0x2C8AA0, 0x2C8AA9
COOK, COOK_ROOM, COOK_WEAPON, ASKED, ASKED_END = 0x24A920, 0x24AAB4, 0x24AC2B, 0x24AC30, 0x24AC36
READY, READY_END = 0x24ACF0, 0x24ACF5
SCOPE_RESULT, SCOPE_RESULT_END, SCOPE_RESULT_SCOPED = 0x24AD0E, 0x24AD1B, 0x24AD47
LOADED, LOADED_END = 0x24B0E1, 0x24B0E9
DRAW, DRAW_END = 0x245017, 0x24501C
CULLED, CULLED_END = 0x245353, 0x245359
HIDDEN, HIDDEN_END = 0x244017, 0x24401D
HANDS, HANDS_END = 0x244649, 0x24464F
FINGERS_VIEW, FINGERS_VIEW_END, SET_WEAPON_MODEL = 0x298664, 0x298669, 0x390620
SHELLS, SHELLS_END = 0x2443EE, 0x2443F4
RELEASE, RELEASE_END = 0x24B190, 0x24B197
CALCULATE = 0x243BC0
CHILD, HAS, RELEASE_PTR, DESTROY, MALLOC, FREE = 0x0F6B10, 0x0F6C00, 0x011540, 0x0E0210, 0x0E4F90, 0x1539D0
ADD, REMOVE, UPDATE = 0x21CD0, 0x21F40, 0x22280
SKELETON_INSTANCE, GAME_ALLOCATOR = 0x9F0F24, 0x1F1D9C4
T_INT, T_TABLE, T_STRING = 1, 3, 5
BARREL, SCOPE, AIM, FRAME, HAND_RECORDS, FINGER_SETS = 0x6E0, 0x744, 0x7A8, 0x660, 0x1A20, 0x20F0

VIEW_CAMERA, VIEW_NO_GUN, VIEW_SUBMIT, VIEW_BEGIN, VIEW_END, FOREGROUND = 0x1F3505, 0x1F3579, 0x1F8F7, 0x1F796, 0x1F7BA, 0x13E0C2
VIEW_ENDS = (VIEW_CAMERA + 6, VIEW_NO_GUN + 6, VIEW_SUBMIT + 6, VIEW_BEGIN + 8, VIEW_END + 6, FOREGROUND + 7)
CONFIGS_START, CONFIGS_START_END = 0x1EA210, 0x1EA216     # on_configs_loaded's start, after its replayed prologue
ALIGN, ALIGNED = 0x295AA5, 0x295AB8                       # process_hands_ik before calculate_aim_alignment; after
LIST_HEAD = 0x1F1D66C                                     # the console's list of variables
ENDS = VIEW_ENDS + (CONFIGS_START_END, ALIGNED, DICT_QUERY_END, DICT_CONFIGS_END, COOK_WEAPON, ASKED_END, READY_END, LOADED_END, DRAW_END, CULLED_END,
        HIDDEN_END, HANDS_END, FINGERS_VIEW_END, SHELLS_END, RELEASE_END, SCOPE_RESULT_END, SCOPE_RESULT_SCOPED)

AK_ID = 3        # the sample's AK-74M

# The scope items of the made-up dictionary: the sample's PSO-1 and EOTech.
SCOPE_ITEMS = {1755: "gameplay/items/scopes/pso1", 1771: "gameplay/items/scopes/eotech"}

STACK, STOP, HEAP = 0x21000000, 0x21100000, 0x30000000
RENDERER = 0x31000000
REGS = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "ebx": UC_X86_REG_EBX,
        "esi": UC_X86_REG_ESI, "edi": UC_X86_REG_EDI, "ebp": UC_X86_REG_EBP}


# ------------------------------------------------------------ the model

def multiply(a, b):
    """multiply_affine 0xEE700: a then b, row vectors, the w column 0, 0, 0, 1."""
    out = [0.0] * 16
    for r in range(4):
        for c in range(3):
            out[4 * r + c] = sum(a[4 * r + k] * b[4 * k + c] for k in range(3)) + (b[12 + c] if r == 3 else 0.0)
    out[15] = 1.0
    return out


def rotation_y(angle):
    c, s = math.cos(angle), math.sin(angle)
    return [c, 0, s, 0, 0, 1, 0, 0, -s, 0, c, 0, 0, 0, 0, 1]


def translate(x, y, z):
    return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]


IDENTITY = translate(0, 0, 0)
PI = struct.unpack("<f", struct.pack("<f", math.pi))[0]


def matrix(twelve):
    """An affine's twelve floats (rows x, y, z, t) as the game's sixteen."""
    return [v for row in range(4) for v in (*twelve[3 * row:3 * row + 3], 1.0 if row == 3 else 0.0)]


def frame_of(offset, bone, world):
    """calculate_locator: the extra (identity), R_y(pi), the locator's matrix,
    the bone's, the world's."""
    return multiply(multiply(multiply(multiply(IDENTITY, rotation_y(PI)), offset), bone), world)


def close(a, b, eps=1e-5):
    return a is not None and b is not None and len(a) == len(b) and all(abs(x - y) <= eps for x, y in zip(a, b))


def loadouts():
    """sample-v4.loadouts.tsv: {(gun, scope, muzzle, grip): a case}, with a
    second sight {(gun, scope, muzzle, grip, second sight, side)}. A case is
    "whole", ("refused", why) or a dict: body, bones, parts [(model, bone,
    matrix)], optic, eye and eye2 (by anchor), second, fire, wrist (each
    (bone, matrix)), fingers (fifteen matrices, or None), effect, aimed (the
    second sight's aimed model)."""
    cases, case = {}, None
    for line in (SAMPLE / "sample-v4.loadouts.tsv").read_text().splitlines():
        head, *rest = line.split("\t")
        placed = lambda cells: (int(cells[0]), matrix([float(v) for v in cells[1:13]]))
        if head == "case":
            ids = tuple(int(v) for v in rest[:5])
            key = ids[:4] if ids[4] == 0 else ids + ({"left": 0, "right": 1}[rest[5]],)
        elif head == "whole":
            cases[key] = "whole"
        elif head == "refused":
            cases[key] = ("refused", rest[0])
        elif head == "bare":
            case = cases[key] = {"body": rest[0], "bones": int(rest[1]), "parts": [], "eye": {}, "eye2": {},
                                 "fingers": None}
        elif head == "part":
            case["parts"].append((rest[0], *placed(rest[1:])))
        elif head in ("optic", "fire", "wrist", "second"):
            case[head] = placed(rest)
        elif head in ("eye", "eye2"):
            case[head][rest[0]] = placed(rest[1:])
        elif head == "fingers":
            floats = [float(v) for v in rest]
            case["fingers"] = [matrix(floats[12 * n:12 * n + 12]) for n in range(15)]
        elif head in ("effect", "aimed"):
            case[head] = rest[0]
        else:
            raise ValueError(line)
    return cases


def own_fingers():
    """The crate's own_fingers: what the game is taken to have made of the
    gun's own finger locators."""
    return [translate(0.001 * n, 0.002 * n, 0.003 * n) for n in range(1, 16)]


# ------------------------------------------------------------ the runs

class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(STACK - 0x40000, 0x80000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x1000, UC_PROT_ALL)
        emu.mem_map(HEAP, 0x800000, UC_PROT_ALL)
        emu.mem_map(RENDERER, 0x201000, UC_PROT_ALL)
        self.next = HEAP
        self.children = {}
        self.rendered, self.destroyed, self.freed, self.fingers_filled = [], [], [], 0
        for at in (CHILD, HAS, RELEASE_PTR, DESTROY, ADD, REMOVE, UPDATE, MALLOC, FREE, SET_WEAPON_MODEL):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        # a run stops at the end it names: a hook an end, none on every instruction
        for at in ENDS:
            emu.hook_add(UC_HOOK_CODE, self.stop, begin=at, end=at)
        self.ends = ()
        # the game's allocator's slot: any address, the stubs do the work
        self.u32(GAME_ALLOCATOR, self.block(0x40))
        # kernel32's GetCurrentThreadId (the Lock's owner): 1; the log's
        # CreateFileA, WriteFile (its lines kept) and CloseHandle
        stub = STOP + 0x800
        self.u32(0x870118, stub)
        emu.mem_write(stub, bytes.fromhex("b801000000c3"))
        self.logged = b""
        for slot, at, code in ((0x870048, STOP + 0x820, "b855000000c21c00"), (0x870108, STOP + 0x840, None),
                               (0x870040, STOP + 0x860, "b801000000c20400")):
            self.u32(slot, at)
            if code:
                emu.mem_write(at, bytes.fromhex(code))
        emu.hook_add(UC_HOOK_CODE, self.log, begin=STOP + 0x840, end=STOP + 0x840)

    def block(self, size):
        at = self.next
        self.next += (size + 0xF) & ~0xF
        self.emu.mem_write(at, bytes(size))
        return at

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def u16(self, at):
        return struct.unpack("<H", self.emu.mem_read(at, 2))[0]

    def byte(self, at):
        return self.emu.mem_read(at, 1)[0]

    def floats(self, at, n=16):
        return list(struct.unpack(f"<{n}f", self.emu.mem_read(at, 4 * n)))

    def put_floats(self, at, values):
        self.emu.mem_write(at, struct.pack(f"<{len(values)}f", *values))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 128)).split(b"\0")[0]

    def string(self, text):
        at = self.block(len(text) + 1)
        self.emu.mem_write(at, text.encode() + b"\0")
        return at

    def text_node(self, text):
        """A text node (type 5): its string, the count its bytes with the zero."""
        return self.node(T_STRING, self.string(text), len(text.encode()) + 1)

    def node(self, kind=T_INT, data=0, count=0):
        """A config node (18h bytes: the value, the type at +14h, the count +16h)."""
        at = self.block(0x18)
        self.u32(at, data)
        self.emu.mem_write(at + 0x14, struct.pack("<HH", kind, count))
        return at

    def table(self, entries):
        """A table node whose children the lookup stubs give by name."""
        at = self.node(T_TABLE, self.block(0x18 * max(1, len(entries))), len(entries))
        for name, child in entries.items():
            self.children[(at, name.encode())] = child
        return at

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        pops, value = 0, 0
        if address in (CHILD, HAS):
            table = emu.reg_read(UC_X86_REG_EAX)
            key = self.text(emu.reg_read(UC_X86_REG_EDI if address == CHILD else UC_X86_REG_ESI))
            child = self.children.get((table, key))
            if address == CHILD:
                if child is None:
                    raise KeyError(f"child {table:#x} {key!r}")
                value = child
            else:
                value = 1 if child is not None else 0
        elif address == RELEASE_PTR:
            pass
        elif address == DESTROY:
            self.destroyed.append(emu.reg_read(UC_X86_REG_ECX))
        elif address == MALLOC:                     # ESI the allocator, EAX the size, one stack argument
            value, pops = self.block(emu.reg_read(UC_X86_REG_EAX)), 4
        elif address == FREE:                       # ECX the allocator, EAX the memory; its caller pops
            self.freed.append(emu.reg_read(UC_X86_REG_EAX))
        elif address == SET_WEAPON_MODEL:           # stdcall: the corrector, &instance, first view
            corrector = self.u32(sp + 4)
            for n in range(4):
                self.put_floats(corrector + 0x3C0 * n, [v for m in own_fingers() for v in m])
            self.fingers_filled += 1
            pops = 12
        else:
            reference = emu.reg_read(UC_X86_REG_EAX)
            instance = self.u32(reference)
            op = {ADD: "add", REMOVE: "remove", UPDATE: "update"}[address]
            before = self.byte(instance + 0x10D)
            assert (op != "add" or before == 0) and (op != "remove" or before == 1), (op, before)
            emu.mem_write(instance + 0x10D, bytes([0 if op == "remove" else 1]))
            at = None if op == "remove" else self.floats(self.u32(sp + 12))
            self.rendered.append((op, instance, at))
            pops = 8 if op == "remove" else 16
        emu.reg_write(UC_X86_REG_EAX, value)
        emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def log(self, emu, address, size, _):
        """WriteFile(handle, bytes, size, &written, 0): stdcall, ret 14h."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        data, size_ = self.u32(sp + 8), self.u32(sp + 12)
        self.logged += bytes(emu.mem_read(data, size_))
        if self.u32(sp + 16):
            self.u32(self.u32(sp + 16), size_)
        emu.reg_write(UC_X86_REG_EAX, 1)
        emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 20)

    def stop(self, emu, address, size, _):
        if address in self.ends:
            emu.emu_stop()

    def run(self, start, ends, sp=None, **regs):
        for name, value in regs.items():
            self.emu.reg_write(REGS[name], value)
        if sp is not None:
            self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.ends = tuple(ends)
        self.emu.emu_start(start, STOP, count=400_000_000)
        self.ends = ()
        return self.emu.reg_read(UC_X86_REG_EIP)

    def settings(self):
        """on_configs_loaded's start: where it stopped and {name: value
        address} of the variables it registered."""
        before = self.u32(LIST_HEAD)
        sp = STACK - 0x2000
        self.u32(sp, STOP)
        stopped = self.run(CONFIGS_START, (CONFIGS_START_END,), sp)
        out, node = {}, self.u32(LIST_HEAD)
        self.nodes = {}
        while node and node != before:
            name = self.text(self.u32(node + 0x10)).decode()
            out[name] = self.u32(node + 0x40)
            self.nodes[name] = node
            node = self.u32(node + 0x0C)
        return stopped, out

    def console(self, name, text):
        """A user.cfg line `<name> <text>` as the console runs it: the variable's own setter (its
        vtable's slot 1: cc_float's 0xECB70, cc_bool's 0xECA30), sscanf (0x422B46) answered here."""
        node = self.nodes[name]
        at = self.block(0x40)
        self.emu.mem_write(at, text.encode() + b"\0")
        sp = STACK - 0x2800
        self.u32(sp, STOP)
        self.u32(sp + 4, at)
        self.run(self.u32(self.u32(node) + 4), (), sp, ecx=node)

    def sscanf(self, emu, address, size, _):
        """sscanf(text, "%f", &out), cdecl: the text's float, 1 field (0 for none)."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        text, out = self.text(self.u32(sp + 4)).decode(), self.u32(sp + 12)
        try:
            self.put_floats(out, [float(text)])
            value = 1
        except ValueError:
            value = 0
        emu.reg_write(UC_X86_REG_EAX, value)
        emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4)

    # -------------------------------------------------- the table

    def dictionary_query(self):
        """The dictionary cook's query at its site: [(path, class)] it hands
        query_resources (EAX the requests, ECX their count)."""
        sp = STACK - 0x1000
        paths = [self.string(f"resources/gameplay/config_{n}") for n in range(5)]
        for n, path in enumerate(paths):
            self.u32(sp + 0x3C + 8 * n, path)
            self.u32(sp + 0x40 + 8 * n, 0x1F)
        stopped = self.run(DICT_QUERY, (DICT_QUERY_END,), sp, eax=0, ecx=0)
        if stopped != DICT_QUERY_END:
            return None
        requests, count = self.emu.reg_read(UC_X86_REG_EAX), self.emu.reg_read(UC_X86_REG_ECX)
        return [(bytes(self.text(self.u32(requests + 8 * n))), self.u32(requests + 8 * n + 4)) for n in range(count)]

    def dictionary_configs(self, count, table, dictionary=0):
        """The dictionary cook's callback at its entry, its query's results
        `count` and the sixth's managed resource holding `table` (None: 0).
        The resource: its reference count, its node's pin count."""
        queries = self.block(0x50 + 0x2E0 * 6)
        self.u32(queries + 0x38, count)
        self.u32(queries + 0x12C, dictionary)
        resource = node = 0
        if table is not None:
            resource, node = self.block(0xF0), self.block(0x34 + len(table))
            self.emu.mem_write(node + 0x34, table)
            self.u32(resource + 0xD4, node)
            self.u32(resource + 0x5C, len(table))
            self.u32(resource + 0xDC, 1)
            self.u32(queries + 0x128 + 0x2E0 * 5, resource)
        sp = STACK - 0x1800
        self.u32(sp, STOP)
        self.u32(sp + 4, queries)
        stopped = self.run(DICT_CONFIGS, (DICT_CONFIGS_END,), sp, ecx=self.block(0x40))
        return stopped, (self.u32(resource + 0xDC), self.u32(node + 0x2C)) if resource else None

    # -------------------------------------------------- objects

    def config(self, data=None):
        """A weapon config's resource: one shell, two shot effects, one
        preview animation, no scope; `data` its data table's ints (None: no
        data table)."""
        resource = self.block(0x140)
        particles = self.table({"bullet_shells_count": self.node(T_INT, 1), "shoot_pfx_count": self.node(T_INT, 2),
                                "bullet_shells": self.text_node("shells/5_45x040"),
                                "shoot": self.text_node("fx/weapons/ak_74")})
        animation = self.text_node("weapons/ak_74/preview")
        preview = self.node(4, animation, 1)
        entries = {"particles": particles, "object": self.table({"model": self.text_node("weapons/ak74m/ak74m")}),
                   "user_animations_in_place": self.table({"preview": preview})}
        if data is not None:
            entries["data"] = self.table({k: self.node(T_INT, v) for k, v in data.items()})
        root = self.table(entries)
        self.u32(resource + 0x108, root)
        self.u32(resource + 0xD0, 1)
        return resource, root

    def weapon_cook(self):
        """The weapon cook: its game's items dictionary holds SCOPE_ITEMS,
        each entry its dict id and its config's path (+118h)."""
        cook, game, dictionary = self.block(0x40), self.block(0x5400), self.block(0x140)
        entries = self.block(0x3A0 * len(SCOPE_ITEMS))
        for n, (dict_id, path) in enumerate(sorted(SCOPE_ITEMS.items())):
            self.emu.mem_write(entries + 0x3A0 * n, struct.pack("<H", dict_id))
            self.u32(entries + 0x3A0 * n + 0x118, self.string(path))
        self.u32(dictionary + 0x130, entries)
        self.u32(dictionary + 0x134, len(SCOPE_ITEMS))
        self.u32(game + 0x53A4, dictionary)
        self.u32(cook + 0x24, game)
        return cook

    def scope_result(self, root):
        """The callback at its question whether the weapon has a scope: where
        it goes on (the game's own checks, or the scope's taking), and the
        two slots the skipped instructions set."""
        cook, sp = self.weapon_cook(), STACK - 0x7000
        self.emu.mem_write(sp, b"\xEE" * 0x40)
        stopped = self.run(SCOPE_RESULT, (SCOPE_RESULT_END, SCOPE_RESULT_SCOPED), sp, eax=root, ecx=cook, ebx=0)
        return stopped, self.u32(sp + 0x30) == cook, self.u32(sp + 0x24)

    def cook(self, resource, weapon):
        """on_weapon_config_loaded to its weapon_cook call, then from the
        call's return with `weapon` its result to the end of the site's
        bytes: the requests [(path, class)], whether they fit their alloca,
        and the alloca's bytes."""
        cook = self.weapon_cook()
        # its argument (the config's query) holds the config's resource at
        # +12Ch (0x24A92F; retained into [ebp+8]), its parent query at +20h
        query = self.block(0x140)
        self.u32(query + 0x12C, resource)
        sp = STACK - 0x2000
        for i, value in enumerate((STOP, query, 0, 0, 0)):
            self.u32(sp + 4 * i, value)
        stopped = self.run(COOK, (COOK_WEAPON,), sp, ecx=cook, ebx=0x11111111, esi=0x33333333, edi=0x44444444,
                           ebp=0x22222222)
        if stopped != COOK_WEAPON:
            return None
        # weapon_cook(root, ...) takes five stack arguments and pops them
        after = self.emu.reg_read(UC_X86_REG_ESP) + 0x14
        if self.run(ASKED, (ASKED_END,), after, eax=weapon) != ASKED_END:
            return None
        ebp = self.emu.reg_read(UC_X86_REG_EBP)
        begin, end, capacity = (self.u32(ebp - 0x40 + 4 * i) for i in range(3))
        requests = [(self.text(self.u32(at)).decode(), self.u32(at + 4)) for at in range(begin, end, 8)]
        return requests, end <= capacity, capacity - begin

    def weapon(self, bones, records):
        """A client weapon whose model is a skeleton model instance of
        `bones` bones, in the scene, with its world and renderer; its
        barrel, scope and aim records `records` (three (bone, matrix)), its
        hand records and finger sets as the game would have made them."""
        weapon, holder, instance, model = self.block(0x4800), self.block(0x120), self.block(0x6300), self.block(0x200)
        self.u32(weapon + 0x1810, holder)
        self.u32(holder + 0x108, instance)
        self.u32(instance, SKELETON_INSTANCE)
        self.u32(instance + 0x62B4, model)
        binds = self.block(0x40 * max(1, bones))
        self.u32(model + 0x140, binds)
        self.u32(model + 0x144, binds + 0x40 * bones)
        world, game, scene = self.block(0x100), self.block(0x5400), self.block(0x100)
        self.u32(world + 0x9C, game)
        self.u32(game + 0x5360, RENDERER)
        self.u32(RENDERER + 0x200064, scene)
        self.u32(world + 4, scene)
        self.u32(weapon + 0x180C, world)
        self.emu.mem_write(instance + 0x10D, b"\1")
        self.emu.mem_write(instance + 0x114, b"\1\1")
        for at, (bone, offset) in zip((BARREL, SCOPE, AIM), records):
            self.record(weapon + at, bone, offset)
        for n in range(8):
            self.record(weapon + HAND_RECORDS + 0x64 * n, 2, translate(0.5 + n, 0.25, 0.125))
        for n in range(4):
            self.put_floats(weapon + FINGER_SETS + 0x3C0 * n, [v for m in own_fingers() for v in m])
        return weapon, instance

    def record(self, at, bone, offset):
        self.put_floats(at + 0x20, offset)
        self.emu.mem_write(at + 0x60, struct.pack("<H", bone))

    def read_record(self, at):
        return self.u16(at + 0x60), self.floats(at + 0x20)

    def part(self):
        """A part's model holder (a static model's): its instance, count 1."""
        holder, instance = self.block(0x120), self.block(0x300)
        self.u32(holder + 0x108, instance)
        self.u32(holder + 0xD0, 1)
        return holder, instance

    def callback(self, weapon, resource, results, reach=True):
        """The cook's callback: its entry ([esp+4] the results, one a
        resource each, [esp+8] the config, [esp+0Ch] the weapon), then, as
        the game's shots loop does before load_weapon, its argument slots
        taken as temporaries, then the site after load_weapon (EDI the
        weapon) when `reach`."""
        queries = self.block(0x50 + 0x2E0 * max(1, len(results)))
        self.u32(queries + 0x38, len(results))
        for i, resource_ in enumerate(results):
            self.u32(queries + 0x50 + 0x2E0 * i + 0xDC, resource_)
        entry = STACK - 0x3000
        self.emu.mem_write(entry - 0x40, bytes(0x80))
        self.u32(entry, STOP)
        self.u32(entry + 4, queries)
        self.u32(entry + 8, resource)
        self.u32(entry + 0xC, weapon)
        stopped = self.run(READY, (READY_END,), entry)
        if stopped != READY_END or not reach:
            return stopped
        sp = entry - 0x34                   # sub esp,24h and four pushes
        self.u32(sp + 0x38, 0)              # the shots' temporaries
        self.u32(sp + 0x40, self.block(0x120))
        return self.run(LOADED, (LOADED_END,), sp, edi=weapon)

    def matrices(self, world, bones):
        w, b = self.block(64), self.block(64 * len(bones))
        self.put_floats(w, world)
        for i, bone in enumerate(bones):
            self.put_floats(b + 64 * i, bone)
        return w, b

    def draw(self, weapon, world, bones):
        frame = STACK - 0x4000
        w, b = self.matrices(world, bones)
        self.u32(frame + 8, w)
        self.u32(frame + 0xC, b)
        self.u32(frame + 0x14, 0)
        self.rendered = []
        return self.run(DRAW, (DRAW_END,), STACK - 0x5000, ebx=weapon, ebp=frame)

    def calculate(self, bone, offset, world, bones):
        """The game's own calculate_locator 0x243BC0 (EDI the out, ret 10h)."""
        loc, extra, out = self.block(0x64), self.block(64), self.block(64)
        self.record(loc, bone, offset)
        w, b = self.matrices(world, bones)
        self.put_floats(extra, IDENTITY)
        sp = STACK - 0x6000
        for i, value in enumerate((STOP, loc, w, b, extra)):
            self.u32(sp + 4 * i, value)
        self.run(CALCULATE, (STOP,), sp, edi=out)
        return self.floats(out)


def weapon_projection(exe, patched, report):
    """The first-person lens through the camera (a gun in hand, 0x1F3505;
    nothing or no gun, 0x1F3579), the scene's draw command (0x1F8F7, read
    back at 0x1F796) and the foreground pass (0x13E0C2). Frames are
    submitted before any is drawn, as on a busy renderer; one command is made
    by the game's own constructor (0x20310) over garbage, handed to its
    on_defer_execution (0x28620) and run by its execute (0x28460), each
    functor's invoker the run's stop."""
    r = Run(exe)
    player, weapon, world, entry, config = [r.block(n) for n in (0xF480, 0x1900, 0x10, 0x118, 0x10C)]
    view = r.block(0x100)                       # a scene view: its reference count at +D0h
    r.u32(player + 0xF474, world)
    r.u32(world + 8, view)
    r.u32(weapon + 0x26C, entry)
    r.u32(entry, 220)

    def pair(hip, aimed):
        at = r.block(8)
        r.put_floats(at, (hip, aimed))
        return r.node(6, at, 8)

    rows = [{"gun": r.node(T_INT, 220), "fov": pair(1.0, 0.5)},
            {"gun": r.node(T_INT, 221), "fov": pair(1.5, 0.5)},
            {"gun": r.node(T_INT, 220), "fov": pair(0.75, 0.6)},
            {"gun": r.text_node("a545"), "fov": pair(1.0, 0.5)}]
    # the native iteration reads the rows inline in their parent's child array
    array = r.block(0x18 * len(rows))
    for n, entries in enumerate(rows):
        at = array + 0x18 * n
        r.u32(at, r.block(0x18 * len(entries)))
        r.emu.mem_write(at + 0x14, struct.pack("<HH", T_TABLE, len(entries)))
        for name, child in entries.items():
            r.children[(at, name.encode())] = child
    r.u32(config + 0x108, r.table({"srp_weapon_views": r.node(T_TABLE, array, len(rows))}))
    r.dictionary_configs(5, None, config)
    if patched:
        report(all(f"row {n}".encode() in r.logged for n in (1, 2, 3)) and b"row 0" not in r.logged,
               "srp_weapon_views: a factor above 1, a gun twice and a row with no dict id each left out with a line, "
               "the first row kept")
    world_projection = [1.125, 0, 0, 0, 0, 2, 0, 0, 0.001, -0.002, 1.01, 1, 0, 0, -0.1, 0]

    def sample(progress, optics=0):
        r.put_floats(weapon + 0x5FC, [progress])
        r.u32(weapon + 0x1808, optics)
        r.run(VIEW_CAMERA, VIEW_ENDS, STACK, eax=weapon, ebx=player)

    def submit(to):
        command = r.block(0xB8)
        r.emu.mem_write(command + 0x14, b"\xCD" * 4)     # the constructor leaves it
        r.u32(command + 0x9C, to)
        r.run(VIEW_SUBMIT, VIEW_ENDS, STACK, eax=command, ebx=RENDERER)
        return command

    def project():
        r.put_floats(STACK + 0x28, world_projection)
        r.run(FOREGROUND, VIEW_ENDS, STACK)
        return r.floats(STACK + 0x28)

    def drawn(params):
        r.run(VIEW_BEGIN, VIEW_ENDS, STACK, esi=params)
        got = project()
        r.run(VIEW_END, VIEW_ENDS, STACK)
        return got, project()

    def lens(angle):
        out = world_projection.copy()
        if patched and angle:
            out[5] = 1.0 / math.tan(math.radians(angle) / 2)
            out[0] = world_projection[0] / world_projection[5] * out[5]
        return out

    sample(1.0)
    aimed = submit(view)
    other_view = submit(0x5678)
    sample(0.5)
    moving = submit(view)
    sample(1.0, optics=r.block(0x120))
    own_optic = submit(view)
    r.u32(entry, 999)
    sample(1.0)
    unknown = submit(view)
    r.run(VIEW_NO_GUN, VIEW_ENDS, STACK, ebx=player)
    no_gun = submit(view)

    # the game's own constructor over garbage, its defer and execute slots
    sample(1.0)
    r.u32(entry, 220)
    sample(1.0)
    vt, run_f, defer_f, params = r.block(0x10), r.block(0x20), r.block(0x20), r.block(0x20)
    r.u32(vt + 4, STOP)
    r.u32(run_f, vt | 1)
    r.u32(defer_f, vt | 1)
    r.u32(params + 4, view)
    made = r.block(0xB8)
    r.emu.mem_write(made, b"\xCD" * 0xB8)
    sp = STACK - 0x800
    for n, value in enumerate((STOP, run_f, params, defer_f)):
        r.u32(sp + 4 * n, value)
    r.run(0x20310, (), sp, edi=made, ecx=r.block(0x40))
    left = r.u32(made + 0x14) == 0xCDCDCDCD and r.u32(made + 0x9C) == view and r.byte(made + 0x10) == 1
    r.run(VIEW_SUBMIT, VIEW_ENDS, STACK, eax=made, ebx=RENDERER)
    r.run(0x28620, (), STACK - 0x800, ecx=made)
    handed = r.u32(r.emu.reg_read(UC_X86_REG_ESP) + 8)
    r.run(0x28460, (), STACK - 0x800, ecx=made)
    payload = r.u32(r.emu.reg_read(UC_X86_REG_ESP) + 8)
    report(left and handed == made and payload == made + 0x98,
           f"the constructor leaves +14h ({r.u32(made + 0x14):#x} after the submit), on_defer_execution is handed the "
           f"command itself, execute its payload at +98h (patched={patched})")

    for command, angle, what in [(aimed + 0x98, 32.5, "aimed"), (other_view + 0x98, None, "another view"),
                                 (moving + 0x98, 48.75, "half aimed"), (own_optic + 0x98, None, "its own optic"),
                                 (unknown + 0x98, 65.0, "a gun with no profile"), (no_gun + 0x98, 65.0, "no gun"),
                                 (payload, 32.5, "the deferred one")]:
        got, after = drawn(command)
        want = f"{angle} degrees" if patched and angle else "the world's"
        report(close(got, lens(angle)) and close(after, world_projection),
               f"foreground draw, {what}: {want} (patched={patched}), the world's after it")


def fitted_optic_projection(run, weapon, report, scope=None):
    """A gun whose optic is the table's (its config's data/srp_scope, its
    parts held) aimed through it: the lens, though the weapon has an optic
    (+1808h); the gun has no profile here, so 65 degrees (with patch 218's
    scope angle `scope`, 65 times it over 32.5)."""
    player, world, entry, view, command = (run.block(n) for n in (0xF480, 0x10, 0x118, 0x100, 0xB8))
    run.u32(player + 0xF474, world)
    run.u32(world + 8, view)
    run.u32(weapon + 0x26C, entry)
    run.u32(entry, AK_ID)
    run.u32(weapon + 0x1808, run.block(0x120))
    run.put_floats(weapon + 0x5FC, [1.0])
    run.run(VIEW_CAMERA, VIEW_ENDS, STACK, eax=weapon, ebx=player)
    run.u32(command + 0x9C, view)
    run.run(VIEW_SUBMIT, VIEW_ENDS, STACK, eax=command, ebx=RENDERER)
    run.run(VIEW_BEGIN, VIEW_ENDS, STACK, esi=command + 0x98)
    world_projection = [1.125, 0, 0, 0, 0, 2, 0, 0, 0.001, -0.002, 1.01, 1, 0, 0, -0.1, 0]
    run.put_floats(STACK + 0x28, world_projection)
    run.run(FOREGROUND, VIEW_ENDS, STACK)
    want = world_projection.copy()
    angle = 65.0 * (scope / 32.5 if scope else 1.0)
    want[5] = 1.0 / math.tan(math.radians(angle) / 2)
    want[0] = world_projection[0] / world_projection[5] * want[5]
    report(close(run.floats(STACK + 0x28), want),
           f"a fitted optic aimed (the PSO-1 on the AK-74M): the lens, {angle:g} degrees"
           + (f" (patch 218's scope angle {scope:g})" if scope else ""))


# the hands' alignment's made-up rig: the aim's place on bone 1, the bone, the camera
ALIGN_LOCATOR = multiply(rotation_y(0.009), translate(0.0089, 0.1019, 0.1328))
ALIGN_BONE = multiply(rotation_y(-0.0148), translate(0.0088, 0.0214, 0.05))
ALIGN_ITEM = multiply(rotation_y(0.3), translate(1.0, 1.5, -2.0))


def align(run, weapon, aim_offset):
    """process_hands_ik from its site before calculate_aim_alignment to the
    blend's test after it (0x295AA5 to 0x295AB8): the weapon's aim offset
    matrix a translation (0, 0, aim_offset), its aim record on bone 1; the
    alignment in the frame."""
    bones, item = run.block(0x80), run.block(0x40)
    run.put_floats(weapon + 0x6A0, translate(0.0, 0.0, aim_offset))
    run.record(weapon + AIM, 1, ALIGN_LOCATOR)
    run.put_floats(bones, IDENTITY)
    run.put_floats(bones + 0x40, ALIGN_BONE)
    run.put_floats(item, ALIGN_ITEM)
    sp = STACK - 0x3000
    run.emu.mem_write(sp, bytes(0x100))
    run.u32(sp, item)                       # the pushed Item bone
    run.u32(sp + 0x14, 0x77770000)          # the controller: not read
    run.u32(sp + 0x1C, weapon)
    run.put_floats(sp + 0x10, [1.0])        # [esp+0Ch] after the call's ret 4: the aim's progress
    stopped = run.run(ALIGN, (ALIGNED,), sp, ecx=bones)
    return stopped, run.floats(sp + 0x24)


def weapon_view_settings(original, built, report):
    """Patch 218 (weapon_view_options.rs): its variables, the lens they make,
    the sight distance's move of the hands' alignment."""
    r = Run(built)
    stopped, values = r.settings()
    defaults = {"srp_weapon_fov": 65.0, "srp_weapon_fov_aimed": 32.5, "srp_weapon_fov_scope": 32.5,
                "srp_sight_distance": 0.0}
    report(stopped == CONFIGS_START_END and all(name in values and r.floats(values[name], 1)[0] == value
                                                for name, value in defaults.items())
           and "srp_weapon_fov_world" in values and r.byte(values["srp_weapon_fov_world"]) == 0,
           "configs.start: srp_weapon_fov 65, srp_weapon_fov_aimed and srp_weapon_fov_scope 32.5, srp_sight_distance 0, "
           "srp_weapon_fov_world off")
    stock = Run(original)
    _, none = stock.settings()
    report(not any(name.startswith(("srp_weapon_fov", "srp_sight")) for name in none), "the original registers none")

    # the values as user.cfg's lines and the console set them: each variable's own setter
    r.emu.hook_add(UC_HOOK_CODE, r.sscanf, begin=0x422B46, end=0x422B46)

    def put(name, value):
        r.console(name, ("on" if value else "off") if name == "srp_weapon_fov_world" else f"{value}")
        got = r.byte(values[name]) if name == "srp_weapon_fov_world" else r.floats(values[name], 1)[0]
        assert got == value, (name, value, got)

    player, world, entry, config, view = (r.block(n) for n in (0xF480, 0x10, 0x118, 0x10C, 0x100))
    r.u32(player + 0xF474, world)
    r.u32(world + 8, view)
    pair = r.block(8)
    r.put_floats(pair, (1.0, 0.5))
    offset_026g = r.node(2, struct.unpack("<I", struct.pack("<f", 0.15))[0])
    rows = [{"gun": r.node(T_INT, 220), "fov": r.node(6, pair, 8), "aim_offset_026g": offset_026g}]
    array = r.block(0x18 * len(rows))
    for n, entries in enumerate(rows):
        at = array + 0x18 * n
        r.u32(at, r.block(0x18 * len(entries)))
        r.emu.mem_write(at + 0x14, struct.pack("<HH", T_TABLE, len(entries)))
        for name, child in entries.items():
            r.children[(at, name.encode())] = child
    r.u32(config + 0x108, r.table({"srp_weapon_views": r.node(T_TABLE, array, len(rows))}))
    r.dictionary_configs(5, None, config)
    report(b"weapon views" not in r.logged, "a row with aim_offset_026g is taken")

    def gun(to):
        weapon = r.block(0x1900)
        r.u32(weapon + 0x26C, entry)
        r.u32(entry, 220)
        r.put_floats(weapon + 0x6A0, translate(0.0, 0.0, to))
        return weapon

    world_projection = [1.125, 0, 0, 0, 0, 2, 0, 0, 0.001, -0.002, 1.01, 1, 0, 0, -0.1, 0]

    def drawn(weapon=None, progress=1.0):
        if weapon is None:
            r.run(VIEW_NO_GUN, VIEW_ENDS, STACK, ebx=player)
        else:
            r.put_floats(weapon + 0x5FC, [progress])
            r.run(VIEW_CAMERA, VIEW_ENDS, STACK, eax=weapon, ebx=player)
        command = r.block(0xB8)
        r.u32(command + 0x9C, view)
        r.run(VIEW_SUBMIT, VIEW_ENDS, STACK, eax=command, ebx=RENDERER)
        r.run(VIEW_BEGIN, VIEW_ENDS, STACK, esi=command + 0x98)
        r.put_floats(STACK + 0x28, world_projection)
        r.run(FOREGROUND, VIEW_ENDS, STACK)
        got = r.floats(STACK + 0x28)
        r.run(VIEW_END, VIEW_ENDS, STACK)
        return got

    def lens(angle):
        out = world_projection.copy()
        if angle:
            out[5] = 1.0 / math.tan(math.radians(angle) / 2)
            out[0] = world_projection[0] / world_projection[5] * out[5]
        return out

    a545 = gun(0.137)
    put("srp_weapon_fov", 90.0)
    put("srp_weapon_fov_aimed", 52.0)
    for weapon, progress, angle, what in ((a545, 1.0, 52.0, "aimed over its iron sights"),
                                          (a545, 0.0, 90.0, "at the hip"), (None, 0.0, 90.0, "no gun")):
        report(close(drawn(weapon, progress), lens(angle)), f"srp_weapon_fov 90, srp_weapon_fov_aimed 52: {what}, "
                                                            f"{angle:g} degrees")
    put("srp_weapon_fov_world", 1)
    report(close(drawn(a545), lens(None)) and close(drawn(None), lens(None)),
           "srp_weapon_fov_world on: the world's projection, a gun's and no gun's")
    put("srp_weapon_fov_world", 0)
    put("srp_weapon_fov", 65.0)
    put("srp_weapon_fov_aimed", 32.5)
    report(close(drawn(a545), lens(32.5)), "back at 65 and 32.5: 0.69d0's 32.5 degrees aimed")
    said = r.logged.decode(errors="replace")
    report("weapon view: srp_weapon_fov 90000, srp_weapon_fov_aimed 52000" in said
           and "everything held at the world's projection" in said
           and "weapon view: gun 220 fully aimed through its iron sights: drawn at 520 tenths of a degree" in said
           and "weapon view: gun 220 at the hip through its iron sights: drawn at 900 tenths of a degree" in said
           and "weapon view: the foreground draws what is held at its own projection" in said,
           "the log says the values when they change (the setters' lines), and what the held gun is drawn at, "
           "at the hip and fully aimed" + ("" if "weapon view: srp" in said else f": {said!r}"))

    # the sight distance: the A-545's iron sights, 0.137 (0.69d0's eye) toward 0.15 (revision 73's)
    for percent, want_offset in ((0.0, 0.137), (50.0, 0.1435), (100.0, 0.15)):
        put("srp_sight_distance", percent)
        drawn(a545)
        stopped, ours = align(r, a545, 0.137)
        stock = Run(original)
        stock_stopped, theirs = align(stock, stock.block(0x1900), want_offset)
        exact = ours == theirs if percent == 0.0 else close(ours, theirs, 2e-6)
        report(stopped == stock_stopped == ALIGNED and exact,
               f"srp_sight_distance {percent:g}: the alignment the original makes with aim_offset {want_offset:g}"
               + (" bit for bit" if percent == 0.0 else "") + (" (revision 73's)" if percent == 100.0 else ""))
    other = gun(0.137)
    stopped, ours = align(r, other, 0.137)
    stock = Run(original)
    _, theirs = align(stock, stock.block(0x1900), 0.137)
    report(stopped == ALIGNED and ours == theirs, "a weapon the camera did not sample: its alignment the original's")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None, late_join)
    built = bytes(built)
    table = (SAMPLE / "sample-v4.srpm").read_bytes()
    cases = loadouts()
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    for data, patched in [(original, False), (built, True)]:
        weapon_projection(data, patched, report)
    weapon_view_settings(original, built, report)

    def with_table(data=built):
        run = Run(data)
        run.dictionary_configs(6, table)
        return run

    AK, DTK3, MIKI, PSO1, EOTECH, HK416, DPPRO = 3, 1415, 1438, 1755, 1771, 763, 2112
    NATIVE = [("weapons/ak74m/ak74m", 0x10), ("weapons/ak_74/preview", 0x31), ("shells/5_45x040", 0x3D),
              ("fx/weapons/ak_74", 0x3D), ("fx/weapons/ak_74", 0x3D)]
    GAME_RECORDS = [(1, translate(0.0, 0.042, -0.636)), (1, translate(0.0, 0.07, 0.02)), (0, translate(0.0, 0.08, 0.1))]

    def named(gun, scope=0, muzzle=0, grip=0, second=0, side=0):
        config = {"srp_gun": gun, "srp_scope": scope, "srp_muzzle": muzzle, "srp_grip": grip}
        if second:
            config.update({"srp_second_sight": second, "srp_second_side": side})
        return config

    print("the table's way in (the items dictionary cook's query 0x2C8963, its callback 0x2C8AA0)")
    a, b = Run(original).dictionary_query(), Run(built).dictionary_query()
    report(a is not None and len(a) == 5 and all(kind == 0x1F for _, kind in a),
           f"the original: five binary configs ({a})")
    report(b is not None and a is not None and b == a + [(TABLE_PATH, 3)],
           f"the build: the same five, then {TABLE_PATH.decode()} as raw data, class 3")
    run = Run(built)
    stopped, counts = run.dictionary_configs(6, table)
    report(stopped == DICT_CONFIGS_END and counts == (1, 0) and b"the modules table, bytes " in run.logged,
           f"its sixth result read, kept and said ({run.logged.decode(errors='replace').strip()!r}); the resource "
           f"unpinned, its reference given back (count, pins: {counts})")
    for name, count, bytes_, line in (
            ("a query of five results", 5, table, b""),
            ("no such file (the sixth result 0)", 6, None, b""),
            ("another version's table", 6, table[:4] + b"\x09\x00" + table[6:], b"of another version"),
            ("bytes that are no table", 6, b"SRPX" + table[4:], b"is not one")):
        run = Run(built)
        stopped, counts = run.dictionary_configs(count, bytes_)
        got = run.cook(run.config(named(AK, muzzle=DTK3))[0], run.block(0x4800))
        report(stopped == DICT_CONFIGS_END and counts in (None, (1, 0)) and got is not None and got[0] == NATIVE
               and (line in run.logged if line else run.logged == b""),
               f"{name}: no table (a fitted weapon's cook is the game's own), "
               f"{'a line' if line else 'nothing said'}")

    print("the cook's requests (on_weapon_config_loaded 0x24A920: 0x24AAB4, 0x24AC30)")
    plain = Run(original)
    native = plain.cook(plain.config(named(AK, muzzle=DTK3))[0], plain.block(0x4800))
    report(native is not None and native[0] == NATIVE and native[1], f"the original: {[kind for _, kind in NATIVE]}")
    for name, data, key in (
            ("no data table", None, None),
            ("a data table naming no gun", {"camo_scale": 4}, None),
            ("a gun with nothing fitted", named(AK), (AK, 0, 0, 0)),
            ("a gun the table lacks", named(999, muzzle=DTK3), None),
            ("a fit the gun refuses", named(HK416, muzzle=DTK3), (HK416, 0, DTK3, 0)),
            ("a brake", named(AK, muzzle=DTK3), (AK, 0, DTK3, 0)),
            ("a grip, with its forend", named(AK, grip=MIKI), (AK, 0, 0, MIKI)),
            ("a scope on the dovetail", named(AK, scope=PSO1), (AK, PSO1, 0, 0)),
            ("a scope with its rail", named(AK, scope=EOTECH), (AK, EOTECH, 0, 0)),
            ("all three", named(AK, PSO1, DTK3, MIKI), (AK, PSO1, DTK3, MIKI)),
            ("a second sight on the right, with its offset rail", named(HK416, second=DPPRO, side=1),
             (HK416, 0, 0, 0, DPPRO, 1)),
            ("a second sight on the right beside a scope and a grip",
             named(HK416, scope=EOTECH, grip=MIKI, second=DPPRO, side=1), (HK416, EOTECH, 0, MIKI, DPPRO, 1)),
            ("a second sight on a gun without angled rails", named(1220, second=DPPRO, side=1),
             (1220, 0, 0, 0, DPPRO, 1))):
        run = with_table()
        got = run.cook(run.config(data)[0], run.block(0x4800))
        case = cases.get(key) if key else None
        if not isinstance(case, dict):
            ok = got is not None and native is not None and got == native
            report(ok, f"{name}: the original's requests and alloca")
            continue
        scope = [(SCOPE_ITEMS[key[1]], 0x4D)] if key[1] in SCOPE_ITEMS else []
        aimed = [(case["aimed"], 15)] if "aimed" in case else []
        want = scope + [(case["body"], 0x10)] + NATIVE[1:] + [(model, 15) for model, _, _ in case["parts"]] + aimed
        ok = got is not None and native is not None and got[0] == want and got[1] \
            and got[2] == native[2] + 8 * (len(case["parts"]) + len(scope) + len(aimed))
        report(ok, f"{name}: {'the scope item' + chr(39) + 's config first, class 4Dh; ' if scope else ''}the bare body "
                   f"for the model, then {len(case['parts'])} part(s), class 15, "
                   f"{'the second sight' + chr(39) + 's aimed model after them, ' if aimed else ''}inside the alloca "
                   f"({[path.rsplit('/', 1)[-1] for path, _ in (got[0][len(scope) + len(NATIVE):] if got else [])]})")

    print("the arena an upgraded item's config is rebuilt in (create_binary_config_buffer 0x358AA0)")
    for build, exe, want in (("the build", built, base.ITEM_CONFIG_ARENA), ("the original", original, 0x4000)):
        taken, given = (struct.unpack_from("<I", exe, base.file_offset(va + 1))[0] for va in (0x358AA8, 0x358ABC))
        report(taken == given == want, f"{build}: {taken:#x} bytes taken from the stack, {given:#x} given to the allocator")

    print("the scope's dict id (the callback's question 0x24AD13; the cook's is in the requests above)")
    for name, data, scoped in (
            ("a scope named by data/srp_scope, no addons", named(AK, scope=PSO1), True),
            ("no scope named", named(AK, muzzle=DTK3), False),
            ("a scope the dictionary lacks", named(AK, scope=999), False),
            ("no data table", None, False)):
        for build, exe in (("the build", built), ("the original", original)):
            run = Run(exe)
            stopped, cook_kept, taken = run.scope_result(run.config(data)[1])
            want = SCOPE_RESULT_SCOPED if scoped and exe is built else SCOPE_RESULT_END
            ok = stopped == want and cook_kept and (taken == 0 if stopped == SCOPE_RESULT_SCOPED else True)
            report(ok, f"{name}, {build}: {'the first result is the scope' + chr(39) + 's (0x24AD47), the cook kept, the out slot 0' if want == SCOPE_RESULT_SCOPED else 'the game' + chr(39) + 's own checks'}")

    WORLD = multiply(rotation_y(0.4), translate(10.0, 1.5, -4.0))
    BONES = [IDENTITY, multiply(rotation_y(-0.2), translate(0.0, 0.01, -0.03)), translate(0.3, 0.2, 0.1),
             translate(0.0, 0.0, 0.0)]

    def fitted(key, bones=4, eye="optic"):
        """A weapon cooked with `key` fitted, through the callback: the run,
        the weapon, its model's instance, the parts' holders and instances,
        and the case."""
        case = cases[key]
        run = with_table()
        weapon, instance = run.weapon(bones, GAME_RECORDS)
        resource, _ = run.config(named(*key))
        if run.cook(resource, weapon) is None:
            return None
        parts = [run.part() for _ in case["parts"]]
        run.aimed = [run.part() for _ in ("aimed",) if "aimed" in case]
        scope = [run.block(0x120)] if key[1] in SCOPE_ITEMS else []
        stopped = run.callback(weapon, resource, scope + [run.block(0x120) for _ in NATIVE] + [h for h, _ in parts]
                               + [h for h, _ in run.aimed])
        return (run, weapon, instance, parts, case) if stopped == LOADED_END else None

    print("the callback (0x24ACF0, 0x24B0E1): the parts' models, the three records, the fingers")
    for name, key in (("a brake", (AK, 0, DTK3, 0)), ("a grip", (AK, 0, 0, MIKI)), ("a scope", (AK, PSO1, 0, 0)),
                      ("a scope with its rail", (AK, EOTECH, 0, 0)), ("all three", (AK, PSO1, DTK3, MIKI)),
                      ("a second sight on the right", (HK416, 0, 0, 0, DPPRO, 1))):
        made = fitted(key)
        if made is None:
            report(False, f"{name}: the callback did not reach its sites")
            continue
        run, weapon, _, parts, case = made
        held = all(run.u32(holder + 0xD0) == 2 for holder, _ in parts + run.aimed)
        records = {at: run.read_record(weapon + at) for at in (BARREL, SCOPE, AIM)}
        want = {BARREL: case.get("fire", GAME_RECORDS[0]), SCOPE: case.get("optic", GAME_RECORDS[1]),
                AIM: case["eye"].get("gun_idle", GAME_RECORDS[2])}
        placed = all(records[at][0] == want[at][0] and close(records[at][1], want[at][1]) for at in want)
        sets = [run.floats(weapon + FINGER_SETS + 0x3C0 * n, 240) for n in range(4)]
        own = [v for m in own_fingers() for v in m]
        grip = [v for m in case["fingers"] for v in m] if case["fingers"] else own
        fingers = close(sets[0], grip) and close(sets[3], grip) and close(sets[1], own) and close(sets[2], own)
        report(held and placed and fingers,
               f"{name}: {len(parts)} model(s) retained once; the barrel, scope and aim records "
               f"{'the loadout' + chr(39) + 's' if placed else records}; the idle and lobby finger sets "
               f"{'the grip' + chr(39) + 's own' if case['fingers'] else 'the game' + chr(39) + 's'}"
               f", reload's two the game's")
    made = fitted((AK, PSO1, DTK3, MIKI), bones=3)
    if made is None:
        report(False, "a model of three bones for the table's four: the callback did not reach its sites")
    else:
        run, weapon, _, parts, _ = made
        records = [run.read_record(weapon + at) for at in (BARREL, SCOPE, AIM)]
        report(all(run.u32(holder + 0xD0) == 1 for holder, _ in parts)
               and all(got[0] == want[0] and close(got[1], want[1]) for got, want in zip(records, GAME_RECORDS))
               and b"fewer bones" in run.logged,
               "a model of three bones for the table's four: the records the game's, the models given back, a line")
    made = fitted((AK, PSO1, 0, 0))
    if made is None:
        report(False, "the PSO-1 on the AK-74M: the callback did not reach its sites")
    else:
        fitted_optic_projection(made[0], made[1], report)
        _, values = made[0].settings()
        made[0].put_floats(values["srp_weapon_fov_scope"], [20.0])
        made[0].put_floats(values["srp_weapon_fov_aimed"], [50.0])
        fitted_optic_projection(made[0], made[1], report, scope=20.0)
    run = with_table()
    weapon, _ = run.weapon(4, GAME_RECORDS)
    resource, _ = run.config(named(AK))
    before = bytes(run.emu.mem_read(weapon, 0x4800))
    ok = run.cook(resource, weapon) is not None \
        and run.callback(weapon, resource, [run.block(0x120) for _ in NATIVE]) == LOADED_END
    report(ok and bytes(run.emu.mem_read(weapon, 0x4800)) == before, "nothing fitted: the weapon as the game made it")

    print("the parts in the scene (weapon::draw 0x245017, culled 0x245353, hidden 0x244017)")
    made = fitted((AK, PSO1, DTK3, MIKI))
    if made is None:
        report(False, "a weapon with all three: the callback did not reach its sites")
    else:
        run, weapon, instance, parts, case = made
        run.emu.mem_write(instance + 0x114, b"\1\0")
        stopped = run.draw(weapon, WORLD, BONES)
        frames = [frame_of(offset, BONES[bone], WORLD) for _, bone, offset in case["parts"]]
        own = [run.calculate(bone, offset, WORLD, BONES) for _, bone, offset in case["parts"]]
        added = [(op, inst) for op, inst, _ in run.rendered] == [("add", inst) for _, inst in parts]
        report(stopped == DRAW_END and added and all(close(at, want) for (_, _, at), want in zip(run.rendered, frames))
               and all(close(a, b) for a, b in zip(frames, own)),
               f"the first draw: each of {len(parts)} parts added at its record's frame, the game's own "
               f"calculate_locator's")
        report(all(bytes(run.emu.mem_read(inst + 0x114, 2)) == b"\1\0" for _, inst in parts),
               "drawn as the weapon's model is (the first-person pass, not movable: its two bytes)")
        run.draw(weapon, WORLD, BONES)
        report([op for op, _, _ in run.rendered] == ["update"] * len(parts), "the next draw: each updated")
        run.emu.mem_write(instance + 0x10D, b"\0")
        run.draw(weapon, WORLD, BONES)
        report([op for op, _, _ in run.rendered] == ["remove"] * len(parts),
               "the weapon's model out of the scene (hidden while aiming): each removed")
        run.draw(weapon, WORLD, BONES)
        report(run.rendered == [], "and nothing more while it is out")
        for name, start, end, register in (("culled", CULLED, CULLED_END, "ebx"), ("hidden", HIDDEN, HIDDEN_END, "esi")):
            run.emu.mem_write(instance + 0x10D, b"\1")
            run.draw(weapon, WORLD, BONES)
            run.rendered = []
            stopped = run.run(start, (end,), STACK - 0x5000, **{register: weapon})
            report(stopped == end and [op for op, _, _ in run.rendered] == ["remove"] * len(parts),
                   f"{name}: each removed")

        print("the left hand, the shells (0x244649, 0x298664, 0x2443EE)")
        stopped = run.run(HANDS, (HANDS_END,), STACK - 0x5000, esi=weapon)
        hands = [run.read_record(weapon + HAND_RECORDS + 0x64 * n) for n in range(8)]
        game = [(2, translate(0.5 + n, 0.25, 0.125)) for n in range(8)]
        wrist = case["wrist"]
        report(stopped == HANDS_END
               and all(hands[n][0] == wrist[0] and close(hands[n][1], wrist[1]) for n in (0, 3, 4, 7))
               and all(hands[n][0] == game[n][0] and close(hands[n][1], game[n][1]) for n in (1, 2, 5, 6)),
               "weapon::activate: the idle and lobby records of both views the grip's hand, the reload's the game's")
        corrector = weapon + FINGER_SETS
        sp = STACK - 0x5000
        for i, value in enumerate((corrector, weapon + 0x1810, 1)):
            run.u32(sp + 4 * i, value)
        stopped = run.run(FINGERS_VIEW, (FINGERS_VIEW_END,), sp)
        sets = [run.floats(corrector + 0x3C0 * n, 240) for n in range(4)]
        own = [v for m in own_fingers() for v in m]
        grip = [v for m in case["fingers"] for v in m]
        report(stopped == FINGERS_VIEW_END and run.fingers_filled == 1 and close(sets[0], grip)
               and close(sets[3], grip) and close(sets[1], own) and close(sets[2], own)
               and run.emu.reg_read(UC_X86_REG_ESP) == sp + 12,
               "a change of view: the game refills the sets once, the idle and lobby ones the grip's again")
        # the scope's frame of a draw: what calculate_locator gives the written record
        now = frame_of(case["optic"][1], BONES[case["optic"][0]], WORLD)
        was = frame_of(GAME_RECORDS[1][1], BONES[GAME_RECORDS[1][0]], WORLD)
        run.put_floats(weapon + FRAME, now)
        stopped = run.run(SHELLS, (SHELLS_END,), STACK - 0x5000, esi=weapon, eax=0)
        at = run.emu.reg_read(UC_X86_REG_EAX)
        report(stopped == SHELLS_END and at != weapon + FRAME and close(run.floats(at), was, 1e-4),
               "the shell effect: handed the frame the gun's own scope_point has, not the optic's")

        print("the weapon goes (weapon_cook::delete_resource 0x24B190)")
        run.emu.mem_write(instance + 0x10D, b"\1")
        run.draw(weapon, WORLD, BONES)
        run.rendered, run.destroyed = [], []
        sp = STACK - 0x5000
        run.u32(sp, STOP)
        run.u32(sp + 4, weapon + 0x10)
        stopped = run.run(RELEASE, (RELEASE_END,), sp)
        report(stopped == RELEASE_END and [op for op, _, _ in run.rendered] == ["remove"] * len(parts)
               and all(run.u32(holder + 0xD0) == 1 for holder, _ in parts) and run.destroyed == []
               and len(run.freed) == 1,
               "each part out of the scene, its reference given back once, the parts' memory freed")
        run.draw(weapon, WORLD, BONES)
        report(run.rendered == [], "and a weapon at that address draws nothing of them")

    print("a weapon with nothing fitted, and the original")
    for name, data in (("the build", built), ("the original", original)):
        run = with_table(data) if data is built else Run(data)
        weapon, instance = run.weapon(4, GAME_RECORDS)
        before = bytes(run.emu.mem_read(weapon, 0x4800))
        ok = run.draw(weapon, WORLD, BONES) == DRAW_END and run.rendered == []
        ok = ok and run.run(HANDS, (HANDS_END,), STACK - 0x5000, esi=weapon) == HANDS_END
        ok = ok and run.run(SHELLS, (SHELLS_END,), STACK - 0x5000, esi=weapon, eax=0) == SHELLS_END \
            and run.emu.reg_read(UC_X86_REG_EAX) == weapon + FRAME
        report(ok and bytes(run.emu.mem_read(weapon, 0x4800)) == before,
               f"{name}: draw, activate and the shell effect leave the weapon as it is, nothing in the scene")

    print("FAILED" if failed else "all passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()

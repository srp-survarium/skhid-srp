"""Tests of harness.py on made-up input (no game, Wine, display or server):
python3 -m unittest discover -s srp-026g/scripts/port-regression -p 'test_*.py'"""
import json
from pathlib import Path
import tempfile
import unittest

import harness as h


def proc_entry(root: Path, pid: int, argv: list[str], env: dict[str, str], start: str = "100", state: str = "S"):
    d = root / str(pid)
    d.mkdir(parents=True)
    (d / "cmdline").write_bytes(b"\0".join(a.encode() for a in argv) + b"\0")
    (d / "environ").write_bytes(b"\0".join(f"{k}={v}".encode() for k, v in env.items()) + b"\0")
    fields = [state] + ["0"] * 18 + [start] + ["0"] * 10
    (d / "stat").write_text(f"{pid} (a b) " + " ".join(fields))


def datagram(stamp: str, direction: str, payload: bytes, messages: list[tuple[str, int, int]]) -> str:
    lines = [f"{stamp} {direction} 127.0.0.1:5000 len={len(payload)} " + " ".join(f"{b:02X}" for b in payload),
             "    seq=1 rseq=0 acks=1/7 " + ("multi" if len(messages) > 1 else "single")]
    lines += [f"    msg id={mid} len={length} at={at}" for mid, length, at in messages]
    return "\n".join(lines)


def df(stamp: str, tick: int, crc: int) -> str:
    body = bytes([tick]) + crc.to_bytes(4, "little")
    return datagram(stamp, "IN", bytes([0x21, 0x3F, 0x7E, 0xDF]) + body, [("DF", 5, 4)])


class GameCheck(unittest.TestCase):
    def test_what_is_a_game(self):
        self.assertTrue(h.is_game_command(["survarium_skhid.exe", "-server=1"]))
        self.assertTrue(h.is_game_command(["Z:\\g\\binaries\\x86\\Survarium_New.exe"]))
        self.assertTrue(h.is_game_command(["/nix/store/x/bin/wine", "survarium_old.exe", "-match_replay=a"]))
        self.assertTrue(h.is_game_command(["/nix/store/x/bin/.wine-wrapped", "survarium.exe"]))
        # This harness's own command lines name exes in their arguments: not games.
        self.assertFalse(h.is_game_command(["bash", "run.sh", "--new-exe", "/x/survarium_new.exe"]))
        self.assertFalse(h.is_game_command(["python3", "harness.py", "has-frames", "survarium.exe"]))
        self.assertFalse(h.is_game_command(["/nix/store/x/bin/wine", "explorer.exe"]))
        self.assertFalse(h.is_game_command([]))

    def test_owner_games_ignores_ours_and_finds_the_rest(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "proc"
            ours = f"{tmp}/work"
            proc_entry(root, 10, ["survarium_new.exe"], {"WINEPREFIX": f"{ours}/prefixes/new"})
            proc_entry(root, 11, ["bash", "run.sh", "--old-exe", "survarium.exe"], {})
            self.assertEqual(h.owner_games(root, ours, []), [])
            proc_entry(root, 12, ["survarium_skhid.exe"], {"WINEPREFIX": "/home/x/runtime/client-1"})
            reasons = h.owner_games(root, ours, [])
            self.assertEqual(len(reasons), 1)
            self.assertIn("pid 12", reasons[0])
            # A prefix that merely starts with our folder's name is not ours.
            proc_entry(root, 13, ["survarium_skhid.exe"], {"WINEPREFIX": f"{ours}-other/p"})
            self.assertEqual(len(h.owner_games(root, ours, [])), 2)

    def test_a_recorded_local_clients_run_is_busy_only_while_alive(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "proc"
            proc_entry(root, 20, ["srp"], {}, start="555")
            state = Path(tmp) / "state.json"
            state.write_text(json.dumps({"id": "x", "clients": {}, "server": {"pid": 20, "started": "555"}}))
            self.assertEqual(len(h.owner_games(root, None, [state])), 1)
            state.write_text(json.dumps({"id": "x", "clients": {}, "server": {"pid": 20, "started": "999"}}))
            self.assertEqual(h.owner_games(root, None, [state]), [])  # the pid was reused
            proc_entry(root, 21, ["wine"], {}, start="7", state="Z")
            state.write_text(json.dumps({"id": "x", "clients": {"0": {"pid": 21, "started": "7"}}}))
            self.assertEqual(h.owner_games(root, None, [state]), [])  # a zombie is gone
            self.assertEqual(h.owner_games(root, None, [Path(tmp) / "missing.json"]), [])


class Hashes(unittest.TestCase):
    def test_reports_are_read_from_single_and_multi_datagrams(self):
        multi = datagram("2026-10-26T12:29:51.108Z", "IN",
                         bytes.fromhex("203F7FDB020010131605DF0F357DEF45"), [("DB", 6, 4), ("DF", 5, 11)])
        out = datagram("2026-10-26T12:29:51.110Z", "OUT", bytes.fromhex("4121FFDF0F357DEF45"), [("DF", 5, 4)])
        text = "\n".join([multi, df("2026-10-26T12:29:51.109Z", 0x10, 0xD063C64E), out])
        self.assertEqual(h.packet_hash_reports(text), [
            ("2026-10-26T12:29:51.108Z", 0x0F, 0x45EF7D35),
            ("2026-10-26T12:29:51.109Z", 0x10, 0xD063C64E),
        ])

    def series(self, start_s: float, n: int, crc=lambda i: i * 7919):
        """n reports, one a tick (50 ms), the tick mod 256."""
        rows = []
        for i in range(n):
            t = start_s + i * 0.05
            stamp = f"2026-10-26T12:{int(t // 60):02d}:{t % 60:06.3f}Z"
            rows.append((stamp, i % 256, crc(i)))
        return rows

    def test_two_clients_that_agree(self):
        a, b = self.series(0, 600), self.series(0.02, 600)  # 30 s: the tick wraps twice
        result = h.pair_reports(a, b)
        self.assertEqual((result["paired"], result["differ"]), (600, 0))

    def test_a_difference_is_found_after_the_wrap(self):
        a = self.series(0, 600)
        b = self.series(0.03, 600, crc=lambda i: i * 7919 + (1 if i == 300 else 0))
        result = h.pair_reports(a, b)
        self.assertEqual(result["differ"], 1)
        self.assertIn("tick 44", result["first"])  # 300 mod 256

    def test_a_resent_report_counts_once_and_late_reports_do_not_pair_with_the_next_wrap(self):
        a = self.series(0, 10)
        a.insert(3, a[2])
        b = self.series(12.8, 10)  # the same tick numbers one wrap later, other CRCs
        self.assertEqual(h.pair_reports(a, self.series(0, 10))["paired"], 10)
        self.assertEqual(h.pair_reports(a, b, window=6.0)["paired"], 0)

    def test_hash_files_tick_by_tick(self):
        old = h.read_hash_csv("# srp\ntime_ms,crc\n50,0a0b0c0d\n100,1\n150,ff\n")
        self.assertEqual(old, [(50, 0x0A0B0C0D), (100, 1), (150, 0xFF)])
        same = h.compare_hashes(old, list(old))
        self.assertEqual((same["common"], same["differ"], same["first"]), (3, 0, None))
        changed = h.compare_hashes(old, [(50, 0x0A0B0C0D), (100, 2)])
        self.assertEqual((changed["differ"], changed["only_old"]), (1, 1))
        self.assertIn("tick 100", changed["first"])


class Frames(unittest.TestCase):
    HEADER = ("# srp_frames 1; revision 34; started 2026-10-25 18:08:04.198 local\n"
              "frame,t,frame_us,state,flags,alive,logic_n\n")

    def csv(self, rows):
        return self.HEADER + "\n".join(f"{i},{i / 100:.6f},{us},{state},,1," for i, (us, state) in enumerate(rows))

    def test_only_the_states_asked(self):
        text = self.csv([(16000, 1), (17000, 6), (40000, 5), (5000, 2)])
        self.assertEqual(h.frame_times(text, {6}), [17000])
        self.assertEqual(h.frame_times(text, {5, 6}), [17000, 40000])
        self.assertEqual(h.frame_times("", {6}), [])

    def test_percentiles(self):
        s = h.frame_stats(list(range(1, 101)))
        self.assertEqual((s["frames"], s["p50"], s["p95"], s["p99"], s["max"]), (100, 50, 95, 99, 100))

    def test_a_regression_beyond_the_threshold_fails(self):
        old = h.frame_stats([16000] * 900 + [30000] * 100)
        same = h.frame_stats([16200] * 900 + [30500] * 100)
        worse = h.frame_stats([16000] * 800 + [45000] * 200)
        self.assertEqual(h.compare_frames(old, same, 0.10, 1.0)[0], "PASS")
        status, detail = h.compare_frames(old, worse, 0.10, 1.0)
        self.assertEqual(status, "FAIL")
        self.assertIn("p95", detail)
        self.assertEqual(h.compare_frames(old, h.frame_stats([1] * 10), 0.1, 1.0)[0], "SKIPPED")


class Logs(unittest.TestCase):
    def test_error_kinds_drop_times_and_numbers(self):
        old = ("game_core <ERROR>\t[13:34:29.113]\tWrong map config. Map id = 114\n"
               "game <info>\t[13:34:34.822]\tsrp: lobby character's player made in 517 ms\n")
        new = old.replace("114", "143") + "game <ERROR>\t[13:35:00.000]\tsomething new 0x1F3D90\n"
        self.assertEqual(h.error_kinds(old), {"game_core ERROR: Wrong map config. Map id = #"})
        self.assertEqual(h.error_kinds(new) - h.error_kinds(old), {"game ERROR: something new #"})

    def test_server_log_counts(self):
        text = "\n".join([
            "18:34:17.223Z [match] [INFO] Match 0x3fe3c512: player 0 gave the initial world (2377 bytes at 50 ms, CRC 3b38b20a)",
            "18:35:00.000Z [match] [WARN] Match 0x3fe3c512: player 1 is out of sync (tick 10); resyncing it",
            "18:35:01.000Z [match] [WARN] Match 0x3fe3c512: the clients disagree at 5000 ms, no majority: [1, 2]",
            "21:11:52.077Z [match] [INFO] Match 0x5b4a6230: over at 47452 ms (TimeLimit); winner Some(2)",
        ])
        c = h.scan_server(text)
        self.assertEqual((c["matches_started"], c["out_of_sync"], c["disagree"], c["matches_over"], c["panic"]),
                         (1, 1, 1, 1, 0))

    def test_client_scan(self):
        r = h.scan_client("0024:err:seh:x\nwine: Unhandled page fault on read access\n", "ok\n", 1)
        self.assertEqual((r["wine_crash_lines"], r["crash_reports"]), (2, 1))
        self.assertEqual(h.scan_client("", "Replay of other engine version is used\n", 0)["fatal_lines"], 1)


class Setup(unittest.TestCase):
    def test_server_env_wins_over_the_shell_and_the_config(self):
        env = h.server_env({"PATH": "/bin", "SRP_DB": "off", "SRP_X": "shell"},
                           {"SRP_GAME_DATA": "/g.db", "SRP_DATA_DIR": "/live", "SRP_ADMIN_TOKEN": "t"},
                           Path("/w"), [1, 2, 3, 4], {"SRP_MATCH_MAPS": "61", "SRP_EMPTY": ""})
        self.assertEqual(env["PATH"], "/bin")
        self.assertNotIn("SRP_X", env)
        self.assertNotIn("SRP_ADMIN_TOKEN", env)
        self.assertNotIn("SRP_EMPTY", env)
        self.assertEqual((env["SRP_DB"], env["SRP_DATA_DIR"], env["SRP_GAME_DATA"]), ("on", "/w/data", "/g.db"))
        self.assertEqual((env["SRP_LOGIN_PORT"], env["SRP_WEB_PORT"], env["SRP_MATCH_MAPS"]), ("1", "4", "61"))
        self.assertEqual(env["SRP_PACKET_LOG"], "/w/packets")

    def test_user_cfg(self):
        lines = h.user_cfg(["r_fullscreen on", "account_digest abc", "snd_volume 1"], "1280x720")
        self.assertEqual(lines, ["snd_volume 1", "store_user_password on", "r_fullscreen off", "r_resolution 1280x720"])

    def test_game_farm_links_the_game_and_copies_the_exes(self):
        with tempfile.TemporaryDirectory() as tmp:
            game, out = Path(tmp) / "game", Path(tmp) / "farm"
            (game / "binaries/x86").mkdir(parents=True)
            (game / "resources.db").write_text("db")
            (game / "binaries/x86/dxgi.dll").write_text("dll")
            (game / "binaries/x86/survarium_skhid.exe").write_text("theirs")
            exe = Path(tmp) / "new.exe"
            exe.write_text("ours")
            h.game_farm(game, out, {"new": exe})
            self.assertTrue((out / "resources.db").is_symlink())
            self.assertEqual((out / "binaries/x86/survarium_new.exe").read_text(), "ours")
            self.assertTrue((out / "binaries/x86/dxgi.dll").is_file())
            self.assertFalse((out / "binaries/x86/survarium_skhid.exe").exists())

    def test_prefix_from_a_template(self):
        with tempfile.TemporaryDirectory() as tmp:
            template, prefix = Path(tmp) / "t", Path(tmp) / "p"
            (template / "drive_c/users/sheep").mkdir(parents=True)
            (template / "system.reg").write_text("")
            h.prepare_prefix(template, prefix, "1280x720")
            cfg = prefix / "drive_c/users/sheep/Documents/Survarium/user.cfg"
            self.assertIn("r_resolution 1280x720", cfg.read_text())
            (cfg.parent / "Survarium_sheep.log").write_text("x")
            h.clear_documents(prefix)
            self.assertFalse((cfg.parent / "Survarium_sheep.log").exists())
            self.assertTrue(cfg.exists())
            with self.assertRaises(SystemExit):
                h.prepare_prefix(template, template, "1280x720")  # not one of ours


class CommandLine(unittest.TestCase):
    """The subcommands as run.sh calls them."""

    def test_compare_df_on_two_packet_logs(self):
        with tempfile.TemporaryDirectory() as tmp:
            packets = Path(tmp)
            a = [df(f"2026-10-26T12:30:{i * 0.05:06.3f}Z", i, 1000 + i) for i in range(100)]
            b = [df(f"2026-10-26T12:30:{i * 0.05 + 0.01:06.3f}Z", i, 1000 + i) for i in range(100)]
            (packets / "match-127.0.0.1_1-1.log").write_text("\n".join(a))
            (packets / "match-127.0.0.1_2-2.log").write_text("\n".join(b))
            (packets / "match-127.0.0.1_3-3.log").write_text("")  # srp-bot: no hashes
            (packets / "lobby-127.0.0.1_1-1.log").write_text("\n".join(a))  # not a match log
            self.assertEqual(h.main(["compare-df", str(packets)]), 0)
            b[50] = df("2026-10-26T12:30:02.510Z", 50, 7)
            (packets / "match-127.0.0.1_2-2.log").write_text("\n".join(b))
            self.assertEqual(h.main(["compare-df", str(packets)]), 1)
            (packets / "match-127.0.0.1_2-2.log").write_text("")
            self.assertEqual(h.main(["compare-df", str(packets)]), 2)  # one client: nothing to compare

    def test_frames_compare_and_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            old, new, results = Path(tmp) / "old.csv", Path(tmp) / "new.csv", Path(tmp) / "results.tsv"
            rows = lambda us: Frames.HEADER + "\n".join(f"{i},0,{us},6,,," for i in range(300))
            old.write_text(rows(16000))
            new.write_text(rows(16100))
            self.assertEqual(h.main(["frames-compare", str(old), str(new), "--states", "5,6"]), 0)
            new.write_text(rows(30000))
            self.assertEqual(h.main(["frames-compare", str(old), str(new)]), 1)
            results.write_text("a\tPASS\tok\nb\tSKIPPED\twhy\n")
            self.assertEqual(h.main(["summary", str(results)]), 1)
            results.write_text("a\tPASS\tok\n")
            self.assertEqual(h.main(["summary", str(results)]), 0)

    def test_error_kinds_new_and_scans(self):
        with tempfile.TemporaryDirectory() as tmp:
            old, new = Path(tmp) / "old.log", Path(tmp) / "new.log"
            old.write_text("game <ERROR>\t[1:2:3]\tA 1\n")
            new.write_text("game <ERROR>\t[1:2:4]\tA 2\n")
            self.assertEqual(h.main(["error-kinds-new", "--old", str(old), "--new", str(new)]), 0)
            new.write_text("game <ERROR>\t[1:2:4]\tB\n")
            self.assertEqual(h.main(["error-kinds-new", "--old", str(old), "--new", str(new)]), 1)
            wine = Path(tmp) / "wine.log"
            wine.write_text("fine\n")
            missing = str(Path(tmp) / "nothing" / "*.log")  # an unmatched glob, as bash leaves it
            self.assertEqual(h.main(["scan-client", "--wine-log", str(wine), "--game-log", str(old), missing]), 0)
            self.assertEqual(h.main(["scan-client", "--wine-log", str(wine), "--crash-reports", "1"]), 1)
            server = Path(tmp) / "server.log"
            server.write_text("x [match] [INFO] Match 0x1: player 0 gave the initial world (1 bytes at 50 ms, CRC 0)\n")
            self.assertEqual(h.main(["scan-server", str(server)]), 0)
            server.write_text("x [srp] [ERROR] srp: SRP_MIN_PATCH_REVISION is 0\n")
            self.assertEqual(h.main(["scan-server", str(server)]), 1)


class Summary(unittest.TestCase):
    def test_exit_code(self):
        self.assertEqual(h.summary([("a", "PASS", "")])[1], 0)
        self.assertEqual(h.summary([("a", "PLANNED", "")])[1], 0)
        self.assertEqual(h.summary([("a", "PASS", ""), ("b", "SKIPPED", "why")])[1], 1)
        self.assertEqual(h.summary([("a", "FAIL", "")])[1], 1)
        self.assertEqual(h.summary([])[1], 1)
        text, _ = h.summary([("replay:x:hashes", "SKIPPED", "no hashes")])
        self.assertIn("replay:x:hashes  SKIPPED  no hashes", text)


if __name__ == "__main__":
    unittest.main()

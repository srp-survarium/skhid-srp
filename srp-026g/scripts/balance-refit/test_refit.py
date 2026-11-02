"""Run: python -I scripts/balance-refit/test_refit.py   (no packages needed)"""
import math
import random
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import refit  # noqa: E402

P, G, L, R = refit.IN_FORCE


def line(t, body):
    return f"{t}Z [lobby] [INFO] {body}"


def value(g, l, rating, m):
    return P + G * g + L * l + R * m / (m + 5) * rating


def player(name, g, l, rating, m):
    return f"{name} {value(g, l, rating, m):.1f} (gear {g}, level {l}, rating {rating:+.2f} of {m} matches)"


def minute(mid, s, sides, out=""):
    """A per-minute line as the lobby writes it; sides: (count, players in) for each team."""
    (c0, n0), (c1, n1) = sides
    return line(f"12:{s // 60:02d}:30.000",
                f"Match {mid} at {s} s: team 0 counts {c0:.1f} ({n0} in, lead 0.0), team 1 counts {c1:.1f} "
                f"({n1} in, lead 0.0); points [0, 0], kills [0, 0]; team 0 expected at 50 %"
                + (f"; out: {out}" if out else ""))


def settled(mid, winner, players, reason="TimeLimit"):
    return f"13:00:00.000Z [srp] [INFO] Match {mid} settled ({reason}, winner Some({winner}), {players} players): x"


def read(text):
    with tempfile.TemporaryDirectory() as d:
        (Path(d) / "srp-2026-10-13.log").write_text(text, encoding="utf-8")
        return list(refit.read_matches([d], "", ""))


class Refit(unittest.TestCase):
    def test_parse_pair_and_check(self):
        with tempfile.TemporaryDirectory() as d:
            log = Path(d) / "srp-2026-10-13.log"
            log.write_text(
                line("12:00:00.000", "Match 0xa sides: team 0 counts 30.0: "
                     + player("A, B (x)", 3, 2, 0.5, 4) + " | team 1 counts 20.0: "
                     + player("C", 1, 1, 0.0, 0)) + "\n"
                + "12:30:00.000Z [srp] [INFO] Match 0xa settled (TimeLimit, winner Some(1), 2 players): x\n"
                + line("12:40:00.000", "Match 0xb sides: team 0 counts 1.0: " + player("D", 0, 0, 0, 0)
                       + " | team 1 counts 1.0: " + player("E", 0, 0, 0, 0)) + "\n"
                + "13:00:00.000Z [srp] [INFO] Match 0xb settled (ModeRule, winner Some(2), 2 players): x\n",
                encoding="utf-8")
            ms = list(refit.read_matches([d], "", ""))
            self.assertEqual([m["id"] for m in ms], ["0xa", "0xb"])
            a = ms[0]
            self.assertEqual(a["winner"], 1)
            # A name with ", " and "(" stays one player.
            self.assertEqual([p["name"] for side in a["sides"] for p in side], ["A, B (x)", "C"])
            self.assertTrue(refit.explained_by(a, refit.IN_FORCE, 5.0))
            self.assertFalse(refit.explained_by(a, refit.BEFORE, 5.0))
            self.assertEqual(ms[1]["winner"], 2)
            # since filter; the same file named twice is read once
            self.assertEqual(len(list(refit.read_matches([d], "2026-10-13T12:35", ""))), 1)
            self.assertEqual(len(list(refit.read_matches([d, log], "", ""))), 2)

    def test_lines_since_k_d_balance(self):
        # Since 2026-11-01 the "sides" line names the match's equipment level
        # and each player's parts go on with its K/D there.
        parts = "(gear 3, level 2, rating +0.50 of 4 matches, at 6: kd 1.25 of 40/31)"
        ms = read("\n".join([
            line("12:00:00.000", f"Match 0xc sides (equipment level 6): team 0 counts 4.20: A, B 2.25 {parts}"
                 + f", C 1.95 {parts} | team 1 counts 2.00: D 2.00 {parts}"),
            line("12:05:00.000", f"Match 0xc late join: E 2.10 {parts} to team 1; with them team 0 counts "
                 "4.20 (2 in, lead 0.00), team 1 counts 4.10 (2 in, lead 0.00); the sides were 2.20 apart"),
            settled("0xc", 0, 4),
        ]))
        self.assertEqual(len(ms), 1)
        sides = ms[0]["sides"]
        self.assertEqual([p["name"] for p in sides[0]], ["A, B", "C"])
        self.assertEqual((sides[0][0]["gear"], sides[0][0]["matches"]), (3, 4))
        self.assertEqual([p["name"] for p in sides[1]], ["D"])
        self.assertEqual(ms[0]["events"][0], ("join", [dict(name="E", value=2.1, team=1, gear=3, level=2,
                                                           rating=0.5, matches=4)]))

    def test_features_and_trust(self):
        side = [dict(value=0, gear=3, level=2, rating=1.0, matches=5), dict(value=0, gear=1, level=1, rating=-1.0, matches=0)]
        self.assertEqual(refit.side_features(side, 5.0), (2, 4, 3, 0.5))

    def test_lines_of_a_real_log(self):
        # The lines of a match on the test server (2026-10-13), the names changed:
        # the late-join line had no parts then, and the per-minute lines no "out".
        ms = read("\n".join([
            "22:39:13.506Z [lobby] [INFO] Match 0x5d3dc38a sides: team 0 counts 12.0: Aa 12.0 (gear 0, level 0, rating +0.00 of 0 matches) | team 1 counts 12.0: 77 12.0 (gear 0, level 0, rating +0.00 of 0 matches)",
            "22:39:36.539Z [lobby] [INFO] Match 0x5d3dc38a at 0 s: team 0 counts 12.0 (1 in, lead 0.0), team 1 counts 12.0 (1 in, lead 0.0); points [0, 0], kills [0, 0]; team 0 expected at 50 %",
            "22:41:11.685Z [lobby] [INFO] Match 0x5d3dc38a late join: 5 12.0 to team 0; with them team 0 counts 24.0 (2 in, lead 0.0), team 1 counts 12.0 (1 in, lead 0.0); the sides were 0.0 apart",
            "22:41:36.720Z [lobby] [INFO] Match 0x5d3dc38a at 120 s: team 0 counts 24.0 (2 in, lead 0.0), team 1 counts 12.0 (1 in, lead 0.0); points [0, 0], kills [0, 0]; team 0 expected at 90 %",
            "22:42:36.802Z [lobby] [INFO] Match 0x5d3dc38a at 180 s: team 0 counts 12.0 (1 in, lead 0.0), team 1 counts 12.0 (1 in, lead 0.0); points [0, 0], kills [0, 0]; team 0 expected at 50 %",
            "22:47:41.845Z [srp] [INFO] Match 0x5d3dc38a settled (Deserted, winner Some(2), 3 players): Aa +1 exp +21 silver, 77 +0 exp +0 silver, 5 +0 exp +0 silver",
        ]))
        self.assertEqual(len(ms), 1)
        m = ms[0]
        self.assertTrue(m["late"])
        self.assertEqual((m["winner"], m["settled_players"], m["reason"]), (2, 3, "Deserted"))
        # Two were in it for two of its three minutes.
        self.assertEqual(refit.players_in(m), 2)
        self.assertEqual([kind for kind, _ in m["events"]], ["minute", "join", "minute", "minute"])
        self.assertEqual(m["events"][1][1], [dict(name="5", value=12.0, team=0)])
        self.assertEqual(m["events"][2][1]["n"], (2, 1))
        # Only the first minute gives the roster: after it a player without
        # parts is in, and then one of two is out and the old line does not say who.
        snapshots, unread = refit.rosters(m)
        self.assertEqual((len(snapshots), unread), (1, 2))
        _, how = refit.match_features(m, 5.0)
        self.assertEqual(how, dict(minutes=1, unread=2, changed=True))

    def test_rosters_by_minute(self):
        a, b, c, d = player("Ann, the 2nd", 4, 3, 1.0, 5), player("Bo", 1, 1, 0.0, 0), \
            player("Cy", 2, 2, -0.5, 15), player("Di", 3, 1, 0.0, 2)
        va, vb, vc, vd = value(4, 3, 1.0, 5), value(1, 1, 0.0, 0), value(2, 2, -0.5, 15), value(3, 1, 0.0, 2)
        ve = value(5, 4, 0.0, 0)
        text = "\n".join([
            line("12:00:00.000", f"Match 0xc sides: team 0 counts {va + vb:.1f}: {a}, {b} | team 1 counts {vc + vd:.1f}: {c}, {d}"),
            minute("0xc", 0, ((va + vb, 2), (vc + vd, 2))),
            minute("0xc", 60, ((vb, 1), (vc + vd, 2)), out="Ann, the 2nd"),
            line("12:01:40.000", "Match 0xc late join: " + player("Ed", 5, 4, 0.0, 0) + " to team 0; with them team 0 "
                 f"counts {vb + ve:.1f} (2 in, lead 0.0), team 1 counts {vc + vd:.1f} (2 in, lead 0.0); the sides were 9.0 apart"),
            minute("0xc", 120, ((vb + ve, 2), (vd, 1)), out="Ann, the 2nd, Cy"),
            minute("0xc", 180, ((vb + ve, 2), (vd, 1)), out="Ann, the 2nd, Cy"),
            settled("0xc", 0, 5),
        ])
        m = read(text)[0]
        self.assertTrue(refit.explained_by(m, refit.IN_FORCE, 5.0))  # the joiner's parts too
        snapshots, unread = refit.rosters(m)
        self.assertEqual(unread, 0)
        self.assertEqual([[sorted(p["name"] for p in side) for side in s] for s in snapshots], [
            [["Ann, the 2nd", "Bo"], ["Cy", "Di"]],
            [["Bo"], ["Cy", "Di"]],
            [["Bo", "Ed"], ["Di"]],
            [["Bo", "Ed"], ["Di"]],
        ])
        diff, how = refit.match_features(m, 5.0)
        self.assertEqual(how, dict(minutes=4, unread=0, changed=True))
        # Players: (2-2) + (1-2) + (2-1) + (2-1) over four minutes.
        self.assertAlmostEqual(diff[0], 0.25)
        # Gear: Ann 4, Bo 1, Cy 2, Di 3, Ed 5.
        self.assertAlmostEqual(diff[1], ((5 - 5) + (1 - 5) + (6 - 3) + (6 - 3)) / 4)
        # The sides as the lobby made them, on request.
        diff, how = refit.match_features(m, 5.0, start_roster=True)
        self.assertEqual((diff[0], diff[1], how["minutes"], how["changed"]), (0, 0, 0, True))

    def test_a_minute_that_does_not_add_up_is_left_out(self):
        a, b = player("Aa", 1, 1, 0.0, 0), player("Bb", 2, 1, 0.0, 0)
        va, vb = value(1, 1, 0.0, 0), value(2, 1, 0.0, 0)
        head = line("12:00:00.000", f"Match 0xd sides: team 0 counts {va:.1f}: {a} | team 1 counts {vb:.1f}: {b}")
        for bad in (minute("0xd", 60, ((va, 1), (vb, 0))),            # one is out and the line does not name it
                    minute("0xd", 60, ((va + 3, 1), (vb, 1))),        # the side counts for more than its players
                    minute("0xd", 60, ((va, 1), (0, 0)), out="Zz")):  # a name the roster does not have
            m = read("\n".join([head, minute("0xd", 0, ((va, 1), (vb, 1))), bad, settled("0xd", 1, 2)]))[0]
            self.assertEqual([len(refit.rosters(m)[0]), refit.rosters(m)[1]], [1, 1])
        # No per-minute lines: the starting roster, and the settled count says whether it changed.
        m = read("\n".join([head, settled("0xd", 1, 2)]))[0]
        self.assertEqual(refit.match_features(m, 5.0)[1], dict(minutes=0, unread=0, changed=False))
        m = read("\n".join([head, settled("0xd", 1, 3)]))[0]
        self.assertTrue(refit.match_features(m, 5.0)[1]["changed"])

    def test_small_and_deserted_matches_are_left_out(self):
        a, b = player("Aa", 1, 1, 0.0, 0), player("Bb", 2, 1, 0.0, 0)
        va, vb = value(1, 1, 0.0, 0), value(2, 1, 0.0, 0)
        head = line("12:00:00.000", f"Match 0xe sides: team 0 counts {va:.1f}: {a} | team 1 counts {vb:.1f}: {b}")
        duel = [minute("0xe", s, ((va, 1), (vb, 1))) for s in (0, 60)]
        grown = [minute("0xe", s, ((va, 2), (vb, 2))) for s in (120, 180, 240)]
        # One against one all through; without per-minute lines the starting roster is the size.
        for lines in (duel, []):
            m = read("\n".join([head] + lines + [settled("0xe", 0, 2)]))[0]
            self.assertEqual(refit.players_in(m), 2)
            self.assertEqual(refit.left_out(m, 4), "small")
            self.assertIsNone(refit.left_out(m, 2))
        # It began one against one and was two against two for most of its time.
        m = read("\n".join([head] + duel + grown + [settled("0xe", 0, 4)]))[0]
        self.assertEqual(refit.players_in(m), 4)
        self.assertIsNone(refit.left_out(m, 4))
        # Half of its minutes with four in is enough; less is not.
        m = read("\n".join([head] + duel + grown[:2] + [settled("0xe", 0, 4)]))[0]
        self.assertEqual(refit.players_in(m), 4)
        m = read("\n".join([head] + duel + grown[:1] + [settled("0xe", 0, 4)]))[0]
        self.assertEqual(refit.players_in(m), 2)
        # A side that left gives a winner the fit does not take; a draw has none.
        m = read("\n".join([head] + duel + grown + [settled("0xe", 0, 4, "Deserted")]))[0]
        self.assertEqual(refit.left_out(m, 4), "deserted")
        m = read("\n".join([head] + duel + grown + [settled("0xe", 2, 4)]))[0]
        self.assertEqual(refit.left_out(m, 4), "no winner")

    def test_split_names(self):
        names = ["a", "a, b", "b", "c"]
        self.assertEqual(refit.split_names("", names), [])
        self.assertEqual(refit.split_names("c", names), ["c"])
        self.assertEqual(refit.split_names("a, b, c", names), ["a", "b", "c"])
        self.assertEqual(refit.split_names("a, b, b", names), ["a, b", "b"])
        self.assertIsNone(refit.split_names("a, x", names))

    def test_fit_recovers_known_weights(self):
        rnd = random.Random(7)
        true = [0.0, 1.0, 0.8, 1.5]  # per unit gear = 1.0
        rows = []
        for _ in range(1500):
            x = [rnd.choice([-1, 0, 0, 1]), rnd.randint(-6, 6), rnd.randint(-5, 5), rnd.uniform(-2, 2)]
            d = 0.4 * sum(t * v for t, v in zip(true, x))
            rows.append((x, 1 if rnd.random() < refit.sigmoid(d) else 0))
        w = refit.fit_logistic(rows)
        scaled = [v / w[1] for v in w]
        self.assertAlmostEqual(scaled[2], 0.8, delta=0.25)
        self.assertAlmostEqual(scaled[3], 1.5, delta=0.5)

    def test_scoring_prefers_true_direction(self):
        rnd = random.Random(3)
        feats, ys = [], []
        for _ in range(800):
            x = [0, rnd.randint(-6, 6), rnd.randint(-5, 5), 0.0]
            feats.append(x)
            ys.append(1 if rnd.random() < refit.sigmoid(0.5 * x[2]) else 0)
        good, _, k, wrong = refit.score_weights(feats, ys, (0, 0, 1, 0))
        bad, _, _, _ = refit.score_weights(feats, ys, (0, 1, 0, 0))
        self.assertLess(good, bad)
        self.assertLess(good, math.log(2))
        self.assertAlmostEqual(k, 0.5, delta=0.1)
        self.assertFalse(wrong)
        # Weights that point the wrong way score as a coin, not as well as the right ones.
        loss, acc, k, wrong = refit.score_weights(feats, ys, (0, 0, -1, 0))
        self.assertTrue(wrong)
        self.assertEqual(k, 0.0)
        self.assertAlmostEqual(loss, math.log(2))
        self.assertLess(acc, 0.5)


if __name__ == "__main__":
    unittest.main()

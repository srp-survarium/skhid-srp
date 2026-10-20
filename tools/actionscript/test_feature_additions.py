"""Optional real-compiler test of feature additions; requires SRP_AS_TOOLS=/path/to/tools.json.

A feature may add methods, traits, nested functions, constants and classes,
and redeclare members it names; everything else must stay byte for byte.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source

PROBE = """package {
    public class Probe {
        public var count:int = 1;
        public function existing(value:int):int { return value + count; }
        public function untouched():String { return "same"; }
    }
}"""

FEATURE = """package {
    public class Probe {
        public var count:int = 1;
        public var extra:String = "a new constant";
        public function existing(value:int):int { return value + count + extra.length; }
        public function untouched():String { return "same"; }
        public function added(x:int):int {
            var twice:Function = function(y:int):int { return y * 2; };
            return twice(x) + 7;
        }
    }
}"""


@unittest.skipUnless(os.environ.get("SRP_AS_TOOLS"), "set SRP_AS_TOOLS to run real compiler integration")
class FeatureAdditions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="srp-as-additions-")
        root = cls.root = Path(cls.temp.name)
        config_path = Path(os.environ["SRP_AS_TOOLS"]).resolve()
        config = json.loads(config_path.read_text())
        cls.sdk = Path(config["flex"])
        cls.tools = movie_source.Toolchain(config_path, root / "bridge")
        source = root / "src"
        source.mkdir()
        (source / "Probe.as").write_text(PROBE)
        (source / "Other.as").write_text('package { public class Other { public function kept():int { return 41; } } }')
        (source / "Main.as").write_text('package { public class Main { public var p:Probe; public var o:Other; } }')
        command = [config["java"], "-jar", str(cls.sdk / "lib" / "mxmlc.jar"), "+flexlib=" + str(cls.sdk / "frameworks"),
                   "+playerglobalHome=" + str(cls.sdk / "frameworks/libs/player"),
                   "-external-library-path=" + str(cls.sdk / "frameworks/libs/player/32.0/playerglobal.swc"),
                   "-debug=false", "-optimize=false", "-library-path=", "-source-path=" + str(source),
                   "-output=" + str(root / "movie.swf"), str(source / "Main.as")]
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=120)
        assert result.returncode == 0, result.stdout
        cls.movie = root / "movie.swf"
        cls.compiled = {}

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def compile(self, name, text):
        if name not in self.compiled:
            source = self.root / (name + ".as")
            source.write_text(text)
            output = self.root / (name + ".swf")
            result = self.tools.run("compile", self.movie, output, "Probe", source, self.root / (name + "-snapshot"),
                                    "flex-strict", self.sdk, log=self.root / (name + "-compile.log"))
            self.assertEqual(result.returncode, 0, (self.root / (name + "-compile.log")).read_text())
            self.compiled[name] = output
        return self.compiled[name]

    def link(self, compiled, permissions, cls="Probe", name="link"):
        table, output = self.root / ("link-" + name + ".tsv"), self.root / ("link-" + name + ".swf")
        table.write_text("".join(line + "\n" for line in permissions))
        if output.exists():
            output.unlink()
        result = self.tools.run("link-feature", self.movie, compiled, output, cls, table, log=self.root / (name + ".log"))
        return result.returncode, (self.root / (name + ".log")).read_text(), output

    def rows(self, output):
        report = output.with_name(output.name + ".methods.tsv").read_text().splitlines()[1:]
        return {line.split("\t")[0]: line.split("\t")[1] for line in report}

    def test_added_members_nested_function_and_constant(self):
        compiled = self.compile("feature", FEATURE)
        status, log, output = self.link(compiled, ["edit\tProbe\texisting\t-\t-", "member\tProbe\textra", "member\tProbe\tadded"])
        self.assertEqual(status, 0, log)
        rows = self.rows(output)
        self.assertEqual(sorted(rows.values()), ["added", "added", "changed"])
        self.assertTrue(any(owner.endswith('"added")/function[0]') for owner in rows))
        # The linked movie reparses, and the untouched class keeps its bytes.
        snapshot = self.root / "feature-linked-snapshot"
        self.tools.run("snapshot", output, snapshot, log=self.root / "feature-linked-snapshot.log").check_returncode()
        before = self.root / "original-snapshot"
        if not before.exists():
            self.tools.run("snapshot", self.movie, before, log=self.root / "original-snapshot.log").check_returncode()
        old, new = movie_source.methods(before), movie_source.methods(snapshot)
        for key, row in old.items():
            if '"existing")' not in key[1]:
                self.assertEqual(row["record_sha256"], new[key]["record_sha256"], key)
        self.assertEqual(len(new), len(old) + 2)

    def test_undeclared_additions_and_changes_are_refused(self):
        compiled = self.compile("feature", FEATURE)
        for permissions, message in [
            (["edit\tProbe\texisting\t-\t-", "member\tProbe\textra"], "undeclared"),
            (["edit\tProbe\texisting\t-\t-", "member\tProbe\tadded"], "undeclared member"),
            (["member\tProbe\textra", "member\tProbe\tadded"], "Untouched method changed"),
            (["edit\tProbe\texisting\t-\t-", "member\tProbe\textra", "member\tProbe\tadded", "member\tMissing\tkept"],
             "no game class of this ABC"),
        ]:
            status, log, output = self.link(compiled, permissions, name="refused")
            self.assertNotEqual(status, 0, permissions)
            self.assertIn(message, log)
            self.assertFalse(output.exists())

    def test_a_redeclared_member_needs_its_permission(self):
        compiled = self.compile("redeclared", PROBE.replace("count:int = 1", "count:int = 2"))
        status, log, _ = self.link(compiled, ["member\tProbe\tuntouched"], name="redeclared-refused")
        self.assertNotEqual(status, 0)
        self.assertIn("changed the declaration of Probe instance count", log)
        status, log, _ = self.link(compiled, ["member\tProbe\tcount"], name="redeclared")
        self.assertEqual(status, 0, log)

    def test_an_owned_class_may_change_signatures_but_not_lose_methods(self):
        compiled = self.compile("owned", FEATURE.replace("untouched():String", "untouched(flag:Boolean = false):String"))
        status, log, _ = self.link(compiled, ["edit\tProbe\texisting\t-\t-"], name="owned-refused")
        self.assertNotEqual(status, 0)
        status, log, output = self.link(compiled, ["class\tProbe"], name="owned")
        self.assertEqual(status, 0, log)
        self.assertIn("changed", self.rows(output).values())
        removed = self.compile("removed", PROBE.replace('public function untouched():String { return "same"; }', ""))
        status, log, _ = self.link(removed, ["class\tProbe"], name="removed")
        self.assertNotEqual(status, 0)
        self.assertIn("removed", log)

    def test_a_new_class_goes_after_the_abc_records(self):
        source = self.root / "NewHelper.as"
        source.write_text("package { public class NewHelper { public static function help(o:Other):int { return o.kept() + 1; } } }")
        added = self.root / "new-class.swf"
        result = self.tools.run("add-script", self.movie, added, "Other", source, self.root / "new-class-snapshot",
                                log=self.root / "new-class.log")
        self.assertEqual(result.returncode, 0, (self.root / "new-class.log").read_text())
        status, log, _ = self.link(added, ["member\tOther\tkept"], cls="Other", name="new-refused")
        self.assertNotEqual(status, 0)
        self.assertIn("undeclared class NewHelper", log)
        status, log, output = self.link(added, ["class\tNewHelper"], cls="Other", name="new-class-linked")
        self.assertEqual(status, 0, log)
        rows = self.rows(output)
        self.assertTrue(rows and set(rows.values()) == {"added"})
        self.assertTrue(any('"NewHelper")/static/' in owner for owner in rows))
        again = self.root / "new-class-again.swf"
        result = self.tools.run("add-script", added, again, "Other", source, self.root / "again-snapshot",
                                log=self.root / "new-class-again.log")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already has a class NewHelper", (self.root / "new-class-again.log").read_text())


if __name__ == "__main__":
    unittest.main()

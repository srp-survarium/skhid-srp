"""Optional real-compiler test of edited and new classes; requires SRP_AS_TOOLS=/path/to/tools.json.

An edited class is compiled whole and goes in as compiled (methods anywhere,
new members, changed declarations, a try anywhere); the classes that were not
compiled keep their records.
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

    def link(self, compiled, cls="Probe", name="link"):
        output = self.root / ("link-" + name + ".swf")
        if output.exists():
            output.unlink()
        result = self.tools.run("link-feature", self.movie, compiled, output, cls, log=self.root / (name + ".log"))
        return result.returncode, (self.root / (name + ".log")).read_text(), output

    def records(self, movie, name):
        snapshot = self.root / (name + "-snapshot")
        if not snapshot.exists():
            self.tools.run("snapshot", movie, snapshot, log=self.root / (name + "-snapshot.log")).check_returncode()
        return movie_source.methods(snapshot)

    def assert_links(self, name, text):
        """The edited Probe goes in as compiled; Other and Main keep their records."""
        status, log, output = self.link(self.compile(name, text), name=name)
        self.assertEqual(status, 0, log)
        old, new = self.records(self.movie, "original"), self.records(output, name + "-linked")
        for key, row in old.items():
            if '"Probe")' not in key[1]:
                self.assertEqual(row["record_sha256"], new[key]["record_sha256"], key)
        return old, new

    def test_added_members_nested_function_and_constant(self):
        old, new = self.assert_links("feature", FEATURE)
        self.assertEqual(len(new), len(old) + 2)
        self.assertTrue(any(owner.endswith('"added")/function[0]') for _, owner in new))

    def test_a_method_in_the_middle_and_a_try_in_an_edited_method(self):
        edited = PROBE.replace(
            "public function existing(value:int):int { return value + count; }",
            "public function existing(value:int):int { try { return value + helper(); } catch (e:Error) { } return count; }\n"
            "        public function helper():int { var a:int = 2; var b:int = 3; return a * b + count; }")
        old, new = self.assert_links("middle", edited)
        self.assertEqual(len(new), len(old) + 1)

    def test_a_changed_declaration_and_signature(self):
        self.assert_links("redeclared", PROBE.replace("count:int = 1", "count:Number = 2").replace(
            "untouched():String", "untouched(flag:Boolean = false):String"))

    def test_a_new_class_goes_after_the_abc_records(self):
        source = self.root / "NewHelper.as"
        source.write_text("package { public class NewHelper { public static function help(o:Other):int { return o.kept() + 1; } } }")
        added = self.root / "new-class.swf"
        result = self.tools.run("add-script", self.movie, added, "Other", source, self.root / "new-class-snapshot",
                                log=self.root / "new-class.log")
        self.assertEqual(result.returncode, 0, (self.root / "new-class.log").read_text())
        status, log, output = self.link(added, cls="Other", name="new-class-linked")
        self.assertEqual(status, 0, log)
        old, new = self.records(self.movie, "original"), self.records(output, "new-class")
        self.assertTrue(any('"NewHelper")/static/' in owner for _, owner in new.keys() - old.keys()))
        again = self.root / "new-class-again.swf"
        result = self.tools.run("add-script", added, again, "Other", source, self.root / "again-snapshot",
                                log=self.root / "new-class-again.log")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already has a class NewHelper", (self.root / "new-class-again.log").read_text())

    def add(self, movie, anchor, name, text, tag):
        source = self.root / tag / (name + ".as")
        source.parent.mkdir(exist_ok=True)
        source.write_text(text)
        output = self.root / (tag + ".swf")
        result = self.tools.run("add-script", movie, output, anchor, source, self.root / (tag + "-snapshot"),
                                log=self.root / (tag + ".log"))
        return result.returncode, (self.root / (tag + ".log")).read_text(), output

    def test_a_removed_class_takes_only_its_own_records(self):
        status, log, added = self.add(self.movie, "Other", "Gone",
                                      "package { public class Gone { public function g():int { return 5; } } }", "gone-added")
        self.assertEqual(status, 0, log)
        removed, report = self.root / "gone-removed.swf", self.root / "gone-removed.tsv"
        result = self.tools.run("remove-script", added, removed, "Gone", report, self.root / "gone-removed-snapshot",
                                log=self.root / "gone-removed.log")
        self.assertEqual(result.returncode, 0, (self.root / "gone-removed.log").read_text())
        owners = [line.split("\t")[0] for line in report.read_text().splitlines()[1:]]
        self.assertTrue(owners and all("Gone" in owner for owner in owners), owners)
        original = self.root / "gone-original"
        self.assertEqual(self.tools.run("snapshot", self.movie, original, log=self.root / "o.log").returncode, 0)

        def methods(snapshot):
            rows = [line.split("\t") for line in (snapshot / "methods.tsv").read_text().splitlines()[1:]]
            return {(row[0], row[2]): row[5] for row in rows}
        self.assertEqual(methods(original), methods(self.root / "gone-removed-snapshot"))
        for cls, tag in (("Other", "only-script"), ("Probe", "not-appended")):
            result = self.tools.run("remove-script", self.movie, self.root / (tag + ".swf"), cls, self.root / (tag + ".tsv"),
                                    self.root / (tag + "-snapshot"), log=self.root / (tag + ".log"))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("is not the last script", (self.root / (tag + ".log")).read_text())

    def test_new_classes_of_a_package_name_each_other_once_declared(self):
        whole_a = ("package rooms { public class A { public var b:B;"
                   " public function A() { b = new B(this); b.draw(); } internal var count:int = 2; } }")
        whole_b = ("package rooms { public class B { internal var a:A;"
                   " public function B(owner:A) { a = owner; } public function draw():int { return a.count; } } }")
        status, log, _ = self.add(self.movie, "Other", "A", whole_a, "a-alone")
        self.assertNotEqual(status, 0)
        self.assertIn("not an existing type", log)
        movie = self.movie
        for name in ("A", "B"):
            status, log, movie = self.add(movie, "Other", name, "package rooms { public class " + name + " { } }",
                                          "stub-" + name)
            self.assertEqual(status, 0, log)
        for name, text in (("A", whole_a), ("B", whole_b)):
            source = self.root / ("whole-" + name + ".as")
            source.write_text(text)
            output = self.root / ("whole-" + name + ".swf")
            result = self.tools.run("compile", movie, output, "rooms." + name, source,
                                    self.root / ("whole-" + name + "-snapshot"), "ffdec", self.sdk,
                                    log=self.root / ("whole-" + name + ".log"))
            self.assertEqual(result.returncode, 0, (self.root / ("whole-" + name + ".log")).read_text())
            movie = output
        owners = (self.root / "whole-B-snapshot/methods.tsv").read_text()
        self.assertIn('"rooms"),"A")/instance-init', owners)
        self.assertIn('"B")/instance/1/QName(PackageNamespace(""),"draw")', owners)
        # A's internal field, read from B through the package's namespace.
        draw = [line.split("\t") for line in owners.splitlines() if '"draw")' in line][0]
        asm = (self.root / "whole-B-snapshot" / ("abc-%04d" % int(draw[0])) / ("method-%05d.asm" % int(draw[1]))).read_text()
        self.assertIn('PackageInternalNs("rooms")', asm)


if __name__ == "__main__":
    unittest.main()

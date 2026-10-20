"""Whole-tree assembly must consume every compiled script, including shared ABCs."""
import hashlib
import difflib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import assemble_baseline
import movie_source


class AssemblyReports(unittest.TestCase):
    def test_complete_unique_selection(self):
        expected = {"First", "Second"}
        rows = [{"class": name} for name in sorted(expected)]
        self.assertEqual(set(assemble_baseline.indexed(rows, expected)), expected)
        for bad in (rows[:1], rows + rows[:1], rows + [{"class": "Other"}]):
            with self.assertRaisesRegex(ValueError, "every exported script"):
                assemble_baseline.indexed(bad, expected)


@unittest.skipUnless(os.environ.get("SRP_AS_TOOLS"), "set SRP_AS_TOOLS to run real compiler integration")
class AssemblyIntegration(unittest.TestCase):
    def test_all_compiled_records_and_shared_scripts(self):
        with tempfile.TemporaryDirectory(prefix="srp-assembly-test-") as temp:
            root = Path(temp)
            config_path = Path(os.environ["SRP_AS_TOOLS"]).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config["flex"])
            tools = movie_source.Toolchain(config_path, root / "bridge")
            sources = {
                "AssemblyMain": """package {
                    import flash.display.Sprite;
                    public class AssemblyMain extends Sprite {
                        private var value:AssemblyValue = new AssemblyValue();
                        public function result():int { return value.result(); }
                    }
                }""",
                "AssemblyValue": """package {
                    public class AssemblyValue implements AssemblyContract {
                        public function result():int { return 7; }
                        public function nested():Function {
                            return function(value:int):int { return value+2; };
                        }
                    }
                }""",
                "AssemblyContract": """package {
                    public interface AssemblyContract { function result():int; }
                }""",
            }
            for name, source in sources.items():
                (root / (name + ".as")).write_text(source)
            seed = root / "seed.swf"
            result = subprocess.run([
                config["java"], "-jar", str(sdk / "lib/mxmlc.jar"),
                "+flexlib=" + str(sdk / "frameworks"),
                "-external-library-path=" + str(sdk / "frameworks/libs/player/32.0/playerglobal.swc"),
                "-library-path=", "-source-path=" + str(root), "-debug=false", "-optimize=false",
                "-output=" + str(seed), str(root / "AssemblyMain.as"),
            ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout)
            helper = root / "AssemblyFixture.java"
            helper.write_text("""
                import java.nio.file.*; import java.util.*;
                import com.jpexs.decompiler.flash.tags.Tag;
                public class AssemblyFixture {
                    public static void main(String[] args) throws Exception {
                        var movie = MovieSource.read(Path.of(args[0]));
                        var tags = new ArrayList<>(movie.getAbcList());
                        var first = tags.get(0);
                        for (int i=1; i<tags.size(); i++) {
                            first.getABC().mergeABC(tags.get(i).getABC());
                            movie.removeTag((Tag)tags.get(i));
                        }
                        ((Tag)first).setModified(true);
                        if (first.getABC().script_info.size()!=3)
                            throw new Exception("Expected three independent scripts");
                        try (var out=Files.newOutputStream(Path.of(args[1]))) { movie.saveTo(out); }
                        System.exit(0);
                    }
                }
            """)
            subprocess.run([config["javac"], "-cp", tools.cp, "-d", str(tools.work),
                            str(helper), str(movie_source.HERE / "AssembleBaseline.java")], check=True)
            reference = root / "reference.swf"
            result = subprocess.run([config["java"], "-cp", tools.cp, "AssemblyFixture",
                                     str(seed), str(reference)], capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            plan = []
            for name in sources:
                compiled, linked = root / (name + "-compiled.swf"), root / (name + "-linked.swf")
                log = root / (name + ".log")
                result = tools.run("compile", reference, compiled, name, root / (name + ".as"),
                                   root / (name + "-snapshot"), "flex-strict", sdk, log=log)
                self.assertEqual(result.returncode, 0, log.read_text())
                result = tools.run("link-layout", reference, compiled, linked, name, log=log)
                self.assertEqual(result.returncode, 0, log.read_text())
                plan.append(f"{name}\t{linked}\t{movie_source.digest(linked)}\n")

            def assemble(lines, name, success):
                selected = root / (name + ".tsv")
                selected.write_text("".join(lines))
                output = root / (name + ".swf")
                result = subprocess.run([
                    config["java"], "-cp", tools.cp, "AssembleBaseline", str(reference),
                    str(selected), str(output), str(root / (name + "-coverage.tsv")),
                ], capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
                self.assertEqual(output.exists(), success)
                return output

            assembled = assemble(plan, "complete", True)
            self.assertEqual(movie_source.uncompressed(assembled), movie_source.uncompressed(reference))
            # Exercise a different assembly order as well as missing/duplicate
            # ownership. No retained script may fill in for an absent compiler.
            assemble(list(reversed(plan)), "reversed", True)
            assemble(plan[:-1], "missing", False)
            assemble(plan + plan[:1], "duplicate", False)
            tampered = plan.copy()
            tampered[0] = tampered[0].rsplit("\t", 1)[0] + "\t" + hashlib.sha256(b"wrong").hexdigest() + "\n"
            assemble(tampered, "changed-artifact", False)

            # Exercise the public command and its report/artifact authentication,
            # using a source-only comment patch that leaves executable code exact.
            project, reconstruction = root / "project", root / "reconstruction"
            (project / "pristine").mkdir(parents=True)
            reconstruction.mkdir()
            (project / "original.swf").write_bytes(movie_source.uncompressed(reference))
            for name, source in sources.items():
                (project / "pristine" / (name + ".as")).write_text(source + "\n")
            pin = {"schema": 1, "release": "assembly-fixture",
                   "movie_sha256": movie_source.digest(project / "original.swf"),
                   "sources": {name + ".as": movie_source.digest(project / "pristine" / (name + ".as"))
                               for name in sources}}
            (project / "manifest.json").write_text(json.dumps(pin))
            original = (project / "pristine/AssemblyMain.as").read_text()
            edited = original + "// Reconstructed source fixture.\n"
            patch = "".join(difflib.unified_diff(original.splitlines(True), edited.splitlines(True),
                                                fromfile="a/AssemblyMain.as", tofile="b/AssemblyMain.as"))
            patch_path = reconstruction / "AssemblyMain.patch"
            patch_path.write_text(patch)
            spec = {key: pin[key] for key in ("schema", "release", "movie_sha256")}
            spec["classes"] = [{
                "class": "AssemblyMain", "patch": patch_path.name,
                "pristine_sha256": pin["sources"]["AssemblyMain.as"],
                "patch_sha256": movie_source.digest(patch_path),
                "result_sha256": hashlib.sha256(edited.encode()).hexdigest(),
            }]
            manifest = reconstruction / "manifest.json"
            manifest.write_text(json.dumps(spec))
            build = root / "build"
            result = subprocess.run([
                sys.executable, str(movie_source.HERE / "build_baseline.py"), "--tools", str(config_path),
                "--reconstruction", str(manifest), "--jobs", "2", str(project), str(build),
            ], capture_output=True, text=True, timeout=180)
            diagnostics = result.stdout + result.stderr
            if result.returncode:
                for path in sorted((build / "linked").glob("class-*/link.log")):
                    diagnostics += "\n" + path.read_text()
            self.assertEqual(result.returncode, 0, diagnostics)
            self.assertEqual((build / "assembled/rebuilt.swf").read_bytes(), (project / "original.swf").read_bytes())
            report = json.loads((build / "assembled/report.json").read_text())
            self.assertTrue(report["complete"] and report["file_exact"])
            self.assertEqual((report["scripts"], report["abc_blocks"], report["class_definitions"]), (3, 1, 3))
            self.assertGreater(report["method_signatures"], report["method_bodies"])
            assemble_baseline.inputs(project, build / "compiled", build / "linked")
            artifact = build / "compiled/class-0000/compiled-code.swf"
            artifact.write_bytes(artifact.read_bytes() + b"tamper")
            with self.assertRaisesRegex(ValueError, "Compiler artifact changed"):
                assemble_baseline.inputs(project, build / "compiled", build / "linked")


if __name__ == "__main__":
    unittest.main()

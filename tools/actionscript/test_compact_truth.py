"""Equivalent native Boolean branches, discarded super calls and string literals."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source


@unittest.skipUnless(os.environ.get("SRP_AS_TOOLS"), "set SRP_AS_TOOLS for native compilation")
class CompactTruthIntegration(unittest.TestCase):
    def test_native_encodings_and_control_flow_guards(self):
        with tempfile.TemporaryDirectory(prefix="srp-compact-truth-") as temp:
            root = Path(temp)
            config_path = Path(os.environ["SRP_AS_TOOLS"]).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config["flex"])
            tools = movie_source.Toolchain(config_path, root / "bridge")
            source = root / "CompactTruth.as"
            source.write_text("""package {
                import flash.display.MovieClip;
                public class CompactTruth extends MovieClip {
                    public function lookups(value:Number):void {
                        try { var label:String = "fixture"; super.stop(); Math.max(value,1); trace(label); }
                        catch(error:Error) { trace(error); }
                    }
                    public function localCopy(value:Object):Object {
                        var result:Object;
                        try { if((result = value)) trace(result, value, "local"); }
                        catch(error:Error) { return null; }
                        return result;
                    }
                    public function test(value:*, count:int):String {
                        var result:String = "fixture";
                        var number:int = 100;
                        try {
                            while(count>0) {
                                if(!value) super.stop();
                                if(value) super.play();
                                count--;
                            }
                        } catch(error:Error) { return "caught"; }
                        return result + number;
                    }
                }
            }""")
            seed = root / "seed.swf"
            process = subprocess.run([
                config["java"], "-jar", str(sdk / "lib/mxmlc.jar"),
                "+flexlib=" + str(sdk / "frameworks"),
                "-external-library-path=" + str(sdk / "frameworks/libs/player/32.0/playerglobal.swc"),
                "-library-path=", "-debug=false", "-optimize=false", "-omit-trace-statements=false",
                "-output=" + str(seed), str(source),
            ], capture_output=True, text=True, timeout=90)
            self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
            helper = Path(__file__).parent / "fixtures/CompactTruthFixture.java"
            subprocess.run([config["javac"], "-cp", tools.cp, "-d", str(tools.work), str(helper)], check=True)
            reference = root / "reference.swf"
            process = subprocess.run([
                config["java"], "-cp", tools.cp, "CompactTruthFixture", str(seed), str(reference),
            ], capture_output=True, text=True, timeout=30)
            self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
            compiled = root / "compiled.swf"
            log = root / "compile.log"
            result = tools.run("compile", reference, compiled, "CompactTruth", source,
                               root / "snapshot", "flex-strict", sdk, log=log)
            self.assertEqual(result.returncode, 0, log.read_text())
            linked = root / "linked.swf"
            log = root / "link.log"
            result = tools.run("link-layout", reference, compiled, linked, "CompactTruth", log=log)
            self.assertEqual(result.returncode, 0, log.read_text())
            self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))


if __name__ == "__main__":
    unittest.main()

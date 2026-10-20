"""Optional real Flex regression test; requires SRP_AS_TOOLS=/path/to/tools.json."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile

import movie_source


@unittest.skipUnless(os.environ.get("SRP_AS_TOOLS"), "set SRP_AS_TOOLS to run real compiler integration")
class CompilerIntegration(unittest.TestCase):
    def test_catalog_and_wildcard_vector_dependency(self):
        with tempfile.TemporaryDirectory(prefix="srp-as-test-") as temp:
            root = Path(temp)
            config_path = Path(os.environ["SRP_AS_TOOLS"]).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config["flex"])
            tools = movie_source.Toolchain(config_path, root / "bridge")
            dep = root / "dependency" / "fixture"
            dep.mkdir(parents=True)
            # Separate inherited, field-type and wildcard dependencies exercise
            # both missing catalog edges and ASC's multiname-zero decoder bug.
            (dep / "Value.as").write_text("""package fixture {
                public class Value {
                    public var items:Vector.<*>;
                    public function count():uint { return items.length; }
                }
            }""")
            (dep / "Base.as").write_text("""package fixture {
                public class Base {
                    public var value:Value;
                }
            }""")
            source = root / "probe" / "Probe.as"
            source.parent.mkdir()
            source.write_text("""package {
                import fixture.Base;
                import fixture.Value;
                public class Probe extends Base {
                    public var explicit:Value = new Value();
                    public function count():uint { return value.count(); }
                    public function item():* { return value.items[0]; }
                }
            }""")

            def compiler(jar, args):
                command = [config["java"], "-jar", str(sdk / "lib" / jar),
                           "+flexlib=" + str(sdk / "frameworks"),
                           "-external-library-path=" + str(sdk / "frameworks/libs/player/32.0/playerglobal.swc"),
                           "-debug=false", "-optimize=false", *args]
                return subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                      text=True, timeout=90)

            library = root / "library.swc"
            result = compiler("compc.jar", ["-library-path=", "-source-path=" + str(dep.parent), "-include-classes=fixture.Base,fixture.Value",
                                            "-output=" + str(library)])
            self.assertEqual(result.returncode, 0, result.stdout)
            # Reproduce FFDec's incomplete catalog without changing the ABC.
            with zipfile.ZipFile(library) as archive:
                entries = {name: archive.read(name) for name in archive.namelist()}
            # Flash's original compiler uses multiname 0 for Vector.<*>; Flex
            # itself emits an explicit '*' QName. Construct the legal Flash
            # representation so the regression reproduces the real input.
            fixture = root / "WildcardFixture.java"
            fixture.write_text("""
                import java.nio.file.*; import java.io.*;
                import com.jpexs.decompiler.flash.SWF;
                import com.jpexs.decompiler.flash.tags.Tag;
                public class WildcardFixture {
                    public static void main(String[] args) throws Exception {
                        var swf = new SWF(Files.newInputStream(Path.of(args[0])), true);
                        int changed = 0;
                        for (var tag : swf.getAbcList()) {
                            var abc = tag.getABC();
                            if (args.length == 3) {
                                int cls = abc.findClassByName("Probe");
                                if (cls < 0) continue;
                                if (args[2].equals("reorder")) {
                                    int left = abc.instance_info.get(cls).iinit_index;
                                    int right = abc.class_info.get(cls).cinit_index;
                                    java.util.Collections.swap(abc.method_info, left, right);
                                    for (var body : abc.bodies) {
                                        if (body.method_info == left) body.method_info = right;
                                        else if (body.method_info == right) body.method_info = left;
                                    }
                                    abc.instance_info.get(cls).iinit_index = right;
                                    abc.class_info.get(cls).cinit_index = left;
                                    ((Tag)tag).setModified(true); changed++; continue;
                                }
                                var body = abc.findBody(abc.instance_info.get(cls).iinit_index);
                                if (args[2].equals("metadata")) body.max_stack++;
                                else {
                                    byte[] code = body.getCodeBytes().clone();
                                    code[0] = 0x20; // pushnull replaces getlocal0
                                    body.setCodeBytes(code);
                                }
                                ((Tag)tag).setModified(true); changed++;
                                continue;
                            }
                            for (int j=1; j<abc.constants.getMultinameCount(); j++) {
                                var m = abc.constants.getMultiname(j);
                                if (m.kind != 0x1d) continue;
                                for (int k=0; k<m.params.length; k++) {
                                    if (m.params[k] == 0) { changed++; continue; }
                                    var param = abc.constants.getMultiname(m.params[k]);
                                    if (abc.constants.getString(param.name_index).equals("*")) {
                                        m.params[k] = 0; changed++; ((Tag)tag).setModified(true);
                                    }
                                }
                            }
                        }
                        if (changed == 0) throw new IOException("Missing wildcard fixture");
                        try (var out = Files.newOutputStream(Path.of(args[1]))) { swf.saveTo(out); }
                        System.exit(0);
                    }
                }
            """)
            subprocess.run([config["javac"], "-cp", config["ffdec"], "-d", str(root), str(fixture)], check=True, timeout=60)
            (root / "library.swf").write_bytes(entries["library.swf"])
            subprocess.run([config["java"], "-cp", str(root) + os.pathsep + config["ffdec"], "WildcardFixture",
                            str(root / "library.swf"), str(root / "wildcard.swf")], check=True, timeout=60)
            entries["library.swf"] = (root / "wildcard.swf").read_bytes()
            catalog = ET.fromstring(entries["catalog.xml"])
            for script in catalog.iter():
                if script.tag.split("}")[-1] == "script":
                    for child in list(script):
                        if child.tag.split("}")[-1] == "dep":
                            script.remove(child)
            # Preserve the default namespace, as emitted by FFDec.
            ET.register_namespace("", "http://www.adobe.com/flash/swccatalog/9")
            entries["catalog.xml"] = ET.tostring(catalog)
            incomplete = root / "incomplete.swc"
            with zipfile.ZipFile(incomplete, "w") as archive:
                for name, data in entries.items():
                    archive.writestr(name, data)

            def compile_probe(swc, name):
                return compiler("mxmlc.jar", ["-strict=true", "-library-path=" + str(swc),
                    "-source-path=" + str(source.parent), "-output=" + str(root / name), str(source)])

            rejected = compile_probe(incomplete, "rejected.swf")
            self.assertNotEqual(rejected.returncode, 0, "Fixture failed to reproduce incomplete dependency failure")
            repaired = root / "repaired.swc"
            tools.run("dependencies", incomplete, repaired, log=root / "repair.log").check_returncode()
            accepted = compile_probe(repaired, "accepted.swf")
            self.assertEqual(accepted.returncode, 0, accepted.stdout)
            self.assertTrue((root / "accepted.swf").is_file())
            # Verify the adapter didn't mutate the original compiler input.
            with zipfile.ZipFile(incomplete) as archive:
                self.assertEqual(archive.read("library.swf"), entries["library.swf"])
            # An unchanged compiler-produced ABC survives layout linking, but
            # code or verifier metadata mutations cannot be filled from retail.
            accepted_file = root / "accepted.swf"
            linked = root / "linked.swf"
            tools.run("link-layout", accepted_file, accepted_file, linked, "Probe", log=root / "link.log").check_returncode()
            self.assertEqual(movie_source.uncompressed(accepted_file), movie_source.uncompressed(linked))
            reordered = root / "reordered.swf"
            subprocess.run([config["java"], "-cp", str(root) + os.pathsep + config["ffdec"], "WildcardFixture",
                            str(accepted_file), str(reordered), "reorder"], check=True, timeout=60)
            restored = root / "restored.swf"
            self.assertNotEqual(movie_source.uncompressed(accepted_file), movie_source.uncompressed(reordered))
            tools.run("link-layout", accepted_file, reordered, restored, "Probe", log=root / "reorder.log").check_returncode()
            self.assertEqual(movie_source.uncompressed(accepted_file), movie_source.uncompressed(restored))
            for mutation in ("code", "metadata"):
                corrupted = root / (mutation + ".swf")
                subprocess.run([config["java"], "-cp", str(root) + os.pathsep + config["ffdec"], "WildcardFixture",
                                str(accepted_file), str(corrupted), mutation], check=True, timeout=60)
                destination = root / (mutation + "-linked.swf")
                rejected = tools.run("link-layout", accepted_file, corrupted, destination, "Probe", log=root / (mutation + ".log"))
                self.assertNotEqual(rejected.returncode, 0)
                self.assertFalse(destination.exists(), "Rejected compilation must never produce a linked movie")


if __name__ == "__main__":
    unittest.main()

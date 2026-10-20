"""An explicit default-package type import must preserve its class identity."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source


@unittest.skipUnless(os.environ.get("SRP_AS_TOOLS"), "set SRP_AS_TOOLS for native compilation")
class GlobalTypeImportIntegration(unittest.TestCase):
    def test_explicit_import_and_different_source_binding(self):
        with tempfile.TemporaryDirectory(prefix="srp-global-type-") as temp:
            root = Path(temp)
            config_path = Path(os.environ["SRP_AS_TOOLS"]).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config["flex"])
            tools = movie_source.Toolchain(config_path, root / "bridge")
            (root / "fixture").mkdir()
            (root / "fixture/SharedName.as").write_text(
                "package fixture { public class SharedName {} }")
            (root / "SharedName.as").write_text(
                "package { public class SharedName {} }")
            source = root / "fixture/ImportProbe.as"
            source.write_text("""package fixture {
                import flash.display.Sprite;
                import fixture.SharedName;
                public class ImportProbe extends Sprite {
                    public var value:fixture.SharedName;
                    public function ImportProbe() { super(); }
                }
            }""")
            entry = root / "Seed.as"
            entry.write_text("""package {
                import flash.display.Sprite;
                import fixture.ImportProbe;
                public class Seed extends Sprite {
                    public var globalValue:SharedName;
                    public var probe:ImportProbe;
                }
            }""")
            seed = root / "seed.swf"
            result = subprocess.run([
                config["java"], "-jar", str(sdk / "lib/mxmlc.jar"),
                "+flexlib=" + str(sdk / "frameworks"),
                "-external-library-path=" + str(sdk / "frameworks/libs/player/32.0/playerglobal.swc"),
                "-library-path=", "-source-path=" + str(root),
                "-debug=false", "-optimize=false", "-omit-trace-statements=false",
                "-output=" + str(seed), str(entry),
            ], capture_output=True, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            helper = root / "GlobalTypeFixture.java"
            helper.write_text("""
                import java.nio.file.*;
                import com.jpexs.decompiler.flash.abc.types.*;
                import com.jpexs.decompiler.flash.abc.types.traits.*;
                import com.jpexs.decompiler.flash.tags.Tag;
                public class GlobalTypeFixture {
                    public static void main(String[] args)throws Exception {
                        var swf=MovieSource.read(Path.of(args[0]));
                        var abc=MovieSource.findScript(swf,"fixture.ImportProbe").abc;
                        if(swf.getAbcIndex().findClass(new com.jpexs.decompiler.graph.TypeItem("SharedName"),
                                abc,null)==null)throw new AssertionError("Missing global dependency");
                        int index=abc.findClassByName("fixture.ImportProbe"),count=0;
                        for(var trait:abc.instance_info.get(index).instance_traits.traits)
                            if(trait instanceof TraitSlotConst field) {
                                if(!"fixture.SharedName".equals(MovieSource.publicName(abc,field.type_index)))
                                    throw new AssertionError("Unexpected native field type");
                                field.type_index=abc.constants.getMultinameId(Multiname.createQName(false,
                                    abc.constants.getStringId("SharedName",true),
                                    abc.constants.getNamespaceId(0x16,"",0,true)),true);
                                count++;
                            }
                        if(count!=1)throw new AssertionError("Unexpected native field count");
                        ((Tag)abc.parentTag).setModified(true);
                        try(var out=Files.newOutputStream(Path.of(args[1]))){swf.saveTo(out);}
                        System.exit(0);
                    }
                }
            """)
            subprocess.run([config["javac"], "-cp", tools.cp, "-d", str(tools.work), str(helper)], check=True)
            reference = root / "reference.swf"
            result = subprocess.run([config["java"], "-cp", tools.cp, "GlobalTypeFixture", str(seed), str(reference)],
                                    capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            imported = source.read_text().replace("import fixture.SharedName;",
                "import SharedName;").replace("value:fixture.SharedName", "value:SharedName")

            for mode, text in [
                ("explicit", imported),
                ("package", imported.replace("value:SharedName", "value:fixture.SharedName")),
                ("implicit", imported.replace("import SharedName;", "")),
                ("ambiguous", imported.replace("import SharedName;", "import SharedName;\nimport fixture.SharedName;")),
            ]:
                selected = root / (mode + ".as")
                selected.write_text(text)
                compiled = root / (mode + ".swf")
                log = root / (mode + "-compile.log")
                result = tools.run("compile", reference, compiled, "fixture.ImportProbe", selected,
                                   root / (mode + "-snapshot"), "ffdec-source", sdk, log=log)
                self.assertEqual(result.returncode, 0, log.read_text())
                linked = root / (mode + "-linked.swf")
                log = root / (mode + "-link.log")
                result = tools.run("link-layout", reference, compiled, linked, "fixture.ImportProbe", log=log)
                self.assertEqual(result.returncode == 0, mode == "explicit", log.read_text())
                if mode == "explicit":
                    self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))
                else:
                    self.assertFalse(linked.exists())


if __name__ == "__main__":
    unittest.main()

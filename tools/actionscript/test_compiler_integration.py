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
                    public function identity(value:int=1):int { return value; }
                }
            }""")

            def compiler(jar, args):
                command = [config["java"], "-jar", str(sdk / "lib" / jar),
                           "+flexlib=" + str(sdk / "frameworks"),
                           "+playerglobalHome=" + str(sdk / "frameworks/libs/player"),
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
                import com.jpexs.decompiler.flash.abc.types.*;
                import com.jpexs.decompiler.flash.abc.types.traits.TraitClass;
                import com.jpexs.decompiler.flash.abc.types.traits.Trait;
                import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
                public class WildcardFixture {
                    public static void main(String[] args) throws Exception {
                        var swf = new SWF(Files.newInputStream(Path.of(args[0])), true);
                        int changed = 0;
                        for (var tag : swf.getAbcList()) {
                            var abc = tag.getABC();
                            if (args.length == 3) {
                                int cls = abc.findClassByName("Probe");
                                if (cls < 0) continue;
                                if (args[2].startsWith("annotation-")) {
                                    var trait = abc.script_info.get(0).traits.traits.get(0);
                                    MetadataInfo metadata;
                                    if (args[2].equals("annotation-new")) metadata = new MetadataInfo(
                                        abc.constants.getStringId("FixtureMetadata", true),
                                        new int[]{abc.constants.getStringId("key", true)},
                                        new int[]{abc.constants.getStringId("original", true)});
                                    else {
                                        var old = abc.metadata_info.get(trait.metadata[0]);
                                        metadata = new MetadataInfo(old.name_index, old.keys.clone(), old.values.clone());
                                        if (args[2].equals("annotation-changed"))
                                            metadata.values[0] = abc.constants.getStringId("changed", true);
                                    }
                                    trait.metadata = new int[]{abc.metadata_info.size()};
                                    trait.kindFlags |= Trait.ATTR_Metadata;
                                    abc.metadata_info.add(metadata);
                                    ((Tag)tag).setModified(true); changed++; continue;
                                }
                                if (java.util.Set.of("prepare", "binding", "scope", "slot", "paramnames", "optional", "binding-code", "debug-lines").contains(args[2])) {
                                    var script = abc.script_info.get(0);
                                    var trait = (TraitClass)script.traits.traits.get(0);
                                    var name = abc.constants.getMultiname(trait.name_index);
                                    var multi = Multiname.createMultiname(false, name.name_index,
                                        abc.constants.getNamespaceSetId(new int[]{name.namespace_index}, true));
                                    int id = abc.constants.getMultinameId(multi, true);
                                    if (args[2].equals("prepare")) {
                                        for (var method : abc.method_info) {
                                            method.unsetFlagHas_paramnames(); method.paramNames = new int[0];
                                        }
                                    } else if (args[2].equals("debug-lines")) {
                                        var body = abc.findBody(abc.class_info.get(cls).cinit_index);
                                        var code = body.getCode();
                                        code.code.add(2, new AVM2Instruction(0, 0xf1, new int[]{name.name_index}));
                                        code.code.add(3, new AVM2Instruction(0, 0xf0, new int[]{17}));
                                        code.markOffsets(); body.setCode(code);
                                    } else if (args[2].equals("slot")) trait.slot_id = 2;
                                    else if (args[2].equals("paramnames") || args[2].equals("optional")) {
                                        for (var method : abc.method_info) if (method.param_types.length == 1) {
                                            if (args[2].equals("paramnames")) {
                                                method.setFlagHas_paramnames();
                                                method.paramNames = new int[]{abc.constants.getStringId("addedDebugName", true)};
                                            } else method.optional[0].value_index = abc.constants.getIntId(2, true);
                                        }
                                    } else {
                                        var body = abc.findBody(script.init_index);
                                        var code = body.getCode();
                                        if (code.code.get(2).definition.instructionCode != 0x65)
                                            throw new IOException("Expected compiler global-scope binding");
                                        if (args[2].equals("binding")) {
                                            code.code.set(2, new AVM2Instruction(code.code.get(2).getAddress(), 0x5d, new int[]{id}));
                                            trait.slot_id = 0;
                                        } else if (args[2].equals("scope")) code.code.get(2).operands[0] = 1;
                                        else code.code.set(0, new AVM2Instruction(0, 0x20, new int[0]));
                                        code.markOffsets(); body.setCode(code);
                                    }
                                    ((Tag)tag).setModified(true); changed++; continue;
                                }
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
                                var body = abc.findBody(args[2].equals("debug-code")
                                    ? abc.class_info.get(cls).cinit_index : abc.instance_info.get(cls).iinit_index);
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

            def mutate(source, destination, mode):
                subprocess.run([config["java"], "-cp", str(root) + os.pathsep + config["ffdec"], "WildcardFixture",
                                str(source), str(destination), mode], check=True, timeout=60)

            # Retain the same pool in both files, then exercise Flash's binding
            # emission and implicit script slot against real compiler code.
            prepared, binding = root / "prepared.swf", root / "binding.swf"
            mutate(accepted_file, prepared, "prepare")
            mutate(prepared, binding, "binding")
            bound = root / "bound.swf"
            tools.run("link-layout", binding, prepared, bound, "Probe", log=root / "binding.log").check_returncode()
            self.assertEqual(movie_source.uncompressed(binding), movie_source.uncompressed(bound))
            named, unnamed = root / "named.swf", root / "unnamed.swf"
            mutate(prepared, named, "paramnames")
            tools.run("link-layout", prepared, named, unnamed, "Probe", log=root / "paramnames.log").check_returncode()
            self.assertEqual(movie_source.uncompressed(prepared), movie_source.uncompressed(unnamed))
            debug, debug_linked = root / "debug.swf", root / "debug-linked.swf"
            mutate(prepared, debug, "debug-lines")
            tools.run("link-layout", debug, prepared, debug_linked, "Probe", log=root / "debug.log").check_returncode()
            self.assertEqual(movie_source.uncompressed(debug), movie_source.uncompressed(debug_linked))
            debug_corrupt, debug_rejected = root / "debug-corrupt.swf", root / "debug-rejected.swf"
            mutate(prepared, debug_corrupt, "debug-code")
            rejected = tools.run("link-layout", debug, debug_corrupt, debug_rejected, "Probe", log=root / "debug-code.log")
            self.assertNotEqual(rejected.returncode, 0)
            self.assertFalse(debug_rejected.exists())
            annotation = root / "annotation.swf"
            mutate(prepared, annotation, "annotation-new")
            for mode in ("duplicate", "changed"):
                candidate, destination = root / (mode + "-annotation.swf"), root / (mode + "-annotation-linked.swf")
                mutate(annotation, candidate, "annotation-" + mode)
                result = tools.run("link-layout", annotation, candidate, destination, "Probe", log=root / (mode + "-annotation.log"))
                if mode == "duplicate":
                    result.check_returncode()
                    self.assertEqual(movie_source.uncompressed(annotation), movie_source.uncompressed(destination))
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(destination.exists())
            for mutation in ("scope", "slot", "optional", "binding-code"):
                corrupted, destination = root / (mutation + ".swf"), root / (mutation + "-linked.swf")
                mutate(prepared, corrupted, mutation)
                rejected = tools.run("link-layout", binding, corrupted, destination, "Probe", log=root / (mutation + ".log"))
                self.assertNotEqual(rejected.returncode, 0, mutation)
                self.assertFalse(destination.exists(), mutation)
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

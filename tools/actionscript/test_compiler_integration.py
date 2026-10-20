"""Optional real Flex regression test; requires SRP_AS_TOOLS=/path/to/tools.json."""
import json
import os
import re
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
                import flash.text.TextField;
                import flash.utils.Timer;
                public class Base {
                    public var value:Value;
                    public var textField:TextField;
                    public var timer:Timer;
                }
            }""")
            source = root / "probe" / "Probe.as"
            source.parent.mkdir()
            source.write_text("""package {
                import fixture.Base;
                import fixture.Value;
                import flash.text.TextFormat;
                public class Probe extends Base {
                    public var explicit:Value = new Value();
                    private var secret:int = 7;
                    private function hidden():int { return secret; }
                    public function inspect(target:Object, key:String):* { return target[key]; }
                    public function nested(value:int):Function {
                        var first:Function = function(delta:int):int { return value+delta+secret; };
                        var second:Function = function(delta:int):int { return first(delta)+1; };
                        return second;
                    }
                    public function branches(n:int=0):int {
                        var total:int=0;
                        while(n>0) { if(n==2) total+=hidden(); n--; }
                        try {
                            switch(n) {
                                case 0: return secret;
                                case 1: return hidden();
                                case 2: return secret+1;
                                case 3: return hidden()+2;
                                default: throw new Error("fixture");
                            }
                        } catch(error:Error) { return secret+total; }
                        return total;
                    }
                    public function count():uint { return value.count(); }
                    public function api():TextFormat {
                        timer.reset();
                        return textField.getTextFormat();
                    }
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
                import com.jpexs.decompiler.flash.abc.avm2.AVM2Code;
                public class WildcardFixture {
                    public static void main(String[] args) throws Exception {
                        var swf = new SWF(Files.newInputStream(Path.of(args[0])), true);
                        int changed = 0;
                        for (var tag : swf.getAbcList()) {
                            var abc = tag.getABC();
                            if (args.length >= 3) {
                                int cls = abc.findClassByName("Probe");
                                if (cls < 0) continue;
                                for (var methodBody : abc.bodies) MovieSource.exactCode(methodBody);
                                if (args[2].equals("feature-pool")) {
                                    var reference=new SWF(Files.newInputStream(Path.of(args[3])),true);
                                    var original=reference.getAbcList().stream().map(t->t.getABC())
                                        .filter(a->a.findClassByName("Probe")>=0).findFirst().orElseThrow();
                                    int found=0;
                                    for(int i=original.constants.getMultinameCount();i<abc.constants.getMultinameCount();i++) {
                                        var m=abc.constants.getMultiname(i);
                                        if(!m.hasOwnName() || !abc.constants.getString(m.name_index).equals("new_feature_property"))continue;
                                        if(!m.hasOwnNamespaceSet())throw new IOException("Expected dynamic feature lookup");
                                        for(int ns:abc.constants.getNamespaceSet(m.namespace_set_index).namespaces)
                                            if(abc.constants.getNamespace(ns).kind==5 && ns>=original.constants.getNamespaceCount())
                                                throw new IOException("Feature lookup retained a fresh private namespace");
                                        found++;
                                    }
                                    if(found==0)throw new IOException("No appended feature name tested");
                                    changed++;continue;
                                }
                                if (args[2].equals("pool")) {
                                    int id=abc.constants.getStringId("fixture",false);
                                    if(id<=0)throw new IOException("Missing original string fixture");
                                    abc.constants.setString(id,"tampered original constant");
                                    ((Tag)tag).setModified(true);changed++;continue;
                                }
                                if (args[2].equals("other-class")) {
                                    int other=abc.findClassByName("Retained");
                                    if(other<0)throw new IOException("Missing retained class");
                                    var body=abc.findBody(abc.instance_info.get(other).iinit_index);
                                    byte[] code=body.getCodeBytes().clone();code[0]=0x20;body.setCodeBytes(code);
                                    ((Tag)tag).setModified(true);changed++;continue;
                                }
                                if (args[2].startsWith("closures-")) {
                                    var sites=new java.util.ArrayList<AVM2Instruction>();
                                    for(var body:abc.bodies)for(var ins:body.getCode().code)
                                        if(ins.definition.instructionCode==0x40)sites.add(ins);
                                    if(sites.size()!=2)throw new IOException("Expected two local function sites");
                                    int left=sites.get(0).operands[0],right=sites.get(1).operands[0];
                                    if(args[2].equals("closures-reference")) {
                                        abc.method_info.get(left).name_index=abc.constants.getStringId("legacyFirst",true);
                                        abc.method_info.get(right).name_index=abc.constants.getStringId("legacySecond",true);
                                    } else if(args[2].equals("closures-reorder")) {
                                        java.util.Collections.swap(abc.method_info,left,right);
                                        abc.method_info.get(left).name_index=0;abc.method_info.get(right).name_index=0;
                                        for(var body:abc.bodies){
                                            if(body.method_info==left)body.method_info=right;
                                            else if(body.method_info==right)body.method_info=left;
                                        }
                                        sites.get(0).operands[0]=right;sites.get(1).operands[0]=left;
                                    } else if(args[2].equals("closures-target")) sites.get(1).operands[0]=left;
                                    else {
                                        var code=abc.findBody(left).getCode();
                                        code.code.set(0,new AVM2Instruction(0,0x20,new int[0]));
                                    }
                                    for(var body:abc.bodies)body.setCode(body.getCode());
                                    ((Tag)tag).setModified(true);changed++;continue;
                                }
                                if (args[2].equals("merge-scripts")) {
                                    int merged=0;
                                    var extra = new SWF(Files.newInputStream(Path.of(args[3])), true);
                                    for (var other : extra.getAbcList()) {
                                        for(var body:other.getABC().bodies) MovieSource.exactCode(body);
                                        abc.mergeABC(other.getABC());merged++;
                                    }
                                    if(merged==0 || abc.script_info.size()<2) throw new IOException("Missing multi-script fixture");
                                    ((Tag)tag).setModified(true);changed++;continue;
                                }
                                if (args[2].equals("traits")) {
                                    java.util.Collections.reverse(abc.instance_info.get(cls).instance_traits.traits);
                                    ((Tag)tag).setModified(true);changed++;continue;
                                }
                                if (args[2].startsWith("control-")) {
                                    var body=abc.bodies.stream().filter(b -> b.exceptions.length>0).findFirst().orElseThrow();
                                    var code=body.getCode();
                                    if (args[2].equals("control-handler")) body.exceptions[0].target=0;
                                    else {
                                        var branch=code.code.stream().filter(i -> i.definition.instructionCode>=0x0c
                                            && i.definition.instructionCode<=0x1a).findFirst().orElseThrow();
                                        long target=0;
                                        if (args[2].equals("control-invalid")) target=code.code.stream()
                                            .filter(i -> i.getBytesLength()>1).findFirst().orElseThrow().getAddress()+1;
                                        branch.operands[0]=(int)(target-branch.getAddress()-branch.getBytesLength());
                                    }
                                    body.setCode(code);((Tag)tag).setModified(true);changed++;continue;
                                }
                                if (args[2].startsWith("namespace-")) {
                                    var namespaces = new java.util.HashMap<Integer,Integer>();
                                    int namespaceCount = abc.constants.getNamespaceCount();
                                    for (int i=1; i<namespaceCount; i++) {
                                        var ns = abc.constants.getNamespace(i);
                                        String label = abc.constants.getString(ns.name_index);
                                        if (ns.kind != 5 || !(label.equals("Probe") || label.startsWith("Probe.as$"))) continue;
                                        String replacement = label.equals("Probe") ? "Probe" : "Probe.as$999";
                                        if (args[2].equals("namespace-owner")) replacement = "OtherOwner";
                                        var copy = new Namespace(); copy.kind = 5;
                                        copy.name_index = abc.constants.getStringId(replacement, true);
                                        namespaces.put(i, abc.constants.addNamespace(copy));
                                        if (args[2].equals("namespace-ambiguous") && label.equals("Probe")) {
                                            var duplicate = new Namespace(); duplicate.kind=5; duplicate.name_index=copy.name_index;
                                            abc.constants.addNamespace(duplicate);
                                        }
                                    }
                                    if (namespaces.isEmpty()) throw new IOException("Missing private namespace fixture");
                                    var names = new java.util.HashMap<Integer,Integer>();
                                    int nameCount = abc.constants.getMultinameCount();
                                    if (args[2].equals("namespace-wide")) {
                                        while(abc.constants.getMultinameCount()<130)
                                            abc.constants.addMultiname(Multiname.createQName(false,
                                                abc.constants.getStringId("padding"+abc.constants.getMultinameCount(),true),
                                                abc.constants.getMultiname(abc.instance_info.get(cls).name_index).namespace_index));
                                    }
                                    for (int i=1; i<nameCount; i++) {
                                        var old = abc.constants.getMultiname(i);
                                        var copy = new Multiname();
                                        copy.kind=old.kind; copy.name_index=old.name_index;
                                        copy.namespace_index=old.namespace_index; copy.namespace_set_index=old.namespace_set_index;
                                        copy.qname_index=old.qname_index; copy.params=old.params == null ? null : old.params.clone();
                                        boolean moved=false;
                                        if (old.hasOwnNamespace() && namespaces.containsKey(old.namespace_index)) {
                                            copy.namespace_index=namespaces.get(old.namespace_index); moved=true;
                                        }
                                        if (old.hasOwnNamespaceSet()) {
                                            int[] oldSet=abc.constants.getNamespaceSet(old.namespace_set_index).namespaces;
                                            int extra=args[2].equals("namespace-member") ? 1 : 0;
                                            int[] newSet=new int[oldSet.length+extra];
                                            for(int j=0;j<oldSet.length;j++) newSet[j]=namespaces.getOrDefault(oldSet[oldSet.length-1-j],oldSet[oldSet.length-1-j]);
                                            if(extra!=0) {
                                                var ns=new Namespace();ns.kind=0x16;ns.name_index=abc.constants.getStringId("unrelated",true);
                                                newSet[oldSet.length]=abc.constants.addNamespace(ns);
                                            }
                                            copy.namespace_set_index=abc.constants.addNamespaceSet(new NamespaceSet(newSet)); moved=true;
                                        }
                                        if(moved) names.put(i,abc.constants.addMultiname(copy));
                                    }
                                    for(var trait:abc.instance_info.get(cls).instance_traits.traits)
                                        trait.name_index=names.getOrDefault(trait.name_index,trait.name_index);
                                    int widths=0,switches=0,forward=0,backward=0,handlers=0;
                                    for(var body:abc.bodies){
                                        var code=body.getCode();
                                        handlers+=body.exceptions.length;
                                        for(var ins:code.code) {
                                            int opcode=ins.definition.instructionCode;
                                            if(opcode==0x1b) switches++;
                                            else if(opcode>=0x0c && opcode<=0x1a) {
                                                if(ins.operands[0]>0)forward++;else if(ins.operands[0]<0)backward++;
                                            }
                                            int oldWidth=ins.getBytesLength();
                                            for(int j=0;j<ins.definition.operands.length;j++)
                                                if(ins.definition.operands[j]==AVM2Code.DAT_MULTINAME_INDEX)
                                                    // FFDec's own incremental relocation constructs
                                                    // the fixture independently of our batch linker.
                                                    ins.setOperand(j,names.getOrDefault(ins.operands[j],ins.operands[j]),code,body);
                                            if(oldWidth!=ins.getBytesLength())widths++;
                                        }
                                        body.setCode(code);
                                    }
                                    if(args[2].equals("namespace-wide") && (widths==0 || switches==0 || forward==0 || backward==0 || handlers==0))
                                        throw new IOException("Incomplete relocation fixture coverage");
                                    ((Tag)tag).setModified(true);changed++;continue;
                                }
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
                                if (java.util.Set.of("prepare", "binding", "scope", "slot", "paramnames", "optional", "binding-code", "debug-lines", "debug-flow").contains(args[2])) {
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
                                    } else if (args[2].equals("debug-flow")) {
                                        var body=abc.bodies.stream().filter(b -> b.exceptions.length>0).findFirst().orElseThrow();
                                        var code=body.getCode();
                                        for(int i=code.code.size()-1;i>=0;i--) {
                                            var instruction=code.code.get(i);
                                            if(instruction.isBranch() || instruction.isJump() || instruction.definition.instructionCode==0x09)
                                                code.insertInstruction(i,new AVM2Instruction(0,0xf0,new int[]{i+1}),body);
                                        }
                                        code.insertInstruction(0,new AVM2Instruction(0,0xf1,new int[]{name.name_index}),body);
                                        body.setCode(code);
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
                                            } else if(method.optional.length>0) method.optional[0].value_index = abc.constants.getIntId(2, true);
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
            subprocess.run([config["javac"], "-cp", str(root / "bridge") + os.pathsep + config["ffdec"], "-d", str(root), str(fixture)], check=True, timeout=60)
            (root / "library.swf").write_bytes(entries["library.swf"])
            subprocess.run([config["java"], "-cp", str(root) + os.pathsep + str(root / "bridge") + os.pathsep + config["ffdec"], "WildcardFixture",
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
            tools.run("dependencies", incomplete, repaired,
                      sdk / "frameworks/libs/player/32.0/playerglobal.swc", log=root / "repair.log").check_returncode()
            with zipfile.ZipFile(repaired) as archive:
                repaired_catalog = ET.fromstring(archive.read("catalog.xml"))
            external_edges = {node.attrib.get("id") for node in repaired_catalog.iter()
                              if node.tag.split("}")[-1] == "dep"}
            self.assertTrue({"flash.text:TextField", "flash.utils:Timer"} <= external_edges)
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
            recompiled, relinked = root / "recompiled.swf", root / "recompiled-linked.swf"
            snapshot = root / "recompiled-snapshot"
            result = tools.run("compile", accepted_file, recompiled, "Probe", source, snapshot,
                               "flex-strict", sdk, log=root / "recompile.log")
            self.assertEqual(result.returncode, 0, (root / "recompile.log").read_text())
            result = tools.run("link-layout", accepted_file, recompiled, relinked, "Probe", log=root / "recompile-link.log")
            self.assertEqual(result.returncode, 0, (root / "recompile-link.log").read_text())
            self.assertEqual(movie_source.uncompressed(accepted_file), movie_source.uncompressed(relinked))
            self.assertTrue((root / "recompiled-snapshot-flex/native-compiler.swf").is_file())
            reordered = root / "reordered.swf"
            subprocess.run([config["java"], "-cp", str(root) + os.pathsep + str(root / "bridge") + os.pathsep + config["ffdec"], "WildcardFixture",
                            str(accepted_file), str(reordered), "reorder"], check=True, timeout=60)
            restored = root / "restored.swf"
            self.assertNotEqual(movie_source.uncompressed(accepted_file), movie_source.uncompressed(reordered))
            tools.run("link-layout", accepted_file, reordered, restored, "Probe", log=root / "reorder.log").check_returncode()
            self.assertEqual(movie_source.uncompressed(accepted_file), movie_source.uncompressed(restored))

            def mutate(source, destination, mode, *extra):
                subprocess.run([config["java"], "-cp", str(root) + os.pathsep + str(root / "bridge") + os.pathsep + config["ffdec"], "WildcardFixture",
                                str(source), str(destination), mode, *map(str, extra)], check=True, timeout=60)

            combined = root / "combined.swf"
            retained_source = root / "Retained.as"
            retained_source.write_text("package { public class Retained { public function untouched():int { return 41; } } }")
            retained_movie = root / "retained.swf"
            result = compiler("mxmlc.jar", ["-library-path=", "-source-path=" + str(root),
                              "-output=" + str(retained_movie), str(retained_source)])
            self.assertEqual(result.returncode, 0, result.stdout)
            mutate(accepted_file, combined, "merge-scripts", retained_movie)
            combined_output, combined_linked = root / "combined-output.swf", root / "combined-linked.swf"
            result = tools.run("compile", combined, combined_output, "Probe", source, root / "combined-snapshot",
                               "flex-strict", sdk, log=root / "combined-compile.log")
            self.assertEqual(result.returncode, 0, (root / "combined-compile.log").read_text())
            result = tools.run("link-layout", combined, combined_output, combined_linked, "Probe", log=root / "combined-link.log")
            self.assertEqual(result.returncode, 0, (root / "combined-link.log").read_text())
            self.assertEqual(movie_source.uncompressed(combined), movie_source.uncompressed(combined_linked))

            # A real source feature may add constants and edit a selected body,
            # while every other method/class and all old pool entries stay exact.
            feature_source = root / "feature-source.as"
            feature_source.write_text(source.read_text().replace('new Error("fixture")', 'new Error("source feature")'))
            feature_compiled, feature_linked = root / "feature-compiled.swf", root / "feature-linked.swf"
            result = tools.run("compile", combined, feature_compiled, "Probe", feature_source, root / "feature-snapshot",
                               "flex-strict", sdk, log=root / "feature-compile.log")
            self.assertEqual(result.returncode, 0, (root / "feature-compile.log").read_text())
            owners = [owner for _, owner in movie_source.methods(root / "feature-snapshot")
                      if '"Probe")/instance/1/' in owner and '"branches")' in owner]
            self.assertEqual(len(owners), 1)
            permission = root / "feature-permissions.tsv"
            permission.write_text(owners[0] + "\t-\t-\n")
            result = tools.run("link-feature", combined, feature_compiled, feature_linked, "Probe", permission,
                               log=root / "feature-link.log")
            self.assertEqual(result.returncode, 0, (root / "feature-link.log").read_text())
            self.assertNotEqual(movie_source.uncompressed(combined), movie_source.uncompressed(feature_linked))
            for mode in ("code", "metadata", "pool", "other-class"):
                corrupt = root / ("feature-" + mode + ".swf")
                denied = root / ("feature-denied-" + mode + ".swf")
                mutate(feature_compiled, corrupt, mode)
                result = tools.run("link-feature", combined, corrupt, denied, "Probe", permission,
                                   log=root / ("feature-denied-" + mode + ".log"))
                self.assertNotEqual(result.returncode, 0, mode)
                self.assertFalse(denied.exists())
            # Removing construction changes a protected call, even in an
            # authorized method. It needs the exact reviewed before/after pair.
            feature_source.write_text(feature_source.read_text().replace('new Error("source feature")', 'Error("source feature")'))
            changed_calls = root / "feature-calls.swf"
            result = tools.run("compile", combined, changed_calls, "Probe", feature_source, root / "feature-calls-snapshot",
                               "flex-strict", sdk, log=root / "feature-calls-compile.log")
            self.assertEqual(result.returncode, 0, (root / "feature-calls-compile.log").read_text())
            denied = root / "feature-calls-denied.swf"
            result = tools.run("link-feature", combined, changed_calls, denied, "Probe", permission,
                               log=root / "feature-calls-denied.log")
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(denied.exists())
            fingerprints = re.search(r'([0-9a-f]{64}) -> ([0-9a-f]{64})', (root / "feature-calls-denied.log").read_text())
            self.assertIsNotNone(fingerprints, (root / "feature-calls-denied.log").read_text())
            permission.write_text(owners[0] + "\t" + fingerprints[1] + "\t" + fingerprints[2] + "\n")
            result = tools.run("link-feature", combined, changed_calls, root / "feature-calls-reviewed.swf", "Probe", permission,
                               log=root / "feature-calls-reviewed.log")
            self.assertEqual(result.returncode, 0, (root / "feature-calls-reviewed.log").read_text())
            permission.write_text(owners[0] + "\t" + fingerprints[1] + "\t" + "0" * 64 + "\n")
            result = tools.run("link-feature", combined, changed_calls, root / "feature-calls-wrong-review.swf", "Probe", permission,
                               log=root / "feature-calls-wrong-review.log")
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "feature-calls-wrong-review.swf").exists())
            feature_source.write_text(source.read_text().replace('return target[key];', 'return target.new_feature_property;'))
            dynamic_compiled, dynamic_linked = root / "feature-dynamic.swf", root / "feature-dynamic-linked.swf"
            result = tools.run("compile", combined, dynamic_compiled, "Probe", feature_source, root / "feature-dynamic-snapshot",
                               "flex-strict", sdk, log=root / "feature-dynamic-compile.log")
            self.assertEqual(result.returncode, 0, (root / "feature-dynamic-compile.log").read_text())
            inspect_owners = [owner for _, owner in movie_source.methods(root / "feature-dynamic-snapshot")
                              if '"Probe")/instance/1/' in owner and '"inspect")' in owner]
            self.assertEqual(len(inspect_owners), 1)
            permission.write_text(inspect_owners[0] + "\t-\t-\n")
            result = tools.run("link-feature", combined, dynamic_compiled, dynamic_linked, "Probe", permission,
                               log=root / "feature-dynamic-link.log")
            self.assertEqual(result.returncode, 0, (root / "feature-dynamic-link.log").read_text())
            mutate(dynamic_linked, root / "feature-pool-checked.swf", "feature-pool", combined)

            # Retain the same pool in both files, then exercise Flash's binding
            # emission and implicit script slot against real compiler code.
            prepared, binding = root / "prepared.swf", root / "binding.swf"
            mutate(accepted_file, prepared, "prepare")
            closures, closures_reordered, closures_linked = root / "closures.swf", root / "closures-reordered.swf", root / "closures-linked.swf"
            mutate(prepared, closures, "closures-reference")
            mutate(closures, closures_reordered, "closures-reorder")
            result=tools.run("link-layout", closures, closures_reordered, closures_linked, "Probe", log=root / "closures.log")
            self.assertEqual(result.returncode,0,(root / "closures.log").read_text())
            self.assertEqual(movie_source.uncompressed(closures),movie_source.uncompressed(closures_linked))
            for mode in ("target","code"):
                corrupted,destination=root / (mode+"-closure.swf"),root / (mode+"-closure-linked.swf")
                mutate(closures_reordered,corrupted,"closures-"+mode)
                result=tools.run("link-layout",closures,corrupted,destination,"Probe",log=root / (mode+"-closure.log"))
                self.assertNotEqual(result.returncode,0,mode)
                self.assertFalse(destination.exists(),mode)
            traits, traits_linked = root / "traits.swf", root / "traits-linked.swf"
            mutate(prepared, traits, "traits")
            tools.run("link-layout", prepared, traits, traits_linked, "Probe", log=root / "traits.log").check_returncode()
            self.assertEqual(movie_source.uncompressed(prepared), movie_source.uncompressed(traits_linked))
            for mode in ("relocate", "wide", "owner", "ambiguous", "member"):
                candidate, destination = root / (mode + "-namespace.swf"), root / (mode + "-namespace-linked.swf")
                mutate(prepared, candidate, "namespace-" + mode)
                result = tools.run("link-layout", prepared, candidate, destination, "Probe", log=root / (mode + "-namespace.log"))
                if mode in ("relocate", "wide"):
                    result.check_returncode()
                    self.assertEqual(movie_source.uncompressed(prepared), movie_source.uncompressed(destination))
                else:
                    self.assertNotEqual(result.returncode, 0, mode)
                    self.assertFalse(destination.exists(), mode)
            for mode in ("target", "invalid", "handler"):
                candidate, destination = root / (mode + "-control.swf"), root / (mode + "-control-linked.swf")
                mutate(root / "wide-namespace.swf", candidate, "control-" + mode)
                log = root / (mode + "-control.log")
                result = tools.run("link-layout", prepared, candidate, destination, "Probe", log=log)
                self.assertNotEqual(result.returncode, 0, mode)
                self.assertFalse(destination.exists(), mode)
                if mode == "invalid":
                    self.assertIn("not an instruction boundary", log.read_text())
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
            flow, flow_linked = root / "debug-flow.swf", root / "debug-flow-linked.swf"
            mutate(prepared, flow, "debug-flow")
            result=tools.run("link-layout",flow,prepared,flow_linked,"Probe",log=root / "debug-flow.log")
            self.assertEqual(result.returncode,0,(root / "debug-flow.log").read_text())
            self.assertEqual(movie_source.uncompressed(flow),movie_source.uncompressed(flow_linked))
            for mode in ("target","handler"):
                corrupted,destination=root / (mode+"-debug-flow.swf"),root / (mode+"-debug-flow-linked.swf")
                mutate(prepared,corrupted,"control-"+mode)
                result=tools.run("link-layout",flow,corrupted,destination,"Probe",log=root / (mode+"-debug-flow.log"))
                self.assertNotEqual(result.returncode,0,mode)
                self.assertFalse(destination.exists(),mode)
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
                subprocess.run([config["java"], "-cp", str(root) + os.pathsep + str(root / "bridge") + os.pathsep + config["ffdec"], "WildcardFixture",
                                str(accepted_file), str(corrupted), mutation], check=True, timeout=60)
                destination = root / (mutation + "-linked.swf")
                rejected = tools.run("link-layout", accepted_file, corrupted, destination, "Probe", log=root / (mutation + ".log"))
                self.assertNotEqual(rejected.returncode, 0)
                self.assertFalse(destination.exists(), "Rejected compilation must never produce a linked movie")


if __name__ == "__main__":
    unittest.main()

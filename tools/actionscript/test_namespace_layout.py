"""Private namespace identities must not be invented for executable bindings."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source


@unittest.skipUnless(os.environ.get("SRP_AS_TOOLS"), "set SRP_AS_TOOLS for Java integration")
class NamespaceLayoutIntegration(unittest.TestCase):
    def test_unused_namespace_and_used_namespace_rejection(self):
        with tempfile.TemporaryDirectory(prefix="srp-namespace-test-") as temp:
            root = Path(temp)
            tools = movie_source.Toolchain(Path(os.environ["SRP_AS_TOOLS"]), root / "bridge")
            fixture = root / "NamespaceFixture.java"
            fixture.write_text("""
                import com.jpexs.decompiler.flash.abc.*;
                import com.jpexs.decompiler.flash.abc.types.*;
                import com.jpexs.decompiler.flash.abc.types.traits.*;
                import java.io.IOException;
                public class NamespaceFixture {
                    static int namespace(ABC abc, int kind, String name) {
                        var ns=new Namespace(); ns.kind=kind;
                        ns.name_index=abc.constants.getStringId(name,true);
                        return abc.constants.addNamespace(ns);
                    }
                    static ABC base() {
                        var abc=new ABC(null);
                        int pkg=namespace(abc,0x16,"fixture");
                        var name=new Multiname(); name.kind=7; name.namespace_index=pkg;
                        name.name_index=abc.constants.getStringId("Probe",true);
                        var instance=new InstanceInfo();
                        instance.name_index=abc.constants.addMultiname(name);
                        instance.instance_traits=new Traits();
                        abc.instance_info.add(instance);
                        var cls=new ClassInfo(); cls.static_traits=new Traits();
                        abc.class_info.add(cls);
                        return abc;
                    }
                    static int qname(ABC abc,String pkg,String value) {
                        var name=new Multiname();name.kind=7;
                        name.namespace_index=namespace(abc,0x16,pkg);
                        name.name_index=abc.constants.getStringId(value,true);
                        return abc.constants.addMultiname(name);
                    }
                    static ABC types() {
                        var abc=base();
                        int vector=qname(abc,"__AS3__.vec","Vector"),element=qname(abc,"","String");
                        var type=new Multiname();type.kind=0x1d;type.qname_index=vector;type.params=new int[]{element};
                        int first=abc.constants.addMultiname(type);
                        var alias=new Multiname();alias.kind=0x1d;alias.qname_index=vector;alias.params=new int[]{element};
                        abc.constants.addMultiname(alias);
                        var field=new TraitSlotConst();field.name_index=qname(abc,"","items");field.type_index=first;
                        abc.instance_info.get(0).instance_traits.traits.add(field);
                        return abc;
                    }
                    static void typeAliases()throws Exception {
                        for(String mode:new String[]{"valid","changed-type","changed-field","changed-kind","duplicate"}) {
                            var before=types();var after=types();
                            var oldTraits=before.instance_info.get(0).instance_traits;
                            var newTraits=after.instance_info.get(0).instance_traits;
                            var old=(TraitSlotConst)oldTraits.traits.get(0);
                            var field=(TraitSlotConst)newTraits.traits.get(0);
                            field.type_index=old.type_index+1;
                            if(mode.equals("changed-type"))
                                after.constants.getMultiname(field.type_index).params=new int[]{qname(after,"","Boolean")};
                            if(mode.equals("changed-field"))field.name_index=qname(after,"","other");
                            if(mode.equals("changed-kind"))field.kindType=6;
                            if(mode.equals("duplicate"))oldTraits.traits.add(old);
                            int emitted=field.type_index;
                            MovieSource.restoreTraitTypeAliases(before,after,oldTraits,newTraits,java.util.Map.of());
                            if(field.type_index!=(mode.equals("valid")?old.type_index:emitted))
                                throw new AssertionError("Field type alias guard: "+mode);
                        }
                    }
                    public static void main(String[] args)throws Exception {
                        typeAliases();
                        var zero=base();
                        var unnamed=new Namespace(); unnamed.kind=5; unnamed.name_index=0;
                        int unnamedId=zero.constants.addNamespace(unnamed);
                        if(!MovieSource.namespaceText(zero,unnamed).isEmpty()
                            || MovieSource.privateRole(zero,unnamedId,"fixture:Probe","Probe")!=null)
                            throw new AssertionError("String-pool zero is the empty namespace name");
                        for(String mode:new String[]{"unused","qname","set","protected","duplicate"}) {
                            var before=base(); var after=base();
                            int ns=namespace(after,5,"fixture:Probe");
                            if(mode.equals("qname")) {
                                var name=new Multiname();name.kind=7;name.namespace_index=ns;
                                name.name_index=after.constants.getStringId("secret",true);
                                after.constants.addMultiname(name);
                            }
                            if(mode.equals("set"))
                                after.constants.addNamespaceSet(new NamespaceSet(new int[]{1,ns}));
                            if(mode.equals("protected"))after.instance_info.get(0).protectedNS=ns;
                            if(mode.equals("duplicate")) {
                                namespace(before,5,"fixture:Probe");
                                namespace(before,5,"fixture:Probe");
                                namespace(after,5,"fixture:Probe");
                                namespace(after,5,"fixture:Probe");
                                namespace(after,5,"fixture:Probe");
                            }
                            try {
                                var mapping=MovieSource.namespaceLayout(before,after);
                                if(!mode.equals("unused"))throw new AssertionError("Accepted "+mode);
                                if(mapping.containsKey(ns))throw new AssertionError("Invented identity");
                            } catch(IOException failure) {
                                if(mode.equals("unused"))throw failure;
                            }
                        }
                        System.exit(0);
                    }
                }
            """)
            subprocess.run([tools.config["javac"], "-cp", tools.cp, "-d", str(tools.work),
                            str(fixture)], check=True)
            result = subprocess.run([tools.config["java"], "-cp", tools.cp, "NamespaceFixture"],
                                    capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

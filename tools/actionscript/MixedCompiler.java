import java.nio.file.*;
import java.util.*;
import java.io.*;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.abc.ABC;
import com.jpexs.decompiler.flash.abc.avm2.AVM2Code;
import com.jpexs.decompiler.flash.abc.types.*;
import com.jpexs.decompiler.flash.abc.types.traits.*;
import com.jpexs.decompiler.flash.tags.Tag;
/** Compose explicitly selected records from two actual compilations of the same source. */
public final class MixedCompiler {
    static List<String> hiddenStaticMembers(List<String> plan) throws IOException {
        List<String> result = new ArrayList<>();
        String identifier = "[A-Za-z_$][A-Za-z0-9_$]*";
        for (String line : plan) if (line.startsWith("ffdec_hide_static=")) {
            String member = line.substring("ffdec_hide_static=".length());
            if (!member.matches(identifier + "(?:\\." + identifier + ")*/" + identifier)
                    || result.contains(member))
                throw new IOException("Invalid hidden static dependency declaration");
            result.add(member);
        }
        if (!result.isEmpty() && !plan.contains("ffdec_compiler=ffdec-source"))
            throw new IOException("Hidden declarations require ffdec-source");
        return result;
    }
    static final List<String> POOLS=List.of("Int","UInt","Double","String","Namespace","NamespaceSet","Multiname");
    static int count(ABC abc,String pool)throws Exception {
        return (int)abc.constants.getClass().getMethod("get"+pool+"Count").invoke(abc.constants);
    }
    static void prefix(ABC original,ABC compiled)throws Exception {
        if(original.version.major!=compiled.version.major||original.version.minor!=compiled.version.minor||compiled.hasDecimalSupport()||compiled.hasFloatSupport()||compiled.hasFloat4Support())throw new IOException("Unsupported ABC version");
        for(String pool:POOLS) {
            int n=count(original,pool);
            if(count(compiled,pool)<n)throw new IOException("Compacted "+pool);
            for(int i=1; i<n; i++)if(!Objects.equals(MovieSource.poolEntry(original,pool,i),MovieSource.poolEntry(compiled,pool,i)))throw new IOException("Changed pool prefix "+pool+" "+i);
        }
        if(compiled.metadata_info.size()<original.metadata_info.size())throw new IOException("Compacted metadata");
        for(int i=0; i<original.metadata_info.size(); i++) {
            var a=original.metadata_info.get(i);
            var b=compiled.metadata_info.get(i);
            if(a.name_index!=b.name_index||!Arrays.equals(a.keys,b.keys)||!Arrays.equals(a.values,b.values))throw new IOException("Metadata prefix changed");
        }
    }
    static class PoolImport {
        final ABC original,source,dest;
        final Map<Integer,Integer> namespaces;
        final Map<String,Map<Integer,Integer>> maps=new HashMap<>();
        final Set<String> visiting=new HashSet<>();
        PoolImport(ABC original,ABC source,ABC dest,Map<Integer,Integer> namespaces) {
            this.original=original;
            this.source=source;
            this.dest=dest;
            this.namespaces=namespaces;
        }
        int index(String pool,int id)throws Exception {
            if(id<0||id>=count(source,pool))throw new IOException("Invalid "+pool+" index "+id);
            if(id<count(original,pool))return id;
            if(pool.equals("Namespace")&&namespaces.containsKey(id))return namespaces.get(id);
            var map=maps.computeIfAbsent(pool,k->new HashMap<>());
            if(map.containsKey(id))return map.get(id);
            String key=pool+":"+id;
            if(!visiting.add(key))throw new IOException("Cyclic constant reference");
            int result;
            switch(pool) {
                case "Int":result=dest.constants.getIntId(source.constants.getInt(id),true);
                break;
                case "UInt":result=dest.constants.getUIntId(source.constants.getUInt(id),true);
                break;
                case "Double": {
                    double value=source.constants.getDouble(id);
                    long bits=Double.doubleToRawLongBits(value);
                    result=-1;
                    for(int i=1; i<dest.constants.getDoubleCount(); i++)if(Double.doubleToRawLongBits(dest.constants.getDouble(i))==bits) {
                        result=i;
                        break;
                    }
                    if(result<0)result=dest.constants.addDouble(value);
                    break;
                }
                case "String":result=dest.constants.getStringId(source.constants.getString(id),true);
                break;
                case "Namespace": {
                    var value=source.constants.getNamespace(id);
                    if(value.kind==5)throw new IOException("Unmapped new private namespace");
                    int name=index("String",value.name_index);
                    result=-1;
                    for(int i=1; i<dest.constants.getNamespaceCount(); i++) {
                        var existing=dest.constants.getNamespace(i);
                        if(existing.kind==value.kind&&existing.name_index==name) {
                            result=i;
                            break;
                        }
                    }
                    if(result<0)result=dest.constants.addNamespace(new Namespace(value.kind,name));
                    break;
                }
                case "NamespaceSet": {
                    int[] values=source.constants.getNamespaceSet(id).namespaces.clone();
                    for(int i=0; i<values.length; i++)values[i]=index("Namespace",values[i]);
                    result=dest.constants.getNamespaceSetId(values,true);
                    break;
                }
                case "Multiname": {
                    var value=source.constants.getMultiname(id);
                    var copy=new Multiname();
                    copy.kind=value.kind;
                    copy.name_index=value.hasOwnName()?index("String",value.name_index):value.name_index;
                    copy.namespace_index=value.hasOwnNamespace()?index("Namespace",value.namespace_index):value.namespace_index;
                    copy.namespace_set_index=value.hasOwnNamespaceSet()?index("NamespaceSet",value.namespace_set_index):value.namespace_set_index;
                    if(value.kind==0x1d) {
                        copy.qname_index=index("Multiname",value.qname_index);
                        copy.params=value.params.clone();
                        for(int i=0; i<copy.params.length; i++)copy.params[i]=index("Multiname",copy.params[i]);
                    }
                    result=dest.constants.getMultinameId(copy,true);
                    break;
                }
                default:throw new IOException("Unsupported pool "+pool);
            }
            visiting.remove(key);
            map.put(id,result);
            return result;
        }
        int optional(int kind,int id)throws Exception {
            if(id==0)return 0;
            String pool=switch(kind) {
                case 1->"String";
                case 3->"Int";
                case 4->"UInt";
                case 6->"Double";
                case 5,8,0x16,0x17,0x18,0x19,0x1a->"Namespace";
                case 0,0x0a,0x0b,0x0c->null;
                default->throw new IOException("Unsupported value kind "+kind);
            };
            return pool==null?id:index(pool,id);
        }
        int metadata(int id)throws Exception {
            if(id<0||id>=source.metadata_info.size())throw new IOException("Invalid metadata reference");
            if(id<original.metadata_info.size())return id;
            var value=source.metadata_info.get(id);
            int[] keys=value.keys.clone(),values=value.values.clone();
            for(int i=0; i<keys.length; i++)keys[i]=index("String",keys[i]);
            for(int i=0; i<values.length; i++)values[i]=index("String",values[i]);
            int name=index("String",value.name_index);
            for(int i=0; i<dest.metadata_info.size(); i++) {
                var entry=dest.metadata_info.get(i);
                if(entry.name_index==name&&Arrays.equals(entry.keys,keys)&&Arrays.equals(entry.values,values))return i;
            }
            dest.metadata_info.add(new MetadataInfo(name,keys,values));
            return dest.metadata_info.size()-1;
        }
        void traits(Traits values)throws Exception {
            for(var trait:values.traits) {
                trait.name_index=index("Multiname",trait.name_index);
                if(trait.metadata!=null) {
                    trait.metadata=trait.metadata.clone();
                    for(int i=0; i<trait.metadata.length; i++)trait.metadata[i]=metadata(trait.metadata[i]);
                }
                if(trait instanceof TraitSlotConst slot) {
                    slot.type_index=index("Multiname",slot.type_index);
                    slot.value_index=optional(slot.value_kind,slot.value_index);
                }
                else if(!(trait instanceof TraitClass)&&!(trait instanceof TraitMethodGetterSetter)&&!(trait instanceof TraitFunction))throw new IOException("Unknown trait record");
            }
        }
        void method(MethodInfo value)throws Exception {
            value.ret_type=index("Multiname",value.ret_type);
            for(int i=0; i<value.param_types.length; i++)value.param_types[i]=index("Multiname",value.param_types[i]);
            value.name_index=index("String",value.name_index);
            for(var option:value.optional)option.value_index=optional(option.value_kind,option.value_index);
            for(int i=0; i<value.paramNames.length; i++)value.paramNames[i]=index("String",value.paramNames[i]);
        }
        void body(MethodBody value)throws Exception {
            traits(value.traits);
            for(var handler:value.exceptions) {
                handler.name_index=index("Multiname",handler.name_index);
                handler.type_index=index("Multiname",handler.type_index);
            }
            MovieSource.relocateCode(value,instruction-> {
                try {
                    for(int i=0; i<instruction.definition.operands.length; i++) {
                        int type=instruction.definition.operands[i];
                        String pool=type==AVM2Code.DAT_MULTINAME_INDEX?"Multiname":type==AVM2Code.DAT_NAMESPACE_INDEX?"Namespace":type==AVM2Code.DAT_STRING_INDEX?"String":type==AVM2Code.DAT_INT_INDEX?"Int":type==AVM2Code.DAT_UINT_INDEX?"UInt":type==AVM2Code.DAT_DOUBLE_INDEX?"Double":null;
                        if(pool!=null)instruction.operands[i]=index(pool,instruction.operands[i]);
                    }
                }
                catch(IOException e) {
                    throw e;
                }
                catch(Exception e) {
                    throw new IOException(e);
                }
            });
            value.setAbc(dest);
        }
    }
    static Map<Integer,Integer> methodNamespaces(ABC original,ABC source,MethodBody body)throws Exception {
        Map<Integer,List<Integer>> alternatives=new HashMap<>();
        Map<Integer,Integer> result=MovieSource.namespaceLayout(original,source,alternatives);
        var used=MovieSource.bodyNamespaces(source,body);
        var old=original.findBody(body.method_info);
        if(old==null)throw new IOException("Missing corresponding reference owner");
        var oldUsed=MovieSource.bodyNamespaces(original,old);
        for(var item:alternatives.entrySet())if(used.contains(item.getKey())) {
            var candidates=item.getValue().stream().filter(oldUsed::contains).toList();
            if(candidates.size()!=1)throw new IOException("Ambiguous method file namespace");
            result.put(item.getKey(),candidates.get(0));
        }
        return result;
    }
    static void compactAppendedPools(SWF movie,ABC original,ABC compiled)throws Exception {
        // Reparse actual composed compiler records; preserve every original pool entry, importing only live appended references.
        ABC source=new ABC(new com.jpexs.decompiler.flash.abc.ABCInputStream(new com.jpexs.helpers.MemoryInputStream(MovieSource.abcBytes(compiled))),movie,compiled.parentTag);
        for(var body:source.bodies)MovieSource.exactCode(body);
        Map<Integer,List<Integer>> alternatives=new HashMap<>();
        var base=MovieSource.namespaceLayout(original,source,alternatives);
        Map<Integer,Map<Integer,Integer>> perMethod=new HashMap<>();
        for(var body:source.bodies)perMethod.put(body.method_info,methodNamespaces(original,source,body));
        compiled.constants=original.constants.clone();
        compiled.metadata_info=new ArrayList<>(source.metadata_info.subList(0,original.metadata_info.size()));
        var imports=new PoolImport(original,source,compiled,base);
        for(var instance:compiled.instance_info) {
            instance.name_index=imports.index("Multiname",instance.name_index);
            instance.super_index=imports.index("Multiname",instance.super_index);
            instance.protectedNS=imports.index("Namespace",instance.protectedNS);
            for(int i=0; i<instance.interfaces.length; i++)instance.interfaces[i]=imports.index("Multiname",instance.interfaces[i]);
            imports.traits(instance.instance_traits);
        }
        for(var entry:compiled.class_info)imports.traits(entry.static_traits);
        for(var entry:compiled.script_info)imports.traits(entry.traits);
        for(var method:compiled.method_info)imports.method(method);
        for(var body:compiled.bodies) {
            MovieSource.exactCode(body);
            new PoolImport(original,source,compiled,perMethod.get(body.method_info)).body(body);
        }
    }
    public static void compose(String[] args)throws Exception {
        if(args.length!=6)throw new IOException("Expected reference, native output, FFDec output, class, selection and output");
        var reference=MovieSource.read(Path.of(args[0]));
        var primary=MovieSource.read(Path.of(args[1]));
        var secondary=MovieSource.read(Path.of(args[2]));
        String cls=args[3];
        Path selection=Path.of(args[4]),output=Path.of(args[5]);
        List<String> selected=Files.readAllLines(selection);
        if(selected.isEmpty()||selected.stream().anyMatch(String::isBlank)||new HashSet<>(selected).size()!=selected.size())throw new IOException("Empty or duplicate selection");
        hiddenStaticMembers(selected);
        selected.removeIf(line -> line.startsWith("ffdec_hide_static="));
        boolean secondaryScript=selected.remove("script_traits=ffdec");
        selected.remove("ffdec_compiler=ffdec-source");
        if(selected.isEmpty())throw new IOException("No selected methods");
        var script=MovieSource.findScript(reference,cls);
        int home=-1;
        for(int i=0; i<reference.getAbcList().size(); i++)if(reference.getAbcList().get(i).getABC()==script.abc)home=i;
        if(home<0)throw new IOException("Missing script");
        for(var candidate:List.of(primary,secondary)) {
            if(candidate.getAbcList().size()!=reference.getAbcList().size())throw new IOException("ABC count changed");
            for(int i=0; i<reference.getAbcList().size(); i++)if(i!=home&&!Arrays.equals(MovieSource.abcBytes(reference.getAbcList().get(i).getABC()),MovieSource.abcBytes(candidate.getAbcList().get(i).getABC())))throw new IOException("Unrelated ABC changed");
        }
        ABC original=reference.getAbcList().get(home).getABC(),a=primary.getAbcList().get(home).getABC(),b=secondary.getAbcList().get(home).getABC();
        for(var abc:List.of(a,b)) {
            prefix(original,abc);
            for(var body:abc.bodies)MovieSource.exactCode(body);
            MovieSource.restoreNameReferences(original,abc);
            MovieSource.restoreMethodOrder(original,abc);
            if(abc.instance_info.size()!=original.instance_info.size()||abc.script_info.size()!=original.script_info.size())throw new IOException("Declaration counts changed");
            for(int i=0; i<abc.instance_info.size(); i++)if(abc.instance_info.get(i).name_index!=original.instance_info.get(i).name_index)throw new IOException("Class identities changed");
        }
        var owners=MovieSource.names(b);
        var primaryOwners=MovieSource.names(a);
        Set<Integer> methods=new TreeSet<>();
        for(String requested:selected) {
            var found=owners.entrySet().stream().filter(e->e.getValue().equals(requested)).toList();
            if(found.size()!=1)throw new IOException("Unknown or ambiguous method owner: "+requested);
            for(var entry:owners.entrySet())if(entry.getValue().equals(requested)||entry.getValue().startsWith(requested+"/function[")) {
                if(!entry.getValue().equals(primaryOwners.get(entry.getKey())))throw new IOException("Compiler method owners disagree");
                methods.add(entry.getKey());
            }
        }
        for(int id:methods) {
            var body=b.findBody(id);
            if(body==null)throw new IOException("Selected method has no body");
            for(var instruction:body.getCode().code)if(instruction.definition.instructionCode==0x40&&!methods.contains(instruction.operands[0]))throw new IOException("Unselected nested function");
        }
        Map<Integer,Map<Integer,Integer>> namespaceMaps=new HashMap<>();
        for(int id:methods)namespaceMaps.put(id,methodNamespaces(original,b,b.findBody(id)));
        Map<Integer,Integer> declarationNamespaces=MovieSource.namespaceLayout(original,b,new HashMap<>());
        // Normalize and import each selected compiler record. Only reference identities and pool prefixes are used.
        for(int id:methods) {
            var body=b.findBody(id);
            var imports=new PoolImport(original,b,a,namespaceMaps.get(id));
            imports.method(b.method_info.get(id));
            imports.body(body);
            int position=-1;
            for(int i=0; i<a.bodies.size(); i++)if(a.bodies.get(i).method_info==id)position=i;
            if(position<0)throw new IOException("Primary body missing");
            a.bodies.set(position,body);
            a.method_info.set(id,b.method_info.get(id));
        }
        if(secondaryScript) {
            if(a.script_info.size()!=1||a.script_info.get(0).traits.traits.size()!=1||b.script_info.get(0).traits.traits.size()!=1)throw new IOException("Script selection requires sole class trait");
            var x=a.script_info.get(0).traits.traits.get(0);
            var y=b.script_info.get(0).traits.traits.get(0);
            if(!(x instanceof TraitClass p)||!(y instanceof TraitClass q)||p.name_index!=q.name_index||p.class_info!=q.class_info)throw new IOException("Script declarations disagree");
            var imports=new PoolImport(original,b,a,declarationNamespaces);
            imports.traits(b.script_info.get(0).traits);
            a.script_info.get(0).traits=b.script_info.get(0).traits;
        }
        compactAppendedPools(primary,original,a);
        byte[] expected=MovieSource.abcBytes(a);
        ((Tag)a.parentTag).setModified(true);
        try(var out=Files.newOutputStream(output,StandardOpenOption.CREATE_NEW)) {
            primary.saveTo(out);
        }
        var saved=MovieSource.read(output);
        if(!Arrays.equals(expected,MovieSource.abcBytes(saved.getAbcList().get(home).getABC())))throw new IOException("Serialized composition differs");
        System.out.println("Composed "+methods.size()+" actual secondary compiler method records; source declaration selection="+secondaryScript);
    }
}

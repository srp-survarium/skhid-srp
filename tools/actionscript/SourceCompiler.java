import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.LinkedHashMap;
import java.util.Map;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.SourceGeneratorLocalData;
import com.jpexs.decompiler.flash.abc.ScriptPack;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.abc.avm2.parser.AVM2ParseException;
import com.jpexs.decompiler.flash.abc.avm2.parser.script.*;
import com.jpexs.decompiler.flash.abc.types.traits.TraitClass;
import com.jpexs.decompiler.flash.abc.types.traits.Trait;
import com.jpexs.decompiler.flash.abc.types.traits.Traits;
import com.jpexs.decompiler.flash.abc.types.traits.TraitMethodGetterSetter;
import com.jpexs.decompiler.flash.importers.*;
import com.jpexs.decompiler.flash.tags.Tag;
import com.jpexs.decompiler.graph.*;

/** Source-call generation with explicitly selected dependency declarations. */
public final class SourceCompiler extends FFDecAs3ScriptReplacer {
    final List<String> hiddenStaticMembers;

    public SourceCompiler() {
        this(List.of());
    }

    public SourceCompiler(List<String> hiddenStaticMembers) {
        super(false);
        this.hiddenStaticMembers = List.copyOf(hiddenStaticMembers);
    }

    static Object callField(CallAVM2Item call, String name) throws ReflectiveOperationException {
        Field field = CallAVM2Item.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(call);
    }

    static Object propertyField(PropertyAVM2Item property, String name) throws ReflectiveOperationException {
        Field field = PropertyAVM2Item.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(property);
    }

    static final class PublicGlobalCallee extends PropertyAVM2Item {
        PublicGlobalCallee(PropertyAVM2Item property) throws ReflectiveOperationException {
            super(property.object, property.attribute, property.propertyName, property.namespaceSuffix,
                property.abcIndex, List.of(), List.of(), false, "public", 0, null);
            for (Class<?> type = PropertyAVM2Item.class; type != null
                    && GraphTargetItem.class.isAssignableFrom(type); type = type.getSuperclass()) {
                for (Field field : type.getDeclaredFields()) {
                    if (Modifier.isStatic(field.getModifiers())) continue;
                    field.setAccessible(true);
                    field.set(this, field.get(property));
                }
            }
        }

        @Override
        public int resolveProperty(SourceGeneratorLocalData local) throws CompilationException {
            // public::name(...) explicitly selects the public namespace.
            // FFDec's fallback includes every open namespace. Keep the source's
            // qualifier while letting its normal generator emit the real call.
            var pool = abcIndex.getSelectedAbc().constants;
            int namespace = pool.getNamespaceId(0x16, "", 0, true);
            return pool.getMultinameId(com.jpexs.decompiler.flash.abc.types.Multiname.createQName(
                false, pool.getStringId(propertyName, true), namespace), true);
        }
    }

    static final class SourceCall extends CallAVM2Item {
        SourceCall(CallAVM2Item call) throws ReflectiveOperationException {
            super(call.openedNamespaces, call.line, call.name, call.arguments,
                (AbcIndexing)callField(call, "abcIndex"), (TypeItem)callField(call, "thisType"));
            // Keep source positions and all parser metadata as well as the
            // resolved-call context. The toolchain pins the entire FFDec jar.
            for (Class<?> type = CallAVM2Item.class; type != null
                    && GraphTargetItem.class.isAssignableFrom(type); type = type.getSuperclass()) {
                for (Field field : type.getDeclaredFields()) {
                    if (Modifier.isStatic(field.getModifiers())) continue;
                    field.setAccessible(true);
                    field.set(this, field.get(call));
                }
            }
            if (name.getClass() == PropertyAVM2Item.class) {
                var property = (PropertyAVM2Item)name;
                if (property.object == null && !property.attribute
                        && (property.namespaceSuffix == null || property.namespaceSuffix.isEmpty())
                        && "public".equals(propertyField(property, "nsKeyword")))
                    name = new PublicGlobalCallee(property);
            }
        }

        @Override
        public List<GraphSourceItem> toSource(SourceGeneratorLocalData local, SourceGenerator generator,
                boolean needsReturn) throws CompilationException {
            List<GraphSourceItem> output = super.toSource(local, generator, needsReturn);
            // CallAVM2Item resolves the callee before selecting this intrinsic.
            // Its final coerce_s means a call to the built-in String type.
            // String(value) must stringify null/undefined; typed assignment
            // coercions are separate AST nodes and must retain coerce_s.
            if (!output.isEmpty() && output.get(output.size() - 1) instanceof AVM2Instruction last
                    && last.definition.instructionCode == 0x85) {
                output = new ArrayList<>(output);
                output.set(output.size() - 1, new AVM2Instruction(0, 0x70, null));
            }
            return output;
        }
    }

    @SuppressWarnings({"rawtypes", "unchecked"})
    static Object adapt(Object object, IdentityHashMap<Object, Object> visited) throws ReflectiveOperationException {
        if (object == null) return null;
        if (visited.containsKey(object)) return visited.get(object);
        if (object instanceof List list) {
            visited.put(object, object);
            for (int i = 0; i < list.size(); i++) {
                Object previous = list.get(i), next = adapt(previous, visited);
                if (previous != next) list.set(i, next);
            }
            return object;
        }
        if (!(object instanceof GraphTargetItem)) return object;
        // Constructor nodes subclass CallAVM2Item. They must never be replaced
        // with ordinary calls, even when the constructed type is String.
        Object result = object.getClass() == CallAVM2Item.class ? new SourceCall((CallAVM2Item)object) : object;
        visited.put(object, result);
        for (Class<?> type = result.getClass(); type != null
                && GraphTargetItem.class.isAssignableFrom(type); type = type.getSuperclass()) {
            for (Field field : type.getDeclaredFields()) {
                if (Modifier.isStatic(field.getModifiers())) continue;
                field.setAccessible(true);
                Object previous = field.get(result), next = adapt(previous, visited);
                if (previous != next) field.set(result, next);
            }
        }
        return result;
    }

    static final class Parser extends ActionScript3Parser {
        Parser(AbcIndexing index) throws IOException, InterruptedException {
            super(index);
        }

        @Override
        public void addScriptFromTree(List<AssignableAVM2Item> variables, boolean activation, List<DottedChain> imports,
                List<NamespaceItem> namespaces, List<List<NamespaceItem>> allNamespaces, List<GraphTargetItem> tree,
                int position, String documentClass, Integer numberContext, DottedChain pkg)
                throws AVM2ParseException, CompilationException {
            try {
                adapt(tree, new IdentityHashMap<>());
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException("Unsupported pinned compiler AST", e);
            }
            super.addScriptFromTree(variables, activation, imports, namespaces, allNamespaces, tree,
                position, documentClass, numberContext, pkg);
        }
    }

    @Override
    public void replaceScript(ScriptPack script, String source, List<SWF> dependencies)
            throws As3ScriptReplaceException, IOException, InterruptedException {
        if (!script.isSimple || !dependencies.isEmpty())
            throw new IOException("Source compiler requires a simple script and embedded dependencies");
        var abc = script.abc;
        var movie = abc.getSwf();
        var old = abc.script_info.get(script.scriptIndex);
        int count = abc.script_info.size(), position = abc.instance_info.size();
        for (int id : script.traitIndices)
            if (old.traits.traits.get(id) instanceof TraitClass cls) position = cls.class_info + 1;
        script.delete(abc, true);
        var index = movie.getAbcIndex();
        index.selectAbc(abc);
        index.refreshAbc(abc);
        Map<Traits, List<Trait>> declarations = new LinkedHashMap<>();
        try {
            // Reproduce an explicitly selected dependency API. Only public
            // static method declarations in other ABCs may be hidden. No
            // executable bodies are changed; every declaration is restored
            // before output, including after compiler failures.
            for (String member : hiddenStaticMembers) {
                int slash = member.lastIndexOf('/');
                String cls = member.substring(0, slash), method = member.substring(slash + 1);
                int matches = 0;
                for (var tag : movie.getAbcList()) {
                    var dependency = tag.getABC();
                    int id = dependency.findClassByName(cls);
                    if (id < 0) continue;
                    if (dependency == abc) throw new IOException("Cannot hide a target ABC declaration");
                    var traits = dependency.class_info.get(id).static_traits;
                    for (var trait : new ArrayList<>(traits.traits)) {
                        var name = dependency.constants.getMultiname(trait.name_index);
                        if (name.kind != 7 || !dependency.constants.getString(name.name_index).equals(method)) continue;
                        var namespace = dependency.constants.getNamespace(name.namespace_index);
                        if (namespace.kind != 0x16 || !dependency.constants.getString(namespace.name_index).isEmpty())
                            continue;
                        if (!(trait instanceof TraitMethodGetterSetter) || trait.kindType != 1)
                            throw new IOException("Hidden declaration is not a static method: " + member);
                        if (!declarations.containsKey(traits)) {
                            declarations.put(traits, traits.traits);
                            traits.traits = new ArrayList<>(traits.traits);
                        }
                        traits.traits.remove(trait);
                        matches++;
                    }
                    index.refreshAbc(dependency);
                }
                if (matches != 1) throw new IOException("Unknown or ambiguous hidden static method: " + member);
            }
            index.selectAbc(abc);
            new Parser(index).addScript(source, script.getPathScriptName() + ".as", position,
                script.scriptIndex, movie.getDocumentClass(), abc);
        } catch (AVM2ParseException e) {
            var failure = new As3ScriptReplaceException(new As3ScriptReplaceExceptionItem(null, e.text, (int)e.line));
            failure.initCause(e);
            throw failure;
        } catch (CompilationException e) {
            var failure = new As3ScriptReplaceException(new As3ScriptReplaceExceptionItem(null, e.text, e.line));
            failure.initCause(e);
            throw failure;
        } finally {
            for (var entry : declarations.entrySet()) entry.getKey().traits = entry.getValue();
            if (!declarations.isEmpty())
                for (var tag : movie.getAbcList()) if (tag.getABC() != abc) index.refreshAbc(tag.getABC());
            index.selectAbc(abc);
        }
        // All work is in the freshly loaded candidate. A parse/generation failure
        // propagates before MovieSource can serialize or link any output.
        abc.script_info.set(script.scriptIndex, abc.script_info.remove(count));
        abc.script_info.get(script.scriptIndex).setModified(true);
        abc.pack();
        ((Tag)abc.parentTag).setModified(true);
    }
}

import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.SourceGeneratorLocalData;
import com.jpexs.decompiler.flash.abc.ScriptPack;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.abc.avm2.parser.AVM2ParseException;
import com.jpexs.decompiler.flash.abc.avm2.parser.script.*;
import com.jpexs.decompiler.flash.abc.types.traits.TraitClass;
import com.jpexs.decompiler.flash.importers.*;
import com.jpexs.decompiler.flash.tags.Tag;
import com.jpexs.decompiler.graph.*;

/** Generate explicit String conversions from the parsed source call expression. */
public final class SourceCompiler extends FFDecAs3ScriptReplacer {
    public SourceCompiler() {
        super(false);
    }

    static Object callField(CallAVM2Item call, String name) throws ReflectiveOperationException {
        Field field = CallAVM2Item.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(call);
    }

    static final class StringCall extends CallAVM2Item {
        StringCall(CallAVM2Item call) throws ReflectiveOperationException {
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
        Object result = object.getClass() == CallAVM2Item.class ? new StringCall((CallAVM2Item)object) : object;
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
        try {
            new Parser(index).addScript(source, script.getPathScriptName() + ".as", position,
                script.scriptIndex, movie.getDocumentClass(), abc);
        } catch (AVM2ParseException e) {
            throw new As3ScriptReplaceException(new As3ScriptReplaceExceptionItem(null, e.text, (int)e.line));
        } catch (CompilationException e) {
            throw new As3ScriptReplaceException(new As3ScriptReplaceExceptionItem(null, e.text, e.line));
        }
        // All work is in the freshly loaded candidate. A parse/generation failure
        // propagates before MovieSource can serialize or link any output.
        abc.script_info.set(script.scriptIndex, abc.script_info.remove(count));
        abc.script_info.get(script.scriptIndex).setModified(true);
        abc.pack();
        ((Tag)abc.parentTag).setModified(true);
    }
}

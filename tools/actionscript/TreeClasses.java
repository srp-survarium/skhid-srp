import java.io.*;
import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.abc.ABC;
import com.jpexs.decompiler.flash.abc.ScriptPack;
import com.jpexs.decompiler.flash.abc.ABCOutputStream;
import com.jpexs.decompiler.flash.abc.avm2.parser.script.ActionScript3Parser;
import com.jpexs.decompiler.flash.abc.types.*;
import com.jpexs.decompiler.flash.abc.types.traits.*;
import com.jpexs.decompiler.flash.tags.Tag;

/**
 * The class- and script-level structure of a linked ABC: an edited class's
 * ABC keeps the original's classes and scripts at their indices (new ones
 * follow), a new class is compiled into the ABC of a class of the release,
 * and a class gone from the source tree is taken out of the movie.
 */
final class TreeClasses {
    private TreeClasses() {}

    static String dotted(ABC abc, int nameIndex) {
        Multiname name = abc.constants.getMultiname(nameIndex);
        String local = name.name_index == 0 ? "*" : abc.constants.getString(name.name_index);
        if (!name.hasOwnNamespace() || name.namespace_index == 0) return local;
        Namespace ns = abc.constants.getNamespace(name.namespace_index);
        String pkg = ns.name_index == 0 ? "" : abc.constants.getString(ns.name_index);
        return pkg.isEmpty() || ns.kind != Namespace.KIND_PACKAGE ? local : pkg + "." + local;
    }

    static Map<String, Integer> classes(ABC abc) throws IOException {
        Map<String, Integer> result = new LinkedHashMap<>();
        for (int i = 0; i < abc.instance_info.size(); i++)
            if (result.put(dotted(abc, abc.instance_info.get(i).name_index), i) != null)
                throw new IOException("Ambiguous class " + dotted(abc, abc.instance_info.get(i).name_index));
        return result;
    }

    /** Original classes keep their indices; new classes follow them. */
    static void restoreClassOrder(ABC original, ABC compiled) throws IOException {
        Map<String, Integer> before = classes(original), after = classes(compiled);
        for (String cls : before.keySet())
            if (!after.containsKey(cls)) throw new IOException("Feature removed class " + cls);
        Map<Integer, Integer> positions = new HashMap<>();
        int appended = original.instance_info.size();
        boolean moved = false;
        for (var entry : after.entrySet()) {
            Integer at = before.get(entry.getKey());
            if (at == null) at = appended++;
            positions.put(entry.getValue(), at);
            moved |= at.intValue() != entry.getValue().intValue();
        }
        if (moved) compiled.reorganizeClasses(positions);
        if (!new ArrayList<>(classes(compiled).keySet()).subList(0, before.size()).equals(new ArrayList<>(before.keySet())))
            throw new IOException("Class order could not be restored");
    }

    static Set<String> scriptNames(ABC abc, ScriptInfo script) {
        Set<String> result = new TreeSet<>();
        for (Trait trait : script.traits.traits) result.add(dotted(abc, trait.name_index));
        return result;
    }

    /** Original scripts keep their indices (each still defines what it did); new scripts follow. */
    static void checkScriptOrder(ABC original, ABC compiled) throws IOException {
        for (int i = 0; i < original.script_info.size(); i++) {
            Set<String> before = scriptNames(original, original.script_info.get(i));
            Set<String> after = scriptNames(compiled, compiled.script_info.get(i));
            if (!after.containsAll(before)) throw new IOException("Script " + i + " no longer defines " + before);
        }
    }

    /**
     * The other classes of the ABC are not compiled: the class compiled into
     * it (`cls`) may change and grow, every record of another class or script
     * of the original stays as it was.
     */
    static void checkOtherClasses(ABC original, ABC compiled, String cls) throws Exception {
        int k = original.findClassByName(cls);
        Set<Integer> own = new HashSet<>();
        for (int s = 0; s < original.script_info.size(); s++)
            for (Trait trait : original.script_info.get(s).traits.traits)
                if (k < 0 || (trait instanceof TraitClass c && c.class_info == k)) own.add(s);
        String classOwner = k < 0 ? null : MovieSource.name(original, original.instance_info.get(k).name_index) + "/";
        Set<String> scriptOwners = new HashSet<>();
        for (int s : own) scriptOwners.add("script:" + original.script_info.get(s).traits.traits.stream()
            .map(t -> MovieSource.name(original, t.name_index)).sorted().toList() + "/");
        Map<Integer, String> owners = MovieSource.names(original);
        java.util.function.Predicate<String> its = owner -> owner == null || Arrays.stream(owner.split("\\|")).anyMatch(
            part -> (classOwner != null && part.startsWith(classOwner)) || scriptOwners.stream().anyMatch(part::startsWith));
        for (int i = 0; i < original.method_info.size(); i++) {
            if (its.test(owners.get(i))) continue;
            MethodBody a = original.findBody(i), b = compiled.findBody(i);
            if (!Arrays.equals(methodInfoBytes(original.method_info.get(i)), methodInfoBytes(compiled.method_info.get(i)))
                    || (a == null) != (b == null)
                    || (a != null && !Arrays.equals(MovieSource.record(original, a), MovieSource.record(compiled, b))))
                throw new IOException("A class that was not compiled changed: " + owners.get(i));
        }
        List<byte[]> before = records(original), after = records(compiled);
        int methods = original.method_info.size(), bodies = original.bodies.size();
        int newMethods = compiled.method_info.size(), newBodies = compiled.bodies.size();
        for (int j = 0; j < original.instance_info.size(); j++)
            if (j != k && !Arrays.equals(before.get(methods + bodies + j), after.get(newMethods + newBodies + j)))
                throw new IOException("A class that was not compiled changed: " + dotted(original, original.instance_info.get(j).name_index));
        int classes = original.instance_info.size(), newClasses = compiled.instance_info.size();
        for (int s = 0; s < original.script_info.size(); s++)
            if (!own.contains(s) && !Arrays.equals(before.get(methods + bodies + classes + s),
                    after.get(newMethods + newBodies + newClasses + s)))
                throw new IOException("A script that was not compiled changed: " + s);
    }

    static byte[] methodInfoBytes(MethodInfo info) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        new ABCOutputStream(bytes).writeMethodInfo(info);
        return bytes.toByteArray();
    }

    /** The package a source declares ("" for the top level). */
    static String sourcePackage(String source) throws IOException {
        var found = java.util.regex.Pattern.compile(
            "(?m)^\\s*package(?:\\s+([A-Za-z_$][\\w$]*(?:\\.[A-Za-z_$][\\w$]*)*))?\\s*(?:\\{|$)").matcher(source);
        if (!found.find()) throw new IOException("The source declares no package");
        return found.group(1) == null ? "" : found.group(1);
    }

    /** Compile a new class (FFDec's compiler) into the ABC of an existing class, after all its records. */
    static void addScript(Path input, Path output, String anchor, Path source) throws Exception {
        String file = source.getFileName().toString();
        if (!file.endsWith(".as")) throw new IOException("New class source must be a .as file");
        String text = Files.readString(source);
        String pkg = sourcePackage(text), simple = file.substring(0, file.length() - 3);
        String cls = pkg.isEmpty() ? simple : pkg + "." + simple;
        SWF swf = MovieSource.read(input);
        for (var tag : swf.getAbcList())
            for (int i = 0; i < tag.getABC().instance_info.size(); i++)
                if (dotted(tag.getABC(), tag.getABC().instance_info.get(i).name_index).equals(cls))
                    throw new IOException("The movie already has a class " + cls);
        ScriptPack home = MovieSource.findScript(swf, anchor);
        ABC abc = home.abc;
        List<byte[]> before = new ArrayList<>();
        for (MethodBody body : abc.bodies) before.add(MovieSource.record(abc, body));
        byte[] definitions = MovieSource.definitions(abc);
        swf.getAbcIndex().selectAbc(abc);
        ActionScript3Parser.compile(text, abc, swf.getAbcIndex(), file,
            abc.instance_info.size(), abc.script_info.size(), false, "");
        for (int i = 0; i < before.size(); i++)
            if (!Arrays.equals(before.get(i), MovieSource.record(abc, abc.bodies.get(i))))
                throw new IOException("Compiling " + cls + " changed an existing method of " + anchor + "'s ABC");
        byte[] now = MovieSource.definitions(abc);
        if (abc.findClassByName(cls) < 0) throw new IOException("The compiler emitted no class " + cls);
        ((Tag) abc.parentTag).setModified(true);
        try (OutputStream out = Files.newOutputStream(output, StandardOpenOption.CREATE_NEW)) { swf.saveTo(out); }
        System.out.println("Added " + cls + " to the ABC of " + anchor + " (" + (now.length - definitions.length)
            + " definition bytes)");
    }

    /** Every record of an ABC but its constant pools, each serialized on its own, in order. */
    static List<byte[]> records(ABC abc) throws Exception {
        List<byte[]> result = new ArrayList<>();
        for (MethodInfo info : abc.method_info) result.add(methodInfoBytes(info));
        for (MethodBody body : abc.bodies) result.add(MovieSource.record(abc, body));
        for (int i = 0; i < abc.instance_info.size(); i++) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            ABCOutputStream out = new ABCOutputStream(bytes);
            out.writeInstanceInfo(abc.instance_info.get(i));
            out.writeU30(abc.class_info.get(i).cinit_index);
            out.writeTraits(abc.class_info.get(i).static_traits);
            result.add(bytes.toByteArray());
        }
        for (ScriptInfo script : abc.script_info) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            ABCOutputStream out = new ABCOutputStream(bytes);
            out.writeU30(script.init_index);
            out.writeTraits(script.traits);
            result.add(bytes.toByteArray());
        }
        return result;
    }

    /**
     * Take a class of the release out of the movie (srp's own, gone from its
     * source tree). Its script, class and methods must be the last records of
     * an ABC that keeps other scripts: srp's classes of the release were
     * appended to the ABC of a game class. Every other record keeps its index
     * and bytes, every other ABC its bytes, the constant pools stay whole
     * (an entry only the class used stays, unused). The owners of the methods
     * taken out are written to REPORT.
     */
    static void removeScript(Path input, Path output, String cls, Path report) throws Exception {
        SWF swf = MovieSource.read(input);
        ScriptPack pack = MovieSource.findScript(swf, cls);
        ABC abc = pack.abc;
        int home = swf.getAbcList().indexOf(swf.getAbcList().stream().filter(t -> t.getABC() == abc).findFirst().orElseThrow());
        int script = pack.scriptIndex, k = abc.findClassByName(cls);
        if (k < 0) throw new IOException("No class " + cls + " in its script");
        if (abc.script_info.size() < 2 || script != abc.script_info.size() - 1 || k != abc.instance_info.size() - 1)
            throw new IOException(cls + " is not the last script and class of an ABC that has others");
        ScriptInfo info = abc.script_info.get(script);
        if (info.traits.traits.size() != 1 || !(info.traits.traits.get(0) instanceof TraitClass only) || only.class_info != k)
            throw new IOException(cls + "'s script defines more than the class");
        Map<Integer, String> owners = MovieSource.names(abc);
        String classOwner = MovieSource.name(abc, abc.instance_info.get(k).name_index) + "/";
        String scriptOwner = "script:" + info.traits.traits.stream().map(t -> MovieSource.name(abc, t.name_index)).sorted().toList() + "/";
        java.util.function.Predicate<String> its = o -> o != null && Arrays.stream(o.split("\\|"))
            .allMatch(part -> part.startsWith(classOwner) || part.startsWith(scriptOwner));
        int methods = abc.method_info.size(), first = methods;
        while (first > 0 && its.test(owners.get(first - 1))) first--;
        for (int i = 0; i < first; i++) {
            String owner = owners.get(i);
            if (owner != null && (owner.contains(classOwner) || owner.contains(scriptOwner)))
                throw new IOException(cls + "'s methods are not the last of its ABC: " + owner);
        }
        int allBodies = abc.bodies.size(), bodies = allBodies;
        while (bodies > 0 && abc.bodies.get(bodies - 1).method_info >= first) bodies--;
        for (int i = 0; i < bodies; i++)
            if (abc.bodies.get(i).method_info >= first) throw new IOException(cls + "'s method bodies are not the last of its ABC");
        List<String> removed = new ArrayList<>();
        for (int i = first; i < methods; i++) removed.add(owners.get(i));
        List<byte[]> before = records(abc);
        int classes = abc.instance_info.size();
        List<byte[]> kept = new ArrayList<>(before.subList(0, first));
        kept.addAll(before.subList(methods, methods + bodies));
        kept.addAll(before.subList(methods + allBodies, methods + allBodies + k));
        kept.addAll(before.subList(methods + allBodies + classes, methods + allBodies + classes + script));
        abc.script_info.remove(script);
        abc.instance_info.remove(k);
        abc.class_info.remove(k);
        abc.bodies.subList(bodies, allBodies).clear();
        abc.method_info.subList(first, methods).clear();
        abc.resetMethodIndexing();
        abc.clearAllCaches();
        ((Tag) abc.parentTag).setModified(true);
        swf.clearAbcListCache();
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        swf.saveTo(bytes);
        // The saved movie, parsed again, against the input.
        SWF reference = MovieSource.read(input), saved = new SWF(new ByteArrayInputStream(bytes.toByteArray()), true);
        if (reference.getAbcList().size() != saved.getAbcList().size()) throw new IOException("ABC block count changed");
        for (int i = 0; i < reference.getAbcList().size(); i++) {
            ABC a = reference.getAbcList().get(i).getABC(), b = saved.getAbcList().get(i).getABC();
            if (i != home) {
                if (!Arrays.equals(MovieSource.abcBytes(a), MovieSource.abcBytes(b))) throw new IOException("Unrelated ABC block changed: " + i);
                continue;
            }
            for (String pool : List.of("Int", "UInt", "Double", "String", "Namespace", "NamespaceSet", "Multiname")) {
                var count = a.constants.getClass().getMethod("get" + pool + "Count");
                int size = (int) count.invoke(a.constants);
                if ((int) count.invoke(b.constants) != size) throw new IOException("Pool changed: " + pool);
                for (int j = 1; j < size; j++)
                    if (!Objects.equals(MovieSource.poolEntry(a, pool, j), MovieSource.poolEntry(b, pool, j)))
                        throw new IOException("Pool entry changed: " + pool + " " + j);
            }
            if (a.metadata_info.size() != b.metadata_info.size()) throw new IOException("Metadata changed");
            List<byte[]> after = records(b);
            if (after.size() != kept.size()) throw new IOException("Records other than " + cls + "'s went");
            for (int j = 0; j < kept.size(); j++)
                if (!Arrays.equals(kept.get(j), after.get(j))) throw new IOException("A record other than " + cls + "'s changed");
            for (int j = 0; j < b.instance_info.size(); j++)
                if (dotted(b, b.instance_info.get(j).name_index).equals(cls)) throw new IOException(cls + " is still there");
        }
        Files.write(output, bytes.toByteArray(), StandardOpenOption.CREATE_NEW);
        StringBuilder rows = new StringBuilder("method\tstatus\n");
        for (String owner : removed) rows.append(MovieSource.cell(owner)).append("\tremoved\n");
        Files.writeString(report, rows);
        System.out.println("Removed " + cls + ": its script, class and " + removed.size()
            + " methods; every other record is the input's");
    }
}

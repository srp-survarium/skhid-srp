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
 * The source-feature gate: which records of a linked ABC a feature may change.
 *
 * Permission lines (TSV), written by build_feature.py:
 *   method OWNER BEFORE AFTER  an existing method body may change (BEFORE/AFTER
 *                              are reviewed call/cast fingerprints, or "-" when
 *                              that sequence must stay the same, or "?" to
 *                              report a changed sequence for review); the legacy
 *                              three-field form omits the "method" kind
 *   edit CLASS NAME BEFORE AFTER  the same, naming the method by its class and
 *                              name (a getter and setter pair both)
 *   class  CLASS               the feature owns this class (repository-authored
 *                              or new): any of its methods, traits and
 *                              declarations may change or be added
 *   member CLASS NAME          a member of a game class is added or redeclared:
 *                              its traits, signatures and bodies (with nested
 *                              functions) may change or be new
 * Every other original record (method signature and body, class header, trait,
 * script) must be byte-identical, and nothing else may be added. Additions come
 * after all original records, so every original index keeps its meaning.
 */
final class FeatureGate {
    private FeatureGate() {}

    static String dotted(ABC abc, int nameIndex) {
        Multiname name = abc.constants.getMultiname(nameIndex);
        String local = name.name_index == 0 ? "*" : abc.constants.getString(name.name_index);
        if (!name.hasOwnNamespace() || name.namespace_index == 0) return local;
        Namespace ns = abc.constants.getNamespace(name.namespace_index);
        String pkg = ns.name_index == 0 ? "" : abc.constants.getString(ns.name_index);
        return pkg.isEmpty() || ns.kind != Namespace.KIND_PACKAGE ? local : pkg + "." + local;
    }

    static String local(ABC abc, int nameIndex) {
        Multiname name = abc.constants.getMultiname(nameIndex);
        return name.name_index == 0 ? "*" : abc.constants.getString(name.name_index);
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

    static byte[] traitBytes(Trait trait) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        new ABCOutputStream(bytes).writeTrait(trait);
        return bytes.toByteArray();
    }

    static byte[] methodInfoBytes(MethodInfo info) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        new ABCOutputStream(bytes).writeMethodInfo(info);
        return bytes.toByteArray();
    }

    /** The number of traits added or redeclared (each one permitted). */
    static int compareTraits(ABC original, ABC compiled, Traits before, Traits after, Set<String> free,
                             boolean owned, String where) throws IOException {
        int changed = 0;
        for (int j = 0; j < after.traits.size(); j++) {
            Trait b = after.traits.get(j);
            String name = local(compiled, b.name_index);
            boolean same = j < before.traits.size() && Arrays.equals(traitBytes(before.traits.get(j)), traitBytes(b));
            if (same) continue;
            if (!owned && !free.contains(name))
                throw new IOException(j >= before.traits.size() ? "Feature added an undeclared member " + where + name
                    : "Feature changed the declaration of " + where + name);
            changed++;
        }
        return changed;
    }

    /** The owners of a class's methods named `name` (instance or static; the class's own name is its constructor). */
    static List<String> methodOwners(ABC abc, String cls, String name) throws IOException {
        Integer k = classes(abc).get(cls);
        List<String> result = new ArrayList<>();
        if (k == null) return result;
        String owner = MovieSource.name(abc, abc.instance_info.get(k).name_index);
        if (name.equals(local(abc, abc.instance_info.get(k).name_index))) result.add(owner + "/instance-init");
        // "class-init": the class initializer (static declarations' values).
        if (name.equals("class-init")) result.add(owner + "/class-init");
        for (var group : List.of(List.of("/instance", abc.instance_info.get(k).instance_traits),
                                 List.of("/static", abc.class_info.get(k).static_traits)))
            for (Trait trait : ((Traits) group.get(1)).traits)
                if (trait instanceof TraitMethodGetterSetter && local(abc, trait.name_index).equals(name))
                    result.add(owner + group.get(0) + "/" + trait.kindType + "/" + MovieSource.name(abc, trait.name_index));
        return result;
    }

    static boolean under(String owner, String method) {
        return owner.equals(method) || owner.startsWith(method + "/function[");
    }

    static String check(ABC original, ABC compiled, String prefix, Path permissions) throws Exception {
        Map<String, String[]> edited = new LinkedHashMap<>();
        Set<String> owned = new LinkedHashSet<>();
        Map<String, Set<String>> members = new LinkedHashMap<>();
        List<String> lines = new ArrayList<>();
        for (String line : Files.readAllLines(permissions)) {
            String[] fields = line.split("\\t", -1);
            if (fields[0].equals("edit")) {
                if (fields.length != 5) throw new IOException("Invalid feature edit permission");
                List<String> owners = methodOwners(original, fields[1], fields[2]);
                if (owners.isEmpty()) throw new IOException("Feature edit names no method: " + fields[1] + "." + fields[2]);
                for (String owner : owners) lines.add(String.join("\t", "method", owner, fields[3], fields[4]));
            } else lines.add(line);
        }
        for (String line : lines) {
            String[] fields = line.split("\\t", -1);
            String kind = fields[0];
            if (!Set.of("method", "class", "member").contains(kind) && fields.length == 3) {
                kind = "method";
                fields = new String[]{"method", fields[0], fields[1], fields[2]};
            }
            switch (kind) {
                case "method" -> {
                    if (fields.length != 4 || !fields[1].startsWith(prefix) || fields[1].contains("|")
                            || edited.put(fields[1], fields) != null)
                        throw new IOException("Invalid or duplicate feature method permission");
                    if (!(fields[2].equals("-") && fields[3].equals("-"))
                            && !(fields[2].equals("?") && fields[3].equals("?"))
                            && !(fields[2].matches("[0-9a-f]{64}") && fields[3].matches("[0-9a-f]{64}")))
                        throw new IOException("Call/cast permission requires both reviewed fingerprints");
                }
                case "class" -> {
                    if (fields.length != 2 || fields[1].isEmpty() || !owned.add(fields[1]))
                        throw new IOException("Invalid or duplicate feature class permission");
                }
                case "member" -> {
                    if (fields.length != 3 || fields[2].isEmpty()
                            || !members.computeIfAbsent(fields[1], k -> new LinkedHashSet<>()).add(fields[2]))
                        throw new IOException("Invalid or duplicate feature member permission");
                }
                default -> throw new IOException("Unknown feature permission " + kind);
            }
        }
        if (edited.isEmpty() && owned.isEmpty() && members.isEmpty()) throw new IOException("No feature methods authorized");
        Map<String, Integer> oldClasses = classes(original), newClasses = classes(compiled);
        for (String cls : owned)
            if (!newClasses.containsKey(cls)) throw new IOException("Feature class is not in this ABC: " + cls);
        for (String cls : members.keySet())
            if (!oldClasses.containsKey(cls) || owned.contains(cls))
                throw new IOException("Feature member permission names no game class of this ABC: " + cls);

        // Owners whose records may change freely: owned classes, their
        // scripts, and permitted members (with nested functions).
        List<String> freePrefixes = new ArrayList<>();
        List<String> freeMethods = new ArrayList<>();
        for (String cls : owned)
            freePrefixes.add(MovieSource.name(compiled, compiled.instance_info.get(newClasses.get(cls)).name_index) + "/");
        for (ScriptInfo script : compiled.script_info) {
            boolean hosts = false;
            for (Trait trait : script.traits.traits)
                hosts |= trait instanceof TraitClass c && owned.contains(dotted(compiled, compiled.instance_info.get(c.class_info).name_index));
            if (hosts) freePrefixes.add("script:" + script.traits.traits.stream()
                .map(t -> MovieSource.name(compiled, t.name_index)).sorted().toList() + "/");
        }
        for (var entry : members.entrySet()) {
            int k = newClasses.get(entry.getKey());
            String owner = MovieSource.name(compiled, compiled.instance_info.get(k).name_index);
            for (var group : List.of(List.of("/instance", compiled.instance_info.get(k).instance_traits),
                                     List.of("/static", compiled.class_info.get(k).static_traits)))
                for (Trait trait : ((Traits) group.get(1)).traits)
                    if (entry.getValue().contains(local(compiled, trait.name_index)))
                        freeMethods.add(owner + group.get(0) + "/" + trait.kindType + "/" + MovieSource.name(compiled, trait.name_index));
        }
        java.util.function.Predicate<String> free = owner -> owner != null
            && (freePrefixes.stream().anyMatch(owner::startsWith) || freeMethods.stream().anyMatch(m -> under(owner, m)));

        Map<Integer, String> oldOwners = MovieSource.names(original), newOwners = MovieSource.names(compiled);
        for (int i = 0; i < original.method_info.size(); i++) {
            String owner = oldOwners.get(i);
            if (!Objects.equals(owner, newOwners.get(i))) throw new IOException("Method identity moved at " + i);
            if (free.test(owner)) continue;
            if (!Arrays.equals(methodInfoBytes(original.method_info.get(i)), methodInfoBytes(compiled.method_info.get(i))))
                throw new IOException("Feature changed the signature of " + owner);
        }
        int declarations = 0;
        for (int k = 0; k < compiled.instance_info.size(); k++) {
            String cls = dotted(compiled, compiled.instance_info.get(k).name_index);
            if (k >= original.instance_info.size()) {
                if (!owned.contains(cls)) throw new IOException("Feature added an undeclared class " + cls);
                declarations++;
                continue;
            }
            boolean whole = owned.contains(cls);
            InstanceInfo a = original.instance_info.get(k), b = compiled.instance_info.get(k);
            if (!whole && (a.name_index != b.name_index || a.super_index != b.super_index || a.flags != b.flags
                    || a.protectedNS != b.protectedNS || !Arrays.equals(a.interfaces, b.interfaces)
                    || a.iinit_index != b.iinit_index
                    || original.class_info.get(k).cinit_index != compiled.class_info.get(k).cinit_index))
                throw new IOException("Feature changed the class declaration of " + cls);
            Set<String> names = members.getOrDefault(cls, Set.of());
            declarations += compareTraits(original, compiled, a.instance_traits, b.instance_traits, names, whole, cls + " instance ");
            declarations += compareTraits(original, compiled, original.class_info.get(k).static_traits,
                compiled.class_info.get(k).static_traits, names, whole, cls + " static ");
        }
        for (int s = 0; s < compiled.script_info.size(); s++) {
            ScriptInfo script = compiled.script_info.get(s);
            String owner = "script:" + script.traits.traits.stream().map(t -> MovieSource.name(compiled, t.name_index)).sorted().toList() + "/";
            boolean hosts = free.test(owner);
            if (s >= original.script_info.size()) {
                for (Trait trait : script.traits.traits)
                    if (!(trait instanceof TraitClass c) || c.class_info < original.instance_info.size()
                            || !owned.contains(dotted(compiled, compiled.instance_info.get(c.class_info).name_index)))
                        throw new IOException("Feature added an undeclared script trait " + dotted(compiled, trait.name_index));
                continue;
            }
            if (hosts) continue;
            if (original.script_info.get(s).init_index != script.init_index)
                throw new IOException("Feature changed script " + s);
            compareTraits(original, compiled, original.script_info.get(s).traits, script.traits, Set.of(), false, "script ");
        }

        StringBuilder report = new StringBuilder("method\tstatus\tbefore_record\tafter_record\tbefore_calls_casts\tafter_calls_casts\n");
        Set<String> seen = new HashSet<>();
        int changed = 0;
        for (MethodBody after : compiled.bodies) {
            String owner = newOwners.get(after.method_info);
            if (owner == null) throw new IOException("Unowned compiled method " + after.method_info);
            String parent = edited.keySet().stream().filter(m -> under(owner, m)).findFirst().orElse(null);
            boolean isNew = after.method_info >= original.method_info.size();
            MethodBody before = isNew ? null : original.findBody(after.method_info);
            if (!isNew && before == null) throw new IOException("Feature gave a body to " + owner);
            byte[] b = MovieSource.record(compiled, after), a = before == null ? null : MovieSource.record(original, before);
            String status;
            if (isNew) {
                if (!free.test(owner) && parent == null) throw new IOException("Feature added an undeclared method " + owner);
                status = "added";
            } else if (edited.containsKey(owner)) {
                if (!seen.add(owner)) throw new IOException("Ambiguous feature method owner");
                String oldCalls = MovieSource.callCastFingerprint(original, before);
                String newCalls = MovieSource.callCastFingerprint(compiled, after);
                String[] permission = edited.get(owner);
                String state = Arrays.equals(a, b) ? "unchanged" : "changed";
                if (permission[2].equals("?")) {
                    // Review mode: report the sequence change, refuse nothing;
                    // the build then stops before publishing a movie.
                    if (!oldCalls.equals(newCalls)) state = "unreviewed";
                } else if (permission[2].equals("-")) {
                    if (!oldCalls.equals(newCalls)) throw new IOException("Unreviewed calls/casts changed in " + owner
                        + ": " + oldCalls + " -> " + newCalls);
                } else if (!permission[2].equals(oldCalls) || !permission[3].equals(newCalls))
                    throw new IOException("Reviewed calls/casts do not match compiler output for " + owner
                        + ": " + oldCalls + " -> " + newCalls);
                report.append(MovieSource.cell(owner)).append('\t').append(state)
                    .append('\t').append(MovieSource.hash(a)).append('\t').append(MovieSource.hash(b)).append('\t')
                    .append(oldCalls).append('\t').append(newCalls).append('\n');
                if (!Arrays.equals(a, b)) changed++;
                continue;
            } else if (Arrays.equals(a, b)) {
                continue;
            } else if (free.test(owner) || parent != null) {
                status = "changed";
            } else {
                throw new IOException("Untouched method changed: " + owner);
            }
            changed++;
            report.append(MovieSource.cell(owner)).append('\t').append(status).append('\t')
                .append(a == null ? "-" : MovieSource.hash(a)).append('\t').append(MovieSource.hash(b)).append("\t-\t-\n");
        }
        if (!seen.equals(edited.keySet())) throw new IOException("Feature permission names a missing method");
        if (changed == 0 && declarations == 0)
            throw new IOException("Feature compiled without changing any authorized method or declaration");
        return report.toString();
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
            + " definition bytes); the feature link checks every original record");
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

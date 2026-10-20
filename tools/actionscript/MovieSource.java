import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.zip.*;
import javax.xml.parsers.DocumentBuilderFactory;
import javax.xml.transform.TransformerFactory;
import javax.xml.transform.dom.DOMSource;
import javax.xml.transform.stream.StreamResult;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.abc.*;
import com.jpexs.decompiler.flash.abc.types.*;
import com.jpexs.decompiler.flash.abc.types.traits.*;
import com.jpexs.decompiler.flash.configuration.Configuration;
import com.jpexs.decompiler.flash.exporters.modes.ScriptExportMode;
import com.jpexs.decompiler.flash.exporters.settings.ScriptExportSettings;
import com.jpexs.decompiler.flash.importers.*;
import com.jpexs.decompiler.flash.tags.Tag;
import com.jpexs.decompiler.flash.flexsdk.MxmlcAs3ScriptReplacer;
import com.jpexs.decompiler.flash.flexsdk.MxmlcException;

/** FFDec bridge. Output snapshots are diagnostics; only whole-byte comparison proves equality. */
public final class MovieSource {
    // FFDec 25.1.3's Unix path selection returns mxmlc.exe when bin/mxmlc exists.
    // Invoke the same SDK jar directly, keeping the replacer's compiler arguments.
    static final class FlexCompiler extends MxmlcAs3ScriptReplacer {
        final Path sdk;
        final Path diagnostics;
        final boolean strict;
        FlexCompiler(String path, Path diagnostics, boolean strict) {
            super(path); sdk = Path.of(path); this.diagnostics = diagnostics; this.strict = strict;
        }
        // Adapt only the compiler's temporary dependency SWC. Qualify singleton
        // namespace sets and supply the inheritance catalog missing from FFDec's
        // export. The target movie remains untouched.
        static void qualifyDependencyInterfaces(Path swc) throws IOException, InterruptedException {
            Map<String, byte[]> entries = new LinkedHashMap<>();
            try (ZipInputStream zip = new ZipInputStream(Files.newInputStream(swc))) {
                for (ZipEntry entry; (entry = zip.getNextEntry()) != null; ) entries.put(entry.getName(), zip.readAllBytes());
            }
            byte[] library = entries.get("library.swf");
            if (library == null) throw new IOException("Dependency SWC has no library.swf");
            SWF dependency = new SWF(new ByteArrayInputStream(library), true);
            int changed = 0;
            for (var tag : dependency.getAbcList()) {
                ABC abc = tag.getABC();
                for (int j = 1; j < abc.constants.getMultinameCount(); j++) {
                    Multiname m = abc.constants.getMultiname(j);
                    // ASC's SWC reader dereferences multiname 0 for Vector.<*>.
                    // Its explicit public '*' name represents the same wildcard
                    // without that null entry in the compiler dependency only.
                    if (m.kind == 0x1d) {
                        for (int k = 0; k < m.params.length; k++) if (m.params[k] == 0) {
                            m.params[k] = abc.constants.getPublicQnameId("*", true);
                            ((Tag)tag).setModified(true);
                        }
                    }
                    if (m.kind != 0x09) continue;
                    int[] ns = abc.constants.getNamespaceSet(m.namespace_set_index).namespaces;
                    if (ns.length != 1) continue;
                    abc.constants.setMultiname(j, Multiname.createQName(false, m.name_index, ns[0]));
                    ((Tag)tag).setModified(true);
                    changed++;
                }
            }
            ByteArrayOutputStream buf = new ByteArrayOutputStream();
            dependency.saveTo(buf);
            entries.put("library.swf", buf.toByteArray());
            // FFDec's SWC catalog lists AS3 but omits inheritance dependencies.
            // Flex uses these entries to load interfaces before their consumers.
            try {
                Map<String, List<String>> inheritance = new HashMap<>();
                for (var tag : dependency.getAbcList()) {
                    ABC abc = tag.getABC();
                    for (InstanceInfo ins : abc.instance_info) {
                        List<String> bases = new ArrayList<>();
                        if (ins.super_index != 0) bases.add(catalogName(abc, ins.super_index));
                        for (int id : ins.interfaces) bases.add(catalogName(abc, id));
                        inheritance.put(catalogName(abc, ins.name_index), bases);
                    }
                }
                var factory = DocumentBuilderFactory.newInstance();
                factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
                var doc = factory.newDocumentBuilder().parse(new ByteArrayInputStream(entries.get("catalog.xml")));
                var scripts = doc.getElementsByTagName("script");
                Set<String> definitions = new HashSet<>();
                var allDefs = doc.getElementsByTagName("def");
                for (int i = 0; i < allDefs.getLength(); i++)
                    definitions.add(((org.w3c.dom.Element)allDefs.item(i)).getAttribute("id"));
                Map<String, Set<String>> references = new HashMap<>();
                for (var tag : dependency.getAbcList()) {
                    ABC abc = tag.getABC();
                    Set<String> refs = new TreeSet<>();
                    for (int j = 1; j < abc.constants.getMultinameCount(); j++) {
                        if (abc.constants.getMultiname(j).kind != 7) continue;
                        String ref = catalogName(abc, j);
                        if (definitions.contains(ref)) refs.add(ref);
                    }
                    for (InstanceInfo ins : abc.instance_info)
                        references.put(catalogName(abc, ins.name_index), refs);
                }
                for (int i = 0; i < scripts.getLength(); i++) {
                    var script = (org.w3c.dom.Element)scripts.item(i);
                    var defs = script.getElementsByTagName("def");
                    Set<String> deps = new TreeSet<>();
                    Set<String> own = new HashSet<>(), expression = new TreeSet<>();
                    for (int j = 0; j < defs.getLength(); j++)
                        deps.addAll(inheritance.getOrDefault(((org.w3c.dom.Element)defs.item(j)).getAttribute("id"), List.of()));
                    for (int j = 0; j < defs.getLength(); j++) {
                        String id = ((org.w3c.dom.Element)defs.item(j)).getAttribute("id");
                        own.add(id); expression.addAll(references.getOrDefault(id, Set.of()));
                    }
                    expression.removeAll(own); expression.removeAll(deps);
                    for (String base : deps) {
                        var dep = doc.createElement("dep"); dep.setAttribute("id", base); dep.setAttribute("type", "i");
                        script.appendChild(dep);
                    }
                    for (String ref : expression) {
                        var dep = doc.createElement("dep"); dep.setAttribute("id", ref); dep.setAttribute("type", "e");
                        script.appendChild(dep);
                    }
                }
                buf.reset();
                TransformerFactory.newInstance().newTransformer().transform(new DOMSource(doc), new StreamResult(buf));
                entries.put("catalog.xml", buf.toByteArray());
            } catch (Exception error) { throw new IOException("Cannot construct dependency catalog", error); }
            try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(swc))) {
                for (var entry : entries.entrySet()) {
                    zip.putNextEntry(new ZipEntry(entry.getKey()));
                    zip.write(entry.getValue()); zip.closeEntry();
                }
            }
            System.out.println("Qualified " + changed + " singleton namespace references in compiler dependencies");
        }
        static String catalogName(ABC abc, int id) throws IOException {
            Multiname m = abc.constants.getMultiname(id);
            if (m.kind != 7) throw new IOException("Non-qualified inheritance name " + m);
            String ns = abc.constants.getString(abc.constants.getNamespace(m.namespace_index).name_index);
            return (ns.isEmpty() ? "" : ns + ":") + abc.constants.getString(m.name_index);
        }
        @Override public void mxmlc(List<String> args) throws MxmlcException, InterruptedException, IOException {
            Files.createDirectories(diagnostics);
            for (int i = 0; i + 1 < args.size(); i++)
                if (args.get(i).equals("-library-path")) {
                    Path dependency = Path.of(args.get(i + 1));
                    Files.copy(dependency, diagnostics.resolve("dependency-" + i + "-original.swc"));
                    qualifyDependencyInterfaces(dependency);
                    Files.copy(dependency, diagnostics.resolve("dependency-" + i + "-qualified.swc"));
                }
            List<String> command = new ArrayList<>(List.of(
                Path.of(System.getProperty("java.home"), "bin", "java").toString(), "-Xmx1g", "-jar",
                sdk.resolve("lib/mxmlc.jar").toString(), "+flexlib=" + sdk.resolve("frameworks"),
                "+playerglobalHome=" + sdk.resolve("frameworks/libs/player")));
            command.addAll(args);
            if (strict) {
                command.remove("-strict=false");
                command.remove("-debug=true");
                command.addAll(List.of("-strict=true", "-debug=false", "-optimize=false", "-omit-trace-statements=false"));
            }
            command.add("-external-library-path=" + sdk.resolve("frameworks/libs/player/32.0/playerglobal.swc"));
            Files.write(diagnostics.resolve("compiler-arguments.txt"), command);
            Path errors = Files.createTempFile("srp-flex-", ".log");
            Process process = null;
            try {
                ProcessBuilder builder = new ProcessBuilder(command).redirectOutput(diagnostics.resolve("mxmlc.stdout").toFile())
                    .redirectError(errors.toFile());
                builder.environment().put("PLAYERGLOBAL_HOME", sdk.resolve("frameworks/libs/player").toString());
                process = builder.start();
                if (process.waitFor() != 0) throw new MxmlcException(Files.readString(errors));
            } finally {
                if (process != null) process.destroyForcibly();
                Files.copy(errors, diagnostics.resolve("mxmlc.stderr"));
                Files.deleteIfExists(errors);
            }
        }
    }
    static SWF read(Path file) throws Exception {
        try (InputStream in = Files.newInputStream(file)) { return new SWF(in, true); }
    }

    static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    static String name(ABC abc, int index) {
        return index == 0 ? "*" : abc.constants.getMultiname(index).toString(abc.constants, List.of());
    }

    static void traits(ABC abc, String owner, Traits traits, Map<Integer, String> names) {
        for (Trait t : traits.traits) {
            int method = t instanceof TraitMethodGetterSetter ? ((TraitMethodGetterSetter)t).method_info
                : t instanceof TraitFunction ? ((TraitFunction)t).method_info : -1;
            if (method >= 0) names.merge(method, owner + "/" + t.kindType + "/" + name(abc, t.name_index),
                (a, b) -> a + "|" + b);
        }
    }

    static Map<Integer, String> names(ABC abc) {
        Map<Integer, String> result = new TreeMap<>();
        for (int i = 0; i < abc.instance_info.size(); i++) {
            InstanceInfo ins = abc.instance_info.get(i);
            ClassInfo cls = abc.class_info.get(i);
            String owner = name(abc, ins.name_index);
            result.put(ins.iinit_index, owner + "/instance-init");
            result.put(cls.cinit_index, owner + "/class-init");
            traits(abc, owner + "/instance", ins.instance_traits, result);
            traits(abc, owner + "/static", cls.static_traits, result);
        }
        for (ScriptInfo s : abc.script_info) {
            String owner = "script:" + s.traits.traits.stream().map(t -> name(abc, t.name_index)).sorted().toList();
            result.put(s.init_index, owner + "/init");
            traits(abc, owner, s.traits, result);
        }
        return result;
    }

    static byte[] record(ABC abc, MethodBody body) throws Exception {
        ByteArrayOutputStream buf = new ByteArrayOutputStream();
        ABCOutputStream out = new ABCOutputStream(buf);
        out.writeMethodInfo(abc.method_info.get(body.method_info));
        out.writeU30(body.max_stack);
        out.writeU30(body.max_regs);
        out.writeU30(body.init_scope_depth);
        out.writeU30(body.max_scope_depth);
        out.writeU30(body.getCodeBytes().length);
        out.write(body.getCodeBytes());
        out.writeU30(body.exceptions.length);
        for (ABCException e : body.exceptions) {
            out.writeU30(e.start); out.writeU30(e.end); out.writeU30(e.target);
            out.writeU30(e.type_index); out.writeU30(e.name_index);
        }
        out.writeTraits(body.traits);
        return buf.toByteArray();
    }

    static byte[] abcBytes(ABC abc) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        abc.saveToStream(out);
        return out.toByteArray();
    }

    static Object poolEntry(ABC abc, String pool, int index) throws Exception {
        Object value = abc.constants.getClass().getMethod("get" + pool, int.class).invoke(abc.constants, index);
        if (value instanceof Namespace ns) {
            ByteArrayOutputStream buffer = new ByteArrayOutputStream();
            new ABCOutputStream(buffer).writeNamespace(ns);
            return HexFormat.of().formatHex(buffer.toByteArray());
        }
        if (value instanceof NamespaceSet ns) return Arrays.toString(ns.namespaces);
        if (value instanceof Multiname name) {
            ByteArrayOutputStream buffer = new ByteArrayOutputStream();
            new ABCOutputStream(buffer).writeMultiname(name);
            return HexFormat.of().formatHex(buffer.toByteArray());
        }
        if (value instanceof Double d) return Double.doubleToRawLongBits(d);
        if (value instanceof Float f) return Float.floatToRawIntBits(f);
        return value;
    }

    static List<Integer> runtimeMetadata(ABC abc, Trait trait) {
        List<Integer> result = new ArrayList<>();
        if ((trait.kindFlags & Trait.ATTR_Metadata) == 0) return result;
        for (int id : trait.metadata) {
            String name = abc.constants.getString(abc.metadata_info.get(id).name_index);
            if (!name.equals("__go_to_definition_help") && !name.equals("__go_to_ctor_definition_help")) result.add(id);
        }
        return result;
    }

    static void retainDebugMetadata(ABC original, ABC compiled, Traits before, Traits after) throws IOException {
        if (before.traits.size() != after.traits.size()) throw new IOException("Trait count changed");
        for (int j = 0; j < before.traits.size(); j++) {
            Trait a = before.traits.get(j), b = after.traits.get(j);
            if (a.kindType != b.kindType || !name(original, a.name_index).equals(name(compiled, b.name_index)))
                throw new IOException("Trait identities or ordering changed");
            if (!runtimeMetadata(original, a).equals(runtimeMetadata(compiled, b)))
                throw new IOException("Runtime trait metadata changed");
            b.metadata = a.metadata == null ? null : a.metadata.clone();
            b.kindFlags = (b.kindFlags & ~Trait.ATTR_Metadata) | (a.kindFlags & Trait.ATTR_Metadata);
        }
    }

    static void relocateTraitMethods(Traits traits, Map<Integer, Integer> mapping) {
        for (Trait trait : traits.traits) {
            if (trait instanceof TraitMethodGetterSetter method) method.method_info = mapping.get(method.method_info);
            else if (trait instanceof TraitFunction function) function.method_info = mapping.get(function.method_info);
        }
    }

    static void restoreMethodOrder(ABC original, ABC compiled) throws IOException {
        if (original.method_info.size() != compiled.method_info.size()) throw new IOException("Method count changed");
        Map<Integer, String> before = names(original), after = names(compiled);
        if (before.equals(after)) return;
        if (before.size() != original.method_info.size() || after.size() != compiled.method_info.size())
            throw new IOException("Anonymous method ownership is unresolved");
        Map<String, Integer> owners = new HashMap<>();
        for (var entry : before.entrySet())
            if (owners.put(entry.getValue(), entry.getKey()) != null) throw new IOException("Ambiguous method owner");
        Map<Integer, Integer> mapping = new HashMap<>();
        Set<Integer> assigned = new HashSet<>();
        for (var entry : after.entrySet()) {
            Integer index = owners.get(entry.getValue());
            if (index == null || !assigned.add(index)) throw new IOException("Method identities changed");
            mapping.put(entry.getKey(), index);
        }
        List<MethodInfo> infos = new ArrayList<>(Collections.nCopies(compiled.method_info.size(), null));
        for (var entry : mapping.entrySet()) infos.set(entry.getValue(), compiled.method_info.get(entry.getKey()));
        Map<Integer, MethodBody> bodies = new HashMap<>();
        for (MethodBody body : compiled.bodies) {
            var code = body.getCode();
            for (var instruction : code.code) {
                int length = instruction.getBytesLength();
                for (int j = 0; j < instruction.definition.operands.length; j++)
                    if (instruction.definition.operands[j] == com.jpexs.decompiler.flash.abc.avm2.AVM2Code.DAT_METHOD_INDEX) {
                        Integer index = mapping.get(instruction.operands[j]);
                        if (index == null) throw new IOException("Unmapped method instruction operand");
                        instruction.operands[j] = index;
                    }
                // Until branch/exception offset relocation is implemented,
                // never shift an instruction or silently leave stale offsets.
                if (instruction.getBytesLength() != length) throw new IOException("Method reference width changed; offset relocation required");
            }
            body.setCode(code);
            relocateTraitMethods(body.traits, mapping);
            body.method_info = mapping.get(body.method_info);
            if (bodies.put(body.method_info, body) != null) throw new IOException("Duplicate method body");
        }
        for (InstanceInfo ins : compiled.instance_info) {
            ins.iinit_index = mapping.get(ins.iinit_index);
            relocateTraitMethods(ins.instance_traits, mapping);
        }
        for (ClassInfo cls : compiled.class_info) {
            cls.cinit_index = mapping.get(cls.cinit_index);
            relocateTraitMethods(cls.static_traits, mapping);
        }
        for (ScriptInfo script : compiled.script_info) {
            script.init_index = mapping.get(script.init_index);
            relocateTraitMethods(script.traits, mapping);
        }
        List<MethodBody> ordered = new ArrayList<>();
        for (MethodBody body : original.bodies) {
            MethodBody emitted = bodies.remove(body.method_info);
            if (emitted == null) throw new IOException("Compiled method has no body");
            ordered.add(emitted);
        }
        if (!bodies.isEmpty()) throw new IOException("Unexpected compiled method bodies");
        compiled.method_info = infos;
        compiled.bodies = ordered;
        compiled.resetMethodIndexing();
    }

    // Conservative first layout linker. It never imports an original method
    // body. All executable records and class/script definitions come from the
    // compiler. Existing pool entries must retain their original indices and
    // values; unused appended constants, compiler debug names and Adobe editor
    // navigation metadata follow the original layout. A byte comparison of the
    // entire ABC must then pass. Runtime metadata is never silently restored.
    static void linkLayout(Path referenceFile, Path candidateFile, Path output, String cls) throws Exception {
        SWF reference = read(referenceFile), candidate = read(candidateFile);
        if (reference.getAbcList().size() != candidate.getAbcList().size())
            throw new IOException("ABC block count changed");
        int home = -1;
        for (int i = 0; i < reference.getAbcList().size(); i++) {
            if (reference.getAbcList().get(i).getABC().findClassByName(cls) < 0) continue;
            if (home != -1) throw new IOException("Ambiguous class " + cls);
            home = i;
        }
        if (home < 0) throw new IOException("Missing class " + cls);
        for (int i = 0; i < reference.getAbcList().size(); i++) {
            if (i != home && !Arrays.equals(abcBytes(reference.getAbcList().get(i).getABC()),
                    abcBytes(candidate.getAbcList().get(i).getABC())))
                throw new IOException("Unrelated ABC block changed: " + i);
        }
        ABC original = reference.getAbcList().get(home).getABC();
        ABC compiled = candidate.getAbcList().get(home).getABC();
        if (original.version.major != compiled.version.major || original.version.minor != compiled.version.minor)
            throw new IOException("ABC version changed");
        for (String pool : List.of("Int", "UInt", "Double", "String", "Namespace", "NamespaceSet", "Multiname")) {
            var count = original.constants.getClass().getMethod("get" + pool + "Count");
            int size = (int)count.invoke(original.constants);
            if ((int)count.invoke(compiled.constants) < size) throw new IOException("Pool was compacted: " + pool);
            for (int j = 1; j < size; j++)
                if (!Objects.equals(poolEntry(original, pool, j), poolEntry(compiled, pool, j)))
                    throw new IOException("Existing pool entry changed: " + pool + " " + j);
        }
        if (original.hasDecimalSupport() || original.hasFloatSupport() || original.hasFloat4Support())
            throw new IOException("Extended numeric ABC versions are not supported by this linker");
        restoreMethodOrder(original, compiled);
        if (compiled.metadata_info.size() < original.metadata_info.size()) throw new IOException("Metadata pool compacted");
        for (int i = 0; i < original.metadata_info.size(); i++) {
            MetadataInfo a = original.metadata_info.get(i), b = compiled.metadata_info.get(i);
            if (a.name_index != b.name_index || !Arrays.equals(a.keys, b.keys) || !Arrays.equals(a.values, b.values))
                throw new IOException("Existing metadata pool entry changed");
        }
        if (original.instance_info.size() != compiled.instance_info.size() || original.script_info.size() != compiled.script_info.size()
                || original.bodies.size() != compiled.bodies.size()) throw new IOException("ABC record counts changed");
        for (int i = 0; i < original.instance_info.size(); i++) {
            retainDebugMetadata(original, compiled, original.instance_info.get(i).instance_traits, compiled.instance_info.get(i).instance_traits);
            retainDebugMetadata(original, compiled, original.class_info.get(i).static_traits, compiled.class_info.get(i).static_traits);
        }
        for (int i = 0; i < original.script_info.size(); i++)
            retainDebugMetadata(original, compiled, original.script_info.get(i).traits, compiled.script_info.get(i).traits);
        for (int i = 0; i < original.bodies.size(); i++) {
            if (original.bodies.get(i).method_info != compiled.bodies.get(i).method_info) throw new IOException("Method body ordering changed");
            retainDebugMetadata(original, compiled, original.bodies.get(i).traits, compiled.bodies.get(i).traits);
        }
        for (int i = 0; i < original.method_info.size(); i++) {
            MethodInfo before = original.method_info.get(i), after = compiled.method_info.get(i);
            String oldName = before.name_index == 0 ? "" : original.constants.getString(before.name_index);
            String newName = after.name_index == 0 ? "" : compiled.constants.getString(after.name_index);
            if (!oldName.isEmpty() && !oldName.equals(newName))
                throw new IOException("Nonempty debug method name changed at " + i);
            after.name_index = before.name_index;
        }
        compiled.constants = original.constants;
        compiled.metadata_info = original.metadata_info;
        byte[] rebuilt = abcBytes(compiled), expected = abcBytes(original);
        if (!Arrays.equals(rebuilt, expected)) {
            int first = 0;
            while (first < Math.min(rebuilt.length, expected.length) && rebuilt[first] == expected[first]) first++;
            throw new IOException("Compiled ABC still differs at byte " + first + " (original " + expected.length
                + " bytes, compiled " + rebuilt.length + "); no output written");
        }
        ((Tag)compiled.parentTag).setModified(true);
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        candidate.saveTo(bytes);
        SWF saved = new SWF(new ByteArrayInputStream(bytes.toByteArray()), true);
        for (int i = 0; i < reference.getAbcList().size(); i++)
            if (!Arrays.equals(abcBytes(reference.getAbcList().get(i).getABC()), abcBytes(saved.getAbcList().get(i).getABC())))
                throw new IOException("Serialized ABC differs at block " + i);
        Files.write(output, bytes.toByteArray(), StandardOpenOption.CREATE_NEW);
        System.out.println("Exact compiled ABC for " + cls + ": " + rebuilt.length + " bytes; original bodies were not substituted");
    }

    static String cell(String s) { return s.replace("\t", "\\t").replace("\n", "\\n").replace("\r", "\\r"); }

    static void snapshot(SWF swf, Path dir) throws Exception {
        Files.createDirectories(dir);
        StringBuilder rows = new StringBuilder("abc\tmethod\towner\tcode_size\tcode_sha256\trecord_sha256\tassembly_sha256\n");
        for (int block = 0; block < swf.getAbcList().size(); block++) {
            ABC abc = swf.getAbcList().get(block).getABC();
            Path folder = dir.resolve(String.format("abc-%04d", block));
            Files.createDirectories(folder);
            try (OutputStream out = Files.newOutputStream(folder.resolve("block.abc"))) { abc.saveToStream(out); }
            Map<Integer, String> names = names(abc);
            for (MethodBody body : abc.bodies) {
                String owner = names.getOrDefault(body.method_info, "unowned-index:" + body.method_info);
                StringBuilder asm = new StringBuilder();
                for (var instruction : body.getCode().code)
                    asm.append(instruction.toStringNoAddress(abc.constants, List.of())).append('\n');
                byte[] assembly = asm.toString().getBytes(StandardCharsets.UTF_8);
                String stem = String.format("method-%05d", body.method_info);
                Files.write(folder.resolve(stem + ".record"), record(abc, body));
                Files.write(folder.resolve(stem + ".asm"), assembly);
                rows.append(block).append('\t').append(body.method_info).append('\t').append(cell(owner))
                    .append('\t').append(body.getCodeBytes().length).append('\t').append(hash(body.getCodeBytes()))
                    .append('\t').append(hash(record(abc, body))).append('\t').append(hash(assembly)).append('\n');
            }
        }
        Files.writeString(dir.resolve("methods.tsv"), rows);
    }

    static void run(String[] a) throws Exception {
        if (a.length < 3) throw new IllegalArgumentException("export|snapshot|compile INPUT OUTPUT [CLASS SOURCE ffdec|flex SDK]");
        Configuration.parallelSpeedUp.set(false);
        if (a[0].equals("link-layout")) {
            if (a.length != 5) throw new IllegalArgumentException("link-layout ORIGINAL COMPILED OUTPUT CLASS");
            linkLayout(Path.of(a[1]), Path.of(a[2]), Path.of(a[3]), a[4]);
            return;
        }
        if (a[0].equals("dependencies")) {
            Files.copy(Path.of(a[1]), Path.of(a[2]));
            FlexCompiler.qualifyDependencyInterfaces(Path.of(a[2]));
            return;
        }
        SWF swf = read(Path.of(a[1]));
        Path output = Path.of(a[2]);
        switch (a[0]) {
            case "snapshot": snapshot(swf, output); break;
            case "export": {
                Files.createDirectories(output);
                var settings = new ScriptExportSettings(ScriptExportMode.AS, false, false, false, false);
                Set<String> seen = new HashSet<>();
                for (ScriptPack pack : swf.getAS3Packs()) {
                    String cls = pack.getClassPath().toRawString();
                    if (!seen.add(cls)) throw new IOException("Duplicate script path " + cls);
                    Path file = output.resolve(cls.replace('.', '/') + ".as").normalize();
                    if (!file.startsWith(output)) throw new IOException("Invalid script path " + cls);
                    Files.createDirectories(file.getParent());
                    pack.export(swf.getAbcIndex(), file.toFile(), settings, false);
                    System.out.println(cls);
                }
                break;
            }
            case "compile": {
                if (a.length != 8) throw new IllegalArgumentException("compile INPUT OUTPUT CLASS SOURCE ffdec|flex SDK");
                ScriptPack found = null;
                for (ScriptPack pack : swf.getAS3Packs())
                    if (pack.getClassPath().toRawString().equals(a[3])) {
                        if (found != null) throw new IOException("Ambiguous script " + a[3]);
                        found = pack;
                    }
                if (found == null) throw new IOException("Missing script " + a[3]);
                Configuration.flexSdkLocation.set(a[7]);
                As3ScriptReplacerInterface compiler;
                if (a[6].equals("flex") || a[6].equals("flex-strict"))
                    compiler = new FlexCompiler(a[7], Path.of(a[5]).resolveSibling(Path.of(a[5]).getFileName() + "-flex"), a[6].equals("flex-strict"));
                else if (a[6].equals("ffdec")) compiler = As3ScriptReplacerFactory.createFFDec();
                else throw new IllegalArgumentException("Unknown compiler " + a[6]);
                try {
                    if (!found.abc.replaceScriptPack(compiler, found, Files.readString(Path.of(a[4])), List.of()))
                        throw new IOException("Compiler refused " + a[3]);
                    ((Tag)found.abc.parentTag).setModified(true);
                    try (OutputStream out = Files.newOutputStream(output)) { swf.saveTo(out); }
                } finally { compiler.deinitReplacement(found); }
                // Reparse persisted bytes so reports cover what was actually written.
                snapshot(read(output), Path.of(a[5]));
                break;
            }
            default: throw new IllegalArgumentException("Unknown operation " + a[0]);
        }
    }

    public static void main(String[] a) {
        try { run(a); System.exit(0); }
        catch (Exception error) { error.printStackTrace(); System.exit(1); }
    }
}

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
import com.jpexs.decompiler.flash.abc.avm2.AVM2Code;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
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
            qualifyDependencyInterfaces(swc, Set.of());
        }
        static void qualifyDependencyInterfaces(Path swc, Set<String> externalDefinitions) throws IOException, InterruptedException {
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
                definitions.addAll(externalDefinitions);
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
        static Set<String> externalDefinitions(Path library) throws IOException {
            Set<String> externalDefinitions = new HashSet<>();
            try (ZipFile api = new ZipFile(library.toFile())) {
                var factory = DocumentBuilderFactory.newInstance();
                factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
                var doc = factory.newDocumentBuilder().parse(api.getInputStream(api.getEntry("catalog.xml")));
                var definitions = doc.getElementsByTagName("def");
                for (int i = 0; i < definitions.getLength(); i++)
                    externalDefinitions.add(((org.w3c.dom.Element)definitions.item(i)).getAttribute("id"));
            } catch (Exception error) { throw new IOException("Cannot read player API definitions", error); }
            return externalDefinitions;
        }
        @Override public void mxmlc(List<String> args) throws MxmlcException, InterruptedException, IOException {
            Files.createDirectories(diagnostics);
            Set<String> externalDefinitions = externalDefinitions(sdk.resolve("frameworks/libs/player/32.0/playerglobal.swc"));
            for (int i = 0; i + 1 < args.size(); i++)
                if (args.get(i).equals("-library-path")) {
                    Path dependency = Path.of(args.get(i + 1));
                    Files.copy(dependency, diagnostics.resolve("dependency-" + i + "-original.swc"));
                    qualifyDependencyInterfaces(dependency, externalDefinitions);
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
                int output = args.indexOf("-output");
                if (output < 0 || output + 1 >= args.size()) throw new IOException("Missing native compiler output path");
                Files.copy(Path.of(args.get(output + 1)), diagnostics.resolve("native-compiler.swf"));
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

    static ScriptPack findScript(SWF movie, String path) throws IOException {
        ScriptPack found = null;
        for (ScriptPack pack : movie.getAS3Packs()) if (pack.getClassPath().toRawString().equals(path)) {
            if (found != null) throw new IOException("Ambiguous script " + path);
            found = pack;
        }
        if (found == null || !found.isSimple) throw new IOException("Missing or non-simple script " + path);
        return found;
    }

    static SWF importNative(Path originalFile, Path nativeFile, String cls) throws Exception {
        SWF original = read(originalFile), nativeMovie = read(nativeFile);
        ScriptPack source = findScript(original, cls);
        ABC destination = source.abc;
        var emittedTags = nativeMovie.getAbcList();
        if (emittedTags.isEmpty()) throw new IOException("Native compiler emitted no ABC");
        ABC emitted = emittedTags.get(emittedTags.size() - 1).getABC();
        ScriptPack replacement = findScript(nativeMovie, cls);
        if (replacement.abc != emitted || replacement.scriptIndex != 0 || emitted.script_info.size() != 1)
            throw new IOException("Native compiler's final ABC does not define the requested script");
        for (MethodBody body : emitted.bodies) exactCode(body);
        // Delete the replaced script, retaining other scripts and pool context.
        // Every replacement definition comes from the native compiler. Decode
        // retained bodies too before FFDec renumbers any of their references.
        for (MethodBody body : destination.bodies) exactCode(body);
        int classPosition = destination.class_info.size();
        ScriptInfo script = destination.script_info.get(source.scriptIndex);
        for (Trait trait : script.traits.traits)
            if (trait instanceof TraitClass c) classPosition = Math.min(classPosition, c.class_info);
        script.delete(destination, true);
        destination.pack();
        Map<Integer, Integer> classes = new HashMap<>(), scripts = new HashMap<>();
        destination.mergeABC(emitted, new HashMap<>(), new HashMap<>(), new HashMap<>(),
            new HashMap<>(), new HashMap<>(), new HashMap<>(), new HashMap<>(), new HashMap<>(),
            new HashMap<>(), new HashMap<>(), new HashMap<>(), new HashMap<>(), classes, new HashMap<>(), scripts);
        List<Integer> addedScripts = scripts.values().stream().sorted().toList();
        List<ScriptInfo> orderedScripts = new ArrayList<>();
        for (int index : addedScripts) orderedScripts.add(destination.script_info.get(index));
        for (int i = addedScripts.size() - 1; i >= 0; i--) destination.script_info.remove((int)addedScripts.get(i));
        destination.script_info.addAll(source.scriptIndex, orderedScripts);
        List<Integer> addedClasses = classes.values().stream().sorted().toList();
        List<Integer> order = new ArrayList<>();
        for (int i = 0; i < destination.class_info.size(); i++) if (!addedClasses.contains(i)) order.add(i);
        if (classPosition > order.size()) throw new IOException("Original class insertion position is unavailable");
        order.addAll(classPosition, addedClasses);
        Map<Integer, Integer> positions = new HashMap<>();
        for (int i = 0; i < order.size(); i++) positions.put(order.get(i), i);
        destination.reorganizeClasses(positions);
        ((Tag)destination.parentTag).setModified(true);
        return original;
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

    static Map<Integer, String> names(ABC abc) throws IOException {
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
        // A local function belongs to the newfunction site in its parent.
        // Pair sites by lexical occurrence, not compiler method numbers or
        // debugger names. Full parent/child byte comparisons still follow.
        Map<Integer, List<int[]>> incoming = new HashMap<>();
        for (MethodBody body : abc.bodies) {
            int ordinal = 0;
            for (var instruction : exactCode(body).code) if (instruction.definition.instructionCode == 0x40) {
                int child = instruction.operands[0];
                if (child < 0 || child >= abc.method_info.size()) throw new IOException("Invalid local function reference");
                incoming.computeIfAbsent(child, k -> new ArrayList<>()).add(new int[]{body.method_info, ordinal++});
            }
        }
        boolean changed;
        do {
            changed = false;
            for (var entry : incoming.entrySet()) {
                if (result.containsKey(entry.getKey()) || entry.getValue().size() != 1) continue;
                int[] site = entry.getValue().get(0);
                String parent = result.get(site[0]);
                if (parent == null) continue;
                result.put(entry.getKey(), parent + "/function[" + site[1] + "]");
                changed = true;
            }
        } while (changed);
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

    static List<List<String>> runtimeMetadata(ABC abc, Trait trait) {
        List<List<String>> result = new ArrayList<>();
        if ((trait.kindFlags & Trait.ATTR_Metadata) == 0) return result;
        for (int id : trait.metadata) {
            MetadataInfo info = abc.metadata_info.get(id);
            String name = abc.constants.getString(info.name_index);
            if (name.equals("__go_to_definition_help") || name.equals("__go_to_ctor_definition_help")) continue;
            // FFDec can append an identical metadata record under a new ID.
            // Compare the complete resolved record, preserving order and every
            // key/value, before reusing the original layout's metadata index.
            List<String> value = new ArrayList<>(List.of(name));
            for (int key : info.keys) value.add(abc.constants.getString(key));
            for (int text : info.values) value.add(abc.constants.getString(text));
            result.add(value);
        }
        return result;
    }

    static void retainDebugMetadata(ABC original, ABC compiled, Traits before, Traits after) throws IOException {
        if (before.traits.size() != after.traits.size()) throw new IOException("Trait count changed");
        Map<List<Object>, Trait> emitted = new HashMap<>();
        for (Trait trait : after.traits)
            if (emitted.put(List.of(trait.kindType, name(compiled, trait.name_index)), trait) != null)
                throw new IOException("Ambiguous compiled trait identity");
        List<Trait> ordered = new ArrayList<>();
        for (Trait trait : before.traits) {
            Trait found = emitted.remove(List.of(trait.kindType, name(original, trait.name_index)));
            if (found == null) throw new IOException("Trait identities changed");
            ordered.add(found);
        }
        after.traits.clear();
        after.traits.addAll(ordered);
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

    @FunctionalInterface interface InstructionRelocation { void apply(AVM2Instruction instruction) throws IOException; }

    static AVM2Code exactCode(MethodBody body) throws IOException {
        // FFDec's control-flow decoder rewrites unreachable jump operands even
        // on getCode(). Linking needs a linear, byte-preserving instruction
        // reader, including unreachable code. Reject unsupported encodings.
        byte[] bytes = body.getCodeBytes();
        ABCInputStream in = new ABCInputStream(new com.jpexs.helpers.MemoryInputStream(bytes));
        ArrayList<AVM2Instruction> instructions = new ArrayList<>();
        while (in.available() > 0) {
            long offset = in.getPosition();
            int opcode = in.readU8("opcode");
            var definition = AVM2Code.instructionSet[opcode];
            if (definition == null) throw new IOException("Unknown instruction opcode " + opcode);
            int[] operands;
            if (opcode == 0x1b) {
                int target = in.readS24("default"), cases = in.readU30("cases");
                if (cases < 0 || cases >= in.available() / 3) throw new IOException("Truncated switch table");
                operands = new int[cases + 3]; operands[0] = target; operands[1] = cases;
                for (int i = 2; i < operands.length; i++) operands[i] = in.readS24("case");
            } else {
                operands = new int[definition.operands.length];
                for (int i = 0; i < operands.length; i++) operands[i] = switch (definition.operands[i] & 0xff00) {
                    case AVM2Code.OPT_U30 -> in.readU30("operand");
                    case AVM2Code.OPT_U8 -> in.readU8("operand");
                    case AVM2Code.OPT_S8 -> (byte)in.readU8("operand");
                    case AVM2Code.OPT_S16 -> (short)in.readU30("operand");
                    case AVM2Code.OPT_S24 -> in.readS24("operand");
                    default -> throw new IOException("Unsupported instruction operand encoding");
                };
            }
            instructions.add(new AVM2Instruction(offset, definition, operands));
        }
        AVM2Code code = new AVM2Code(instructions);
        if (!Arrays.equals(bytes, code.getBytes())) throw new IOException("Instruction encoding changed during linear decoding");
        body.setCode(code);
        return code;
    }

    static int boundary(Map<Integer, Integer> positions, int offset) throws IOException {
        Integer index = positions.get(offset);
        if (index == null) throw new IOException("Control-flow offset is not an instruction boundary: " + offset);
        return index;
    }

    static void relocateCode(MethodBody body, InstructionRelocation relocation) throws IOException {
        var code = body.getCode();
        int size = code.code.size();
        int[] before = new int[size + 1], after = new int[size + 1];
        Map<Integer, Integer> positions = new HashMap<>();
        for (int i = 0; i < size; i++) before[i + 1] = Math.addExact(before[i], code.code.get(i).getBytesLength());
        for (int i = 0; i <= size; i++) positions.put(before[i], i);
        int[][] targets = new int[size][];
        for (int i = 0; i < size; i++) {
            var instruction = code.code.get(i);
            int opcode = instruction.definition.instructionCode;
            if (opcode == 0x1b) {
                if (instruction.operands.length < 3 || instruction.operands[1] != instruction.operands.length - 3)
                    throw new IOException("Malformed lookupswitch operands");
                targets[i] = new int[instruction.operands.length];
                for (int j = 0; j < instruction.operands.length; j++) if (j != 1)
                    targets[i][j] = boundary(positions, before[i] + instruction.operands[j]);
            } else if (opcode >= 0x0c && opcode <= 0x1a) {
                if (instruction.operands.length != 1) throw new IOException("Malformed branch operands");
                targets[i] = new int[]{boundary(positions, before[i + 1] + instruction.operands[0])};
            } else if (instruction.isBranch() || instruction.isJump()) throw new IOException("Unsupported control-flow opcode");
        }
        int[][] handlers = new int[body.exceptions.length][3];
        for (int i = 0; i < handlers.length; i++) {
            var handler = body.exceptions[i];
            handlers[i] = new int[]{boundary(positions, handler.start), boundary(positions, handler.end), boundary(positions, handler.target)};
        }
        for (int i = 0; i < size; i++) {
            relocation.apply(code.code.get(i));
            after[i + 1] = Math.addExact(after[i], code.code.get(i).getBytesLength());
        }
        for (int i = 0; i < size; i++) if (targets[i] != null) {
            var instruction = code.code.get(i);
            boolean table = instruction.definition.instructionCode == 0x1b;
            for (int j = 0; j < targets[i].length; j++) {
                if (table && j == 1) continue;
                int offset = after[targets[i][j]] - (table ? after[i] : after[i + 1]);
                if (offset < -0x800000 || offset > 0x7fffff) throw new IOException("Relocated branch exceeds signed 24-bit range");
                instruction.operands[j] = offset;
            }
        }
        for (int i = 0; i < handlers.length; i++) {
            var handler = body.exceptions[i];
            handler.start = after[handlers[i][0]]; handler.end = after[handlers[i][1]]; handler.target = after[handlers[i][2]];
        }
        code.markOffsets();
        body.setCode(code);
    }

    static String privateRole(ABC abc, int index, String className, String simpleName) {
        Namespace ns = abc.constants.getNamespace(index);
        if (ns.kind != 5) return null;
        String text = abc.constants.getString(ns.name_index);
        if (text.equals(className)) return "class";
        if (text.matches("\\$[0-9]+") || text.matches(java.util.regex.Pattern.quote(simpleName + ".as$") + "[0-9]+"))
            return "script";
        return null;
    }

    static Map<Integer, Integer> namespaceLayout(ABC original, ABC compiled) throws IOException {
        return namespaceLayout(original, compiled, null);
    }

    static boolean namespaceValue(Traits traits, int namespace) {
        for (Trait trait : traits.traits)
            if (trait instanceof TraitSlotConst slot && slot.isNamespace() && slot.value_index == namespace) return true;
        return false;
    }

    static boolean namespaceDeclaration(ABC abc, Traits traits, int namespace) {
        if (namespaceValue(traits, namespace)) return true;
        for (var trait : traits.traits) {
            Set<Integer> used = new HashSet<>();
            nameNamespaces(abc, trait.name_index, used, new HashSet<>());
            if (used.contains(namespace)) return true;
        }
        return false;
    }

    // A file namespace with no qualified names or namespace values cannot
    // carry a private binding. Require a public namespace in every containing
    // set, so dynamic property creation still uses the public name.
    static boolean emptyFileNamespace(ABC abc, int namespace) {
        for (int i = 1; i < abc.constants.getMultinameCount(); i++) {
            var name = abc.constants.getMultiname(i);
            if (name.hasOwnNamespace() && name.namespace_index == namespace) return false;
        }
        for (int i = 1; i < abc.constants.getNamespaceSetCount(); i++) {
            boolean contains = false, publicName = false;
            for (int member : abc.constants.getNamespaceSet(i).namespaces) {
                contains |= member == namespace;
                var ns = abc.constants.getNamespace(member);
                publicName |= ns.kind == 0x16 && abc.constants.getString(ns.name_index).isEmpty();
            }
            if (contains && !publicName) return false;
        }
        for (var info : abc.method_info) for (var option : info.optional)
            if (option.isNamespace() && option.value_index == namespace) return false;
        for (var info : abc.instance_info) {
            Set<Integer> declared = new HashSet<>();
            nameNamespaces(abc, info.name_index, declared, new HashSet<>());
            if (declared.contains(namespace) || info.protectedNS == namespace
                    || namespaceDeclaration(abc, info.instance_traits, namespace)) return false;
        }
        for (var info : abc.class_info) if (namespaceDeclaration(abc, info.static_traits, namespace)) return false;
        for (var info : abc.script_info) if (namespaceDeclaration(abc, info.traits, namespace)) return false;
        for (var body : abc.bodies) {
            if (namespaceDeclaration(abc, body.traits, namespace)) return false;
            for (var instruction : body.getCode().code)
                for (int i = 0; i < instruction.definition.operands.length; i++)
                    if (instruction.definition.operands[i] == AVM2Code.DAT_NAMESPACE_INDEX
                            && instruction.operands[i] == namespace) return false;
        }
        return true;
    }

    static Map<Integer, Integer> namespaceLayout(ABC original, ABC compiled,
            Map<Integer, List<Integer>> perMethod) throws IOException {
        Map<Integer, Integer> mapping = new HashMap<>();
        int retained = original.constants.getNamespaceCount();
        for (int i = 0; i < retained; i++) mapping.put(i, i);
        // Class/script private namespaces are lexical identities, not strings.
        // Compiler-generated class and file namespace roles must each be
        // unique in the old pool and among newly emitted namespaces. Retained
        // classes can share this ABC, but class identity/order must be fixed.
        if (original.instance_info.size() == compiled.instance_info.size()) {
          for (int owner = 0; owner < original.instance_info.size(); owner++) {
            int nameId = original.instance_info.get(owner).name_index;
            Multiname name = original.constants.getMultiname(nameId);
            if (compiled.instance_info.get(owner).name_index == nameId && name.kind == 7
                    && original.constants.getNamespace(name.namespace_index).kind == 0x16) {
                String simple = original.constants.getString(name.name_index);
                String pkg = original.constants.getString(original.constants.getNamespace(name.namespace_index).name_index);
                String qualified = pkg.isEmpty() ? simple : pkg + ":" + simple;
                for (String role : List.of("class", "script")) {
                    List<Integer> before = new ArrayList<>(), after = new ArrayList<>();
                    for (int i = 1; i < retained; i++)
                        if (role.equals(privateRole(original, i, qualified, simple))) before.add(i);
                    for (int i = retained; i < compiled.constants.getNamespaceCount(); i++)
                        if (role.equals(privateRole(compiled, i, qualified, simple))) after.add(i);
                    if (after.isEmpty()) continue;
                    if (perMethod != null && role.equals("script") && before.size() > 1 && after.size() == 1
                            && before.stream().allMatch(ns -> emptyFileNamespace(original, ns))
                            && emptyFileNamespace(compiled, after.get(0))) {
                        if (perMethod.putIfAbsent(after.get(0), before) != null)
                            throw new IOException("File namespace belongs to multiple classes");
                        continue;
                    }
                    if (before.size() != 1 || after.size() != 1)
                        throw new IOException("Ambiguous private namespace owner: " + role);
                    mapping.put(after.get(0), before.get(0));
                }
            }
          }
        }
        return mapping;
    }

    static void nameNamespaces(ABC abc, int index, Set<Integer> namespaces, Set<Integer> visited) {
        if (index == 0 || !visited.add(index)) return;
        var name = abc.constants.getMultiname(index);
        if (name.hasOwnNamespace()) namespaces.add(name.namespace_index);
        if (name.hasOwnNamespaceSet())
            for (int ns : abc.constants.getNamespaceSet(name.namespace_set_index).namespaces) namespaces.add(ns);
        if (name.kind == 0x1d) {
            nameNamespaces(abc, name.qname_index, namespaces, visited);
            for (int param : name.params) nameNamespaces(abc, param, namespaces, visited);
        }
    }

    static Set<Integer> bodyNamespaces(ABC abc, MethodBody body) {
        Set<Integer> namespaces = new HashSet<>(), visited = new HashSet<>();
        for (var instruction : body.getCode().code)
            for (int i = 0; i < instruction.definition.operands.length; i++)
                if (instruction.definition.operands[i] == AVM2Code.DAT_MULTINAME_INDEX)
                    nameNamespaces(abc, instruction.operands[i], namespaces, visited);
        return namespaces;
    }

    static List<Object> multinameKey(ABC abc, int index, Map<Integer, Integer> namespaces,
            Map<Integer, List<Object>> memo, Set<Integer> visiting) throws IOException {
        if (index == 0) return List.of("wildcard");
        if (memo.containsKey(index)) return memo.get(index);
        if (!visiting.add(index)) throw new IOException("Cyclic type-name reference");
        Multiname m = abc.constants.getMultiname(index);
        List<Object> key = new ArrayList<>(List.of(m.kind));
        if (m.hasOwnName()) key.add(List.of(m.name_index == 0, m.name_index == 0 ? "" : abc.constants.getString(m.name_index)));
        if (m.hasOwnNamespace()) key.add(namespaces.getOrDefault(m.namespace_index, m.namespace_index));
        if (m.hasOwnNamespaceSet()) {
            // AVM2 resolves any matching namespace, raising an ambiguity error
            // for multiple matches. Set order is not lookup precedence.
            key.add(Arrays.stream(abc.constants.getNamespaceSet(m.namespace_set_index).namespaces)
                .map(i -> namespaces.getOrDefault(i, i)).sorted().boxed().toList());
        }
        if (m.kind == 0x1d) {
            key.add(multinameKey(abc, m.qname_index, namespaces, memo, visiting));
            List<Object> params = new ArrayList<>();
            for (int param : m.params) params.add(multinameKey(abc, param, namespaces, memo, visiting));
            key.add(params);
        }
        visiting.remove(index);
        memo.put(index, key);
        return key;
    }

    static void relocateTraitNames(Traits traits, Map<Integer, Integer> names, Map<Integer, Integer> namespaces) {
        for (Trait trait : traits.traits) {
            trait.name_index = names.getOrDefault(trait.name_index, trait.name_index);
            if (trait instanceof TraitSlotConst slot) {
                slot.type_index = names.getOrDefault(slot.type_index, slot.type_index);
                if (slot.value_index != 0 && slot.isNamespace())
                    slot.value_index = namespaces.getOrDefault(slot.value_index, slot.value_index);
            }
        }
    }

    static void restoreNameReferences(ABC original, ABC compiled) throws IOException {
        Map<Integer, List<Integer>> perMethod = new HashMap<>();
        Map<Integer, Integer> namespaces = namespaceLayout(original, compiled, perMethod), names = new HashMap<>();
        Map<String, MethodBody> originalBodies = new HashMap<>();
        if (!perMethod.isEmpty()) {
            var originalOwners = names(original);
            for (var body : original.bodies) {
                String owner = originalOwners.get(body.method_info);
                if (owner != null && originalBodies.put(owner, body) != null) throw new IOException("Ambiguous original method owner");
            }
        }
        Map<List<Object>, List<Integer>> definitions = new HashMap<>();
        Map<Integer, List<Object>> oldKeys = new HashMap<>(), newKeys = new HashMap<>();
        int retained = original.constants.getMultinameCount();
        for (int i = 0; i < retained; i++) {
            names.put(i, i);
            definitions.computeIfAbsent(multinameKey(original, i, Map.of(), oldKeys, new HashSet<>()), k -> new ArrayList<>()).add(i);
        }
        for (int i = retained; i < compiled.constants.getMultinameCount(); i++) {
            var candidates = definitions.get(multinameKey(compiled, i, namespaces, newKeys, new HashSet<>()));
            if (candidates != null && candidates.size() == 1) names.put(i, candidates.get(0));
        }
        for (MethodInfo method : compiled.method_info) {
            method.ret_type = names.getOrDefault(method.ret_type, method.ret_type);
            for (int i = 0; i < method.param_types.length; i++)
                method.param_types[i] = names.getOrDefault(method.param_types[i], method.param_types[i]);
            for (ValueKind option : method.optional) if (option.isNamespace())
                option.value_index = namespaces.getOrDefault(option.value_index, option.value_index);
        }
        for (InstanceInfo ins : compiled.instance_info) {
            ins.name_index = names.getOrDefault(ins.name_index, ins.name_index);
            ins.super_index = names.getOrDefault(ins.super_index, ins.super_index);
            ins.protectedNS = namespaces.getOrDefault(ins.protectedNS, ins.protectedNS);
            for (int i = 0; i < ins.interfaces.length; i++) ins.interfaces[i] = names.getOrDefault(ins.interfaces[i], ins.interfaces[i]);
            relocateTraitNames(ins.instance_traits, names, namespaces);
        }
        for (ClassInfo cls : compiled.class_info) relocateTraitNames(cls.static_traits, names, namespaces);
        for (ScriptInfo script : compiled.script_info) relocateTraitNames(script.traits, names, namespaces);
        Map<Integer, String> methodOwners = perMethod.isEmpty() ? Map.of() : names(compiled);
        for (MethodBody body : compiled.bodies) {
            Map<Integer, Integer> bodyNames = perMethod.isEmpty() ? names : new HashMap<>(names);
            Set<Integer> used = perMethod.isEmpty() ? Set.of() : bodyNamespaces(compiled, body);
            Map<Integer, Integer> localNamespaces = new HashMap<>(namespaces);
            for (var entry : perMethod.entrySet()) if (used.contains(entry.getKey())) {
                MethodBody prior = originalBodies.get(methodOwners.get(body.method_info));
                if (prior == null) throw new IOException("Unowned method has an ambiguous file namespace");
                var oldUsed = bodyNamespaces(original, prior);
                var candidates = entry.getValue().stream().filter(oldUsed::contains).toList();
                if (candidates.size() != 1) throw new IOException("Method has no unique original file namespace");
                localNamespaces.put(entry.getKey(), candidates.get(0));
            }
            if (!localNamespaces.equals(namespaces)) {
                Map<Integer, List<Object>> localKeys = new HashMap<>();
                for (int i = retained; i < compiled.constants.getMultinameCount(); i++) {
                    var candidates = definitions.get(multinameKey(compiled, i, localNamespaces, localKeys, new HashSet<>()));
                    if (candidates != null && candidates.size() == 1) bodyNames.put(i, candidates.get(0));
                }
            }
            relocateTraitNames(body.traits, names, namespaces);
            for (ABCException handler : body.exceptions) {
                handler.type_index = names.getOrDefault(handler.type_index, handler.type_index);
                handler.name_index = names.getOrDefault(handler.name_index, handler.name_index);
            }
            relocateCode(body, instruction -> {
                for (int i = 0; i < instruction.definition.operands.length; i++) {
                    int type = instruction.definition.operands[i];
                    Map<Integer, Integer> map = type == AVM2Code.DAT_MULTINAME_INDEX ? bodyNames
                        : type == AVM2Code.DAT_NAMESPACE_INDEX ? namespaces : Map.of();
                    if (!map.isEmpty()) instruction.operands[i] = map.getOrDefault(instruction.operands[i], instruction.operands[i]);
                }
            });
        }
    }

    static void relocateAddedPoolNames(ABC original, ABC compiled) throws IOException {
        Map<Integer, Integer> namespaces = namespaceLayout(original, compiled);
        Map<Integer, List<Object>> oldKeys = new HashMap<>(), newKeys = new HashMap<>();
        Map<List<Object>, List<Integer>> definitions = new HashMap<>();
        Map<Integer, Integer> names = new HashMap<>();
        for (int i = 0; i < original.constants.getMultinameCount(); i++)
            definitions.computeIfAbsent(multinameKey(original, i, Map.of(), oldKeys, new HashSet<>()), k -> new ArrayList<>()).add(i);
        for (int i = original.constants.getMultinameCount(); i < compiled.constants.getMultinameCount(); i++) {
            var candidates = definitions.get(multinameKey(compiled, i, namespaces, newKeys, new HashSet<>()));
            if (candidates != null && candidates.size() == 1) names.put(i, candidates.get(0));
        }
        // A newly introduced lookup must use the same lexical namespaces as
        // the recompiled class. Mutate appended pool entries only; retaining a
        // fresh private namespace here would silently change name resolution.
        for (int i = original.constants.getNamespaceSetCount(); i < compiled.constants.getNamespaceSetCount(); i++) {
            int[] members = compiled.constants.getNamespaceSet(i).namespaces;
            compiled.constants.setNamespaceSet(i, new NamespaceSet(Arrays.stream(members)
                .map(n -> namespaces.getOrDefault(n, n)).toArray()));
        }
        for (int i = original.constants.getMultinameCount(); i < compiled.constants.getMultinameCount(); i++) {
            var m = compiled.constants.getMultiname(i);
            if (m.hasOwnNamespace()) m.namespace_index = namespaces.getOrDefault(m.namespace_index, m.namespace_index);
            if (m.kind == 0x1d) {
                m.qname_index = names.getOrDefault(m.qname_index, m.qname_index);
                for (int j = 0; j < m.params.length; j++) m.params[j] = names.getOrDefault(m.params[j], m.params[j]);
            }
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
            relocateCode(body, instruction -> {
                for (int j = 0; j < instruction.definition.operands.length; j++)
                    if (instruction.definition.operands[j] == com.jpexs.decompiler.flash.abc.avm2.AVM2Code.DAT_METHOD_INDEX) {
                        Integer index = mapping.get(instruction.operands[j]);
                        if (index == null) throw new IOException("Unmapped method instruction operand");
                        instruction.operands[j] = index;
                    }
            });
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

    // Recognize only the straight-line class-definition initializer emitted by
    // ASC. Script entry uses its global object, containing the class trait.
    // The layout emitter can express that class binding as a property lookup.
    // This is an explicit code-generation rule, not body substitution.
    static boolean classInitializer(ABC abc, ScriptInfo script, MethodBody body) {
        if (script.traits.traits.size() != 1 || !(script.traits.traits.get(0) instanceof TraitClass trait)
                || body == null || body.init_scope_depth < 0 || body.init_scope_depth > 1 || body.max_regs != 1
                || body.exceptions.length != 0 || !body.traits.traits.isEmpty()) return false;
        var code = body.getCode().code;
        int n = code.size();
        if (n < 7 || code.get(0).definition.instructionCode != 0xd0
                || code.get(1).definition.instructionCode != 0x30) return false;
        var binding = code.get(2);
        if (binding.definition.instructionCode != 0x65 || binding.operands[0] != 0) return false;
        if (abc.instance_info.get(trait.class_info).isInterface())
            return n == 7 && code.get(3).definition.instructionCode == 0x20
                && code.get(4).definition.instructionCode == 0x58 && code.get(4).operands[0] == trait.class_info
                && code.get(5).definition.instructionCode == 0x68 && code.get(5).operands[0] == trait.name_index
                && code.get(6).definition.instructionCode == 0x47;
        int i = 3, scopes = 0;
        while (i + 2 < n && code.get(i).definition.instructionCode == 0x5d
                && code.get(i + 1).definition.instructionCode == 0x66
                && code.get(i).operands[0] == code.get(i + 1).operands[0]
                && code.get(i + 2).definition.instructionCode == 0x30) { i += 3; scopes++; }
        if (scopes == 0 || i + 2 >= n || code.get(i).definition.instructionCode != 0x5d
                || code.get(i + 1).definition.instructionCode != 0x66
                || code.get(i).operands[0] != code.get(i + 1).operands[0]
                || code.get(i + 2).definition.instructionCode != 0x58
                || code.get(i + 2).operands[0] != trait.class_info) return false;
        i += 3;
        for (int j = 0; j < scopes; j++, i++)
            if (i >= n || code.get(i).definition.instructionCode != 0x1d) return false;
        return i + 2 == n && code.get(i).definition.instructionCode == 0x68
            && code.get(i).operands[0] == trait.name_index && code.get(i + 1).definition.instructionCode == 0x47;
    }

    static void emitScriptClassBindings(ABC original, ABC compiled) throws IOException {
        if (original.script_info.size() != compiled.script_info.size()) throw new IOException("Script count changed");
        for (int i = 0; i < compiled.script_info.size(); i++) {
            ScriptInfo script = compiled.script_info.get(i);
            MethodBody body = compiled.findBody(script.init_index);
            if (!classInitializer(compiled, script, body)) continue;
            MethodBody reference = original.findBody(original.script_info.get(i).init_index);
            if (reference == null) continue;
            var executable = reference.getCode().code.stream().filter(ins -> !debugInstruction(ins)).toList();
            if (executable.size() < 3) continue;
            var layout = executable.get(2);
            if (layout.definition.instructionCode != 0x5d) continue;
            int index = layout.operands[0];
            Multiname binding = original.constants.getMultiname(index);
            Multiname declared = compiled.constants.getMultiname(script.traits.traits.get(0).name_index);
            // A singleton public package namespace is unambiguous. Private,
            // protected, runtime and multi-namespace bindings are unsupported.
            if (binding.kind != 0x09 || declared.kind != 0x07 || binding.name_index != declared.name_index) continue;
            int[] namespaces = original.constants.getNamespaceSet(binding.namespace_set_index).namespaces;
            if (namespaces.length != 1 || namespaces[0] != declared.namespace_index
                    || compiled.constants.getNamespace(declared.namespace_index).kind != 0x16) continue;
            var code = body.getCode();
            code.code.set(2, new AVM2Instruction(code.code.get(2).getAddress(), 0x5d, new int[]{index}));
            // The recognized body has no branches or exception offsets.
            code.markOffsets();
            body.setCode(code);
        }
    }

    static void restoreScriptSlot(ABC original, ABC compiled) {
        for (int i = 0; i < compiled.script_info.size(); i++) {
            var before = original.script_info.get(i).traits.traits;
            var after = compiled.script_info.get(i).traits.traits;
            // With exactly one script trait and no script inheritance, the
            // automatically assigned slot (0) is slot 1. Other slot layouts
            // require a full slot map and are deliberately left unchanged.
            if (before.size() == 1 && after.size() == 1 && before.get(0) instanceof TraitClass a
                    && after.get(0) instanceof TraitClass b && a.name_index == b.name_index
                    && a.class_info == b.class_info && a.slot_id == 0 && b.slot_id == 1) b.slot_id = 0;
            if (before.size() == 1 && after.size() == 1 && before.get(0) instanceof TraitSlotConst a
                    && after.get(0) instanceof TraitSlotConst b && a.name_index == b.name_index
                    && a.kindType == b.kindType && a.type_index == b.type_index
                    && a.value_index == b.value_index && a.value_kind == b.value_kind
                    && a.slot_id == 0 && b.slot_id == 1) b.slot_id = 0;
        }
    }

    static boolean debugInstruction(AVM2Instruction instruction) {
        int opcode = instruction.definition.instructionCode;
        return opcode == 0xef || opcode == 0xf0 || opcode == 0xf1;
    }

    static int comparisonBranch(int opcode, boolean whenTrue) {
        // Negated ordered comparisons must keep NaN unordered: !(a < b)
        // uses ifnlt, not ifge. Operand order is unchanged for all six pairs.
        return switch (opcode) {
            case 0xab -> whenTrue ? 0x13 : 0x14; // equals
            case 0xac -> whenTrue ? 0x19 : 0x1a; // strictequals
            case 0xad -> whenTrue ? 0x15 : 0x0c; // lessthan
            case 0xae -> whenTrue ? 0x16 : 0x0d; // lessequals
            case 0xaf -> whenTrue ? 0x17 : 0x0e; // greaterthan
            case 0xb0 -> whenTrue ? 0x18 : 0x0f; // greaterequals
            default -> -1;
        };
    }

    // ASC Encoder.OP_getproperty/OP_pop and CodeGenerator.If select these
    // compact encodings. Comparison semantics also follow Adobe's interpreter:
    // https://github.com/adobe/avmplus/blob/master/core/Interpreter.cpp
    // https://github.com/apache/flex-sdk/blob/1644730bd54105c8aab234af7c1a08b2af17865d/modules/asc/src/java/macromedia/abc/Encoder.java
    // Mixed-tool movies can contain both encodings in one method. Match the
    // entire method and its control-flow graph before selecting the shorter
    // encoding; never transfer executable instructions from the reference.
    static void emitCompactOperations(ABC original, ABC compiled) throws IOException {
        for (MethodBody body : compiled.bodies) {
            MethodBody reference = original.findBody(body.method_info);
            if (reference == null || reference.exceptions.length != body.exceptions.length) continue;
            var layout = reference.getCode().code;
            var expected = layout.stream().filter(i -> !debugInstruction(i)).toList();
            var emitted = body.getCode().code;
            if (emitted.stream().anyMatch(MovieSource::debugInstruction)) continue;
            ArrayList<AVM2Instruction> result = new ArrayList<>();
            List<int[]> targets = new ArrayList<>();
            Map<Integer, Integer> positions = new HashMap<>();
            boolean matching = true, rewritten = false;
            int next = 0, end = 0;
            for (int wantedIndex = 0; wantedIndex < expected.size(); wantedIndex++) {
                var wanted = expected.get(wantedIndex);
                if (next == emitted.size()) { matching = false; break; }
                var instruction = emitted.get(next);
                var operandSource = instruction;
                int opcode = instruction.definition.instructionCode, desired = wanted.definition.instructionCode;
                // ASC can use either a combined comparison branch or a Boolean
                // comparison followed by a branch, including within one method.
                // Generate the latter only from an equivalent combined opcode.
                // Its intermediate boundaries have no emitted input counterpart;
                // the complete target/handler checks below reject entry to them.
                if (opcode != desired && desired >= 0xab && desired <= 0xb0) {
                    int branch = wantedIndex + 1;
                    boolean negate = branch < expected.size()
                        && expected.get(branch).definition.instructionCode == 0x96;
                    if (negate) branch++;
                    boolean convert = branch < expected.size()
                        && expected.get(branch).definition.instructionCode == 0x76;
                    if (convert) branch++;
                    int jumpOpcode = branch < expected.size() ? expected.get(branch).definition.instructionCode : -1;
                    if ((jumpOpcode == 0x11 || jumpOpcode == 0x12)
                            && comparisonBranch(desired, (jumpOpcode == 0x11) != negate) == opcode) {
                        positions.put((int)instruction.getAddress(), result.size());
                        result.add(new AVM2Instruction(0, desired, null));
                        targets.add(new int[0]);
                        if (negate) {
                            result.add(new AVM2Instruction(0, 0x96, null));
                            targets.add(new int[0]);
                        }
                        if (convert) {
                            result.add(new AVM2Instruction(0, 0x76, null));
                            targets.add(new int[0]);
                        }
                        result.add(new AVM2Instruction(0, jumpOpcode, instruction.operands.clone()));
                        targets.add(branchTargets(instruction));
                        end = (int)instruction.getAddress() + instruction.getBytesLength();
                        next++;
                        wantedIndex = branch;
                        rewritten = true;
                        continue;
                    }
                }
                int consumed = 1;
                if (opcode != desired) {
                    if (next + 1 >= emitted.size()) { matching = false; break; }
                    var following = emitted.get(next + 1);
                    boolean lookup = desired == 0x60 && opcode == 0x5d
                        && following.definition.instructionCode == 0x66
                        && Arrays.equals(instruction.operands, following.operands)
                        && instruction.operands.length == 1
                        && compiled.constants.getMultiname(instruction.operands[0]).kind == 0x07;
                    boolean discardedCall = desired == 0x4f && opcode == 0x46
                        && following.definition.instructionCode == 0x29;
                    if (lookup || discardedCall) consumed = 2;
                    else {
                        // A comparison produces a Boolean. Optional logical
                        // negation and Boolean conversion cannot change its
                        // coercion side effects or the order of its operands.
                        int branch = next + 1;
                        boolean negate = false;
                        if (emitted.get(branch).definition.instructionCode == 0x96) {
                            negate = true; branch++;
                        }
                        if (branch < emitted.size() && emitted.get(branch).definition.instructionCode == 0x76) branch++;
                        if (branch >= emitted.size()) { matching = false; break; }
                        var jump = emitted.get(branch);
                        int jumpOpcode = jump.definition.instructionCode;
                        if ((jumpOpcode != 0x11 && jumpOpcode != 0x12)
                                || comparisonBranch(opcode, (jumpOpcode == 0x11) != negate) != desired) {
                            matching = false; break;
                        }
                        consumed = branch - next + 1;
                        operandSource = jump;
                    }
                    rewritten = true;
                }
                int[] branches = branchTargets(operandSource);
                if (branches.length == 0 && !Arrays.equals(wanted.operands, operandSource.operands)) {
                    matching = false; break;
                }
                positions.put((int)instruction.getAddress(), result.size());
                result.add(new AVM2Instruction(0, desired, operandSource.operands == null ? null : operandSource.operands.clone()));
                targets.add(branches);
                var last = emitted.get(next + consumed - 1);
                end = (int)last.getAddress() + last.getBytesLength();
                next += consumed;
            }
            if (!matching || !rewritten || next != emitted.size()) continue;
            positions.put(end, result.size());
            var oldPositions = executablePositions(layout);
            // The interior of a folded group has no output boundary. Branches,
            // switch entries and exception boundaries may not enter it.
            for (int i = 0; i < result.size(); i++) {
                int[] a = branchTargets(expected.get(i)), b = targets.get(i);
                if (a.length != b.length) { matching = false; break; }
                for (int j = 0; j < a.length; j++) {
                    Integer target = positions.get(b[j]);
                    if (target == null || !target.equals(oldPositions.get(a[j]))) matching = false;
                }
            }
            for (int i = 0; i < body.exceptions.length; i++) {
                var a = reference.exceptions[i]; var b = body.exceptions[i];
                if (a.type_index != b.type_index || a.name_index != b.name_index) matching = false;
                int[] oldBounds = {a.start, a.end, a.target}, newBounds = {b.start, b.end, b.target};
                for (int j = 0; j < 3; j++) {
                    Integer target = positions.get(newBounds[j]);
                    if (target == null || !target.equals(oldPositions.get(oldBounds[j]))) matching = false;
                }
            }
            if (!matching) continue;
            var code = new AVM2Code(result);
            code.markOffsets();
            int[] offsets = new int[result.size() + 1];
            for (int i = 0; i < result.size(); i++) offsets[i + 1] = offsets[i] + result.get(i).getBytesLength();
            for (int i = 0; i < result.size(); i++) {
                var instruction = result.get(i);
                boolean table = instruction.definition.instructionCode == 0x1b;
                int[] branches = targets.get(i);
                for (int j = 0; j < branches.length; j++) {
                    int offset = offsets[positions.get(branches[j])] - offsets[i + (table ? 0 : 1)];
                    if (offset < -0x800000 || offset > 0x7fffff) throw new IOException("Folded branch exceeds signed 24-bit range");
                    instruction.operands[table && j > 0 ? j + 1 : j] = offset;
                }
            }
            for (var handler : body.exceptions) {
                handler.start = offsets[positions.get(handler.start)];
                handler.end = offsets[positions.get(handler.end)];
                handler.target = offsets[positions.get(handler.target)];
            }
            body.setCode(code);
        }
    }

    static int[] branchTargets(AVM2Instruction instruction) {
        int opcode = instruction.definition.instructionCode, address = (int)instruction.getAddress();
        if (opcode >= 0x0c && opcode <= 0x1a)
            return new int[]{address + instruction.getBytesLength() + instruction.operands[0]};
        if (opcode != 0x1b) return new int[0];
        int[] targets = new int[instruction.operands.length - 1];
        targets[0] = address + instruction.operands[0];
        for (int i = 1; i < targets.length; i++) targets[i] = address + instruction.operands[i + 1];
        return targets;
    }

    static Map<Integer, Integer> executablePositions(List<AVM2Instruction> code) {
        Map<Integer, Integer> positions = new HashMap<>();
        int executable = 0, end = 0;
        for (var instruction : code) {
            positions.put((int)instruction.getAddress(), executable);
            if (!debugInstruction(instruction)) executable++;
            end = (int)instruction.getAddress() + instruction.getBytesLength();
        }
        positions.put(end, executable);
        return positions;
    }

    static boolean sameTargets(int[] before, int[] after, Map<Integer, Integer> oldPositions,
            Map<Integer, Integer> newPositions) throws IOException {
        if (before.length != after.length) return false;
        for (int i = 0; i < before.length; i++)
            if (boundary(oldPositions, before[i]) != boundary(newPositions, after[i])) return false;
        return true;
    }

    static void retainDebugDirectives(ABC original, ABC compiled) throws IOException {
        for (MethodBody body : compiled.bodies) {
            MethodBody before = original.findBody(body.method_info);
            if (before == null || before.exceptions.length != body.exceptions.length) continue;
            var emitted = body.getCode().code;
            var layout = before.getCode().code;
            if (emitted.stream().noneMatch(MovieSource::debugInstruction)
                    && layout.stream().noneMatch(MovieSource::debugInstruction)) continue;
            var executable = layout.stream().filter(i -> !debugInstruction(i)).toList();
            var compiledExecutable = emitted.stream().filter(i -> !debugInstruction(i)).toList();
            if (executable.isEmpty() || executable.size() != compiledExecutable.size()) continue;
            var oldPositions = executablePositions(layout);
            var newPositions = executablePositions(emitted);
            boolean identical = true;
            for (int i = 0; i < compiledExecutable.size(); i++) {
                var a = executable.get(i); var b = compiledExecutable.get(i);
                if (a.definition.instructionCode != b.definition.instructionCode) { identical = false; break; }
                int[] aTargets = branchTargets(a), bTargets = branchTargets(b);
                if (aTargets.length == 0) {
                    if (!Arrays.equals(a.getBytes(), b.getBytes())) identical = false;
                } else if (!sameTargets(aTargets, bTargets, oldPositions, newPositions)) identical = false;
            }
            for (int i = 0; i < body.exceptions.length; i++) {
                var a = before.exceptions[i]; var b = body.exceptions[i];
                if (a.type_index != b.type_index || a.name_index != b.name_index
                        || !sameTargets(new int[]{a.start, a.end, a.target}, new int[]{b.start, b.end, b.target}, oldPositions, newPositions))
                    identical = false;
            }
            if (!identical) continue;
            // Only debugger directives come from the reference's source map.
            // Every executable instruction is the actual compiler instruction,
            // already checked above, including every control-flow target and
            // handler boundary expressed without debugger-only instructions.
            List<AVM2Instruction> merged = new ArrayList<>();
            int next = 0;
            for (var instruction : layout) {
                if (debugInstruction(instruction)) merged.add(new AVM2Instruction(0,
                    instruction.definition.instructionCode, instruction.operands.clone()));
                else merged.add(compiledExecutable.get(next++));
            }
            var code = body.getCode();
            code.code = merged;
            code.markOffsets();
            Map<Integer, Integer> placed = new HashMap<>();
            for (int i = 0; i < layout.size(); i++) placed.put((int)layout.get(i).getAddress(), (int)merged.get(i).getAddress());
            int oldEnd = (int)layout.get(layout.size() - 1).getAddress() + layout.get(layout.size() - 1).getBytesLength();
            int newEnd = (int)merged.get(merged.size() - 1).getAddress() + merged.get(merged.size() - 1).getBytesLength();
            placed.put(oldEnd, newEnd);
            for (int i = 0; i < layout.size(); i++) {
                int[] targets = branchTargets(layout.get(i));
                var instruction = merged.get(i);
                boolean table = instruction.definition.instructionCode == 0x1b;
                for (int j = 0; j < targets.length; j++) {
                    int offset = boundary(placed, targets[j]) - (int)instruction.getAddress() - (table ? 0 : instruction.getBytesLength());
                    if (offset < -0x800000 || offset > 0x7fffff) throw new IOException("Source-map branch exceeds signed 24-bit range");
                    instruction.operands[table && j > 0 ? j + 1 : j] = offset;
                }
            }
            for (int i = 0; i < body.exceptions.length; i++) {
                var a = before.exceptions[i]; var b = body.exceptions[i];
                b.start = boundary(placed, a.start); b.end = boundary(placed, a.end); b.target = boundary(placed, a.target);
            }
            body.setCode(code);
        }
    }

    // Conservative first layout linker. It never imports an original method
    // body. Executable records and class/script definitions come from the
    // compiler, with the class-binding and compact-operation rules above. Existing
    // pool entries must retain their original indices and
    // values; unused appended constants, compiler debug names and Adobe editor
    // navigation metadata follow the original layout. A byte comparison of the
    // entire ABC must then pass. Runtime metadata is never silently restored.
    static void linkLayout(Path referenceFile, Path candidateFile, Path output, String cls) throws Exception {
        linkLayout(referenceFile, candidateFile, output, cls, null);
    }

    static byte[] definitions(ABC abc) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        ABCOutputStream out = new ABCOutputStream(bytes);
        out.writeU30(abc.method_info.size());
        for (var method : abc.method_info) out.writeMethodInfo(method);
        out.writeU30(abc.instance_info.size());
        for (var instance : abc.instance_info) out.writeInstanceInfo(instance);
        for (var cls : abc.class_info) { out.writeU30(cls.cinit_index); out.writeTraits(cls.static_traits); }
        out.writeU30(abc.script_info.size());
        for (var script : abc.script_info) { out.writeU30(script.init_index); out.writeTraits(script.traits); }
        return bytes.toByteArray();
    }

    static String callCastFingerprint(ABC abc, MethodBody body) throws Exception {
        StringBuilder sequence = new StringBuilder();
        for (var instruction : body.getCode().code) {
            int op = instruction.definition.instructionCode;
            if (op == 0x03 || (op >= 0x40 && op <= 0x4f) || op == 0x53 || op == 0x58
                    || (op >= 0x70 && op <= 0x89) || op == 0xb2 || op == 0xb3)
                sequence.append(instruction.toStringNoAddress(abc.constants, List.of())).append('\n');
        }
        return hash(sequence.toString().getBytes(StandardCharsets.UTF_8));
    }

    static String checkFeature(ABC original, ABC compiled, String prefix, Path permissions) throws Exception {
        Map<String, String[]> allowed = new LinkedHashMap<>();
        for (String line : Files.readAllLines(permissions)) {
            String[] fields = line.split("\\t", -1);
            if (fields.length != 3 || !fields[0].startsWith(prefix) || fields[0].contains("|")
                    || allowed.put(fields[0], fields) != null)
                throw new IOException("Invalid or duplicate feature method permission");
            if (!(fields[1].equals("-") && fields[2].equals("-"))
                    && !(fields[1].matches("[0-9a-f]{64}") && fields[2].matches("[0-9a-f]{64}")))
                throw new IOException("Call/cast permission requires both reviewed fingerprints");
        }
        if (allowed.isEmpty()) throw new IOException("No feature methods authorized");
        if (!Arrays.equals(definitions(original), definitions(compiled)))
            throw new IOException("Feature changed class, script, trait or method-signature records");
        Map<Integer, String> owners = names(original);
        Set<String> seen = new HashSet<>();
        StringBuilder report = new StringBuilder("method\tbefore_record\tafter_record\tbefore_calls_casts\tafter_calls_casts\n");
        int changed = 0;
        for (MethodBody before : original.bodies) {
            MethodBody after = compiled.findBody(before.method_info);
            String owner = owners.get(before.method_info);
            byte[] a = record(original, before), b = record(compiled, after);
            if (!allowed.containsKey(owner)) {
                if (!Arrays.equals(a, b)) throw new IOException("Untouched method changed: " + owner);
                continue;
            }
            if (!seen.add(owner)) throw new IOException("Ambiguous feature method owner");
            String oldCalls = callCastFingerprint(original, before), newCalls = callCastFingerprint(compiled, after);
            String[] permission = allowed.get(owner);
            if (permission[1].equals("-")) {
                if (!oldCalls.equals(newCalls)) throw new IOException("Unreviewed calls/casts changed in " + owner
                    + ": " + oldCalls + " -> " + newCalls);
            } else if (!permission[1].equals(oldCalls) || !permission[2].equals(newCalls))
                throw new IOException("Reviewed calls/casts do not match compiler output for " + owner);
            if (!Arrays.equals(a, b)) changed++;
            report.append(cell(owner)).append('\t').append(hash(a)).append('\t').append(hash(b)).append('\t')
                .append(oldCalls).append('\t').append(newCalls).append('\n');
        }
        if (!seen.equals(allowed.keySet())) throw new IOException("Feature permission names a missing method");
        if (changed == 0) throw new IOException("Feature compiled without changing any authorized method");
        return report.toString();
    }

    static void linkLayout(Path referenceFile, Path candidateFile, Path output, String cls, Path permissions) throws Exception {
        SWF reference = read(referenceFile), candidate = read(candidateFile);
        if (reference.getAbcList().size() != candidate.getAbcList().size())
            throw new IOException("ABC block count changed");
        int home = -1;
        ScriptPack source = findScript(reference, cls);
        for (int i = 0; i < reference.getAbcList().size(); i++) {
            if (reference.getAbcList().get(i).getABC() != source.abc) continue;
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
        for (MethodBody body : original.bodies) exactCode(body);
        for (MethodBody body : compiled.bodies) exactCode(body);
        restoreNameReferences(original, compiled);
        if (permissions != null) relocateAddedPoolNames(original, compiled);
        restoreMethodOrder(original, compiled);
        emitScriptClassBindings(original, compiled);
        restoreScriptSlot(original, compiled);
        emitCompactOperations(original, compiled);
        retainDebugDirectives(original, compiled);
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
        Map<Integer, String> originalOwners = names(original), compiledOwners = names(compiled);
        for (int i = 0; i < original.method_info.size(); i++) {
            MethodInfo before = original.method_info.get(i), after = compiled.method_info.get(i);
            String oldName = before.name_index == 0 ? "" : original.constants.getString(before.name_index);
            String newName = after.name_index == 0 ? "" : compiled.constants.getString(after.name_index);
            if (!oldName.isEmpty() && !oldName.equals(newName)
                    && (!originalOwners.containsKey(i) || !originalOwners.get(i).equals(compiledOwners.get(i))))
                throw new IOException("Unowned debug method name changed at " + i);
            after.name_index = before.name_index;
            // Parameter names are optional debugger information, independent
            // of positional parameter types/defaults. Restore the source map
            // only for a uniquely matched method, retaining all other flags.
            if (!before.flagHas_paramnames()) {
                after.unsetFlagHas_paramnames();
                after.paramNames = new int[0];
            } else {
                if (!originalOwners.containsKey(i) || !originalOwners.get(i).equals(compiledOwners.get(i))
                        || before.paramNames.length != before.param_types.length
                        || before.param_types.length != after.param_types.length)
                    throw new IOException("Unmatched debugger parameter names at " + i);
                after.setFlagHas_paramnames();
                after.paramNames = before.paramNames.clone();
            }
        }
        // Feature output retains appended constants. Every original entry was
        // checked above, so untouched operands retain their original meaning.
        if (permissions == null) compiled.constants = original.constants;
        compiled.metadata_info = original.metadata_info;
        byte[] rebuilt = abcBytes(compiled), expected = abcBytes(original);
        int classId = original.findClassByName(cls);
        String ownerPrefix = classId >= 0 ? name(original, original.instance_info.get(classId).name_index) + "/"
            : "script:" + original.script_info.get(source.scriptIndex).traits.traits.stream()
                .map(t -> name(original, t.name_index)).sorted().toList() + "/";
        String featureReport = permissions == null ? null : checkFeature(original, compiled, ownerPrefix, permissions);
        if (permissions == null && !Arrays.equals(rebuilt, expected)) {
            Files.write(output.resolveSibling(output.getFileName() + ".rejected.abc"), rebuilt);
            StringBuilder differences = new StringBuilder();
            for (int i = 0; i < original.method_info.size(); i++) {
                ByteArrayOutputStream a = new ByteArrayOutputStream(), b = new ByteArrayOutputStream();
                new ABCOutputStream(a).writeMethodInfo(original.method_info.get(i));
                new ABCOutputStream(b).writeMethodInfo(compiled.method_info.get(i));
                if (!Arrays.equals(a.toByteArray(), b.toByteArray())) differences.append("method_info ").append(i)
                    .append(" original=").append(HexFormat.of().formatHex(a.toByteArray()))
                    .append(" compiled=").append(HexFormat.of().formatHex(b.toByteArray())).append('\n');
            }
            for (int i = 0; i < original.bodies.size(); i++) {
                MethodBody a = original.bodies.get(i), b = compiled.bodies.get(i);
                if (!Arrays.equals(record(original, a), record(compiled, b))) differences.append("body ").append(a.method_info)
                    .append(" code_equal=").append(Arrays.equals(a.getCodeBytes(), b.getCodeBytes()))
                    .append(" limits=").append(a.max_stack).append('/').append(b.max_stack).append(',')
                    .append(a.max_regs).append('/').append(b.max_regs).append(',')
                    .append(a.init_scope_depth).append('/').append(b.init_scope_depth).append(',')
                    .append(a.max_scope_depth).append('/').append(b.max_scope_depth).append('\n');
            }
            Files.writeString(output.resolveSibling(output.getFileName() + ".differences.txt"), differences);
            int first = 0;
            while (first < Math.min(rebuilt.length, expected.length) && rebuilt[first] == expected[first]) first++;
            throw new IOException("Compiled ABC still differs at byte " + first + " (original " + expected.length
                + " bytes, compiled " + rebuilt.length + "); no linked movie written");
        }
        ((Tag)compiled.parentTag).setModified(true);
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        candidate.saveTo(bytes);
        SWF saved = new SWF(new ByteArrayInputStream(bytes.toByteArray()), true);
        for (int i = 0; i < reference.getAbcList().size(); i++)
            if (!Arrays.equals(i == home ? rebuilt : abcBytes(reference.getAbcList().get(i).getABC()), abcBytes(saved.getAbcList().get(i).getABC())))
                throw new IOException("Serialized ABC differs at block " + i);
        Files.write(output, bytes.toByteArray(), StandardOpenOption.CREATE_NEW);
        if (featureReport != null) Files.writeString(output.resolveSibling(output.getFileName() + ".methods.tsv"), featureReport);
        System.out.println((permissions == null ? "Exact compiled ABC for " : "Verified feature ABC for ")
            + cls + ": " + rebuilt.length + " bytes; original bodies were not substituted");
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
                for (var instruction : exactCode(body).code)
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
        if (a[0].equals("link-feature")) {
            if (a.length != 6) throw new IllegalArgumentException("link-feature ORIGINAL COMPILED OUTPUT CLASS PERMISSIONS.tsv");
            linkLayout(Path.of(a[1]), Path.of(a[2]), Path.of(a[3]), a[4], Path.of(a[5]));
            return;
        }
        if (a[0].equals("link-layout")) {
            if (a.length != 5) throw new IllegalArgumentException("link-layout ORIGINAL COMPILED OUTPUT CLASS");
            linkLayout(Path.of(a[1]), Path.of(a[2]), Path.of(a[3]), a[4]);
            return;
        }
        if (a[0].equals("dependencies")) {
            Files.copy(Path.of(a[1]), Path.of(a[2]));
            FlexCompiler.qualifyDependencyInterfaces(Path.of(a[2]), a.length > 3
                ? FlexCompiler.externalDefinitions(Path.of(a[3])) : Set.of());
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
                    if (compiler instanceof FlexCompiler flex)
                        swf = importNative(Path.of(a[1]), flex.diagnostics.resolve("native-compiler.swf"), a[3]);
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

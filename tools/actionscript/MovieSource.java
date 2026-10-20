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
        final boolean optimized;
        final String libraryClass;
        FlexCompiler(String path, Path diagnostics, boolean strict, String libraryClass, boolean optimized) {
            super(path); sdk = Path.of(path); this.diagnostics = diagnostics; this.strict = strict;
            this.libraryClass = libraryClass; this.optimized = optimized;
        }
        // Adapt only the compiler's temporary dependency SWC. Qualify singleton
        // namespace sets and supply the inheritance catalog missing from FFDec's
        // export. The target movie remains untouched.
        static void qualifyDependencyInterfaces(Path swc) throws IOException, InterruptedException {
            qualifyDependencyInterfaces(swc, Set.of());
        }
        static void splitDependencyScripts(SWF dependency, Map<String, byte[]> entries) throws IOException {
            // A shared ABC becomes one SWC catalog script in FFDec's export.
            // Loading a base class then loads all of its descendants, including
            // ones that depend on the source class currently being compiled.
            // Keep each original script in its own dependency-only ABC so Flex
            // can load the actual inheritance graph without that artificial cycle.
            if (dependency.getAbcList().stream().noneMatch(t -> t.getABC().script_info.size() > 1)) return;
            try {
                var factory = DocumentBuilderFactory.newInstance();
                factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
                var doc = factory.newDocumentBuilder().parse(new ByteArrayInputStream(entries.get("catalog.xml")));
                for (var container : new ArrayList<>(dependency.getAbcList())) {
                    ABC abc = container.getABC();
                    if (abc.script_info.size() < 2) continue;
                    if (!(container instanceof com.jpexs.decompiler.flash.tags.DoABC2Tag original))
                        throw new IOException("Shared dependency requires a named DoABC tag");
                    org.w3c.dom.Element catalog = null;
                    var scripts = doc.getElementsByTagName("script");
                    for (int i = 0; i < scripts.getLength(); i++) {
                        var script = (org.w3c.dom.Element)scripts.item(i);
                        if (!script.getAttribute("name").equals(original.name)) continue;
                        if (catalog != null) throw new IOException("Ambiguous dependency catalog script");
                        catalog = script;
                    }
                    if (catalog == null) throw new IOException("Missing dependency catalog script");
                    Set<String> remaining = new HashSet<>();
                    var defs = catalog.getElementsByTagName("def");
                    for (int i = 0; i < defs.getLength(); i++)
                        remaining.add(((org.w3c.dom.Element)defs.item(i)).getAttribute("id"));
                    int position = dependency.indexOfTag(original);
                    dependency.removeTag(original);
                    byte[] bytes = abcBytes(abc);
                    for (int selected = 0; selected < abc.script_info.size(); selected++) {
                        var tag = new com.jpexs.decompiler.flash.tags.DoABC2Tag(dependency);
                        tag.flags = original.flags;
                        tag.name = "__srp_dependency_" + position + "_" + selected;
                        var copy = new ABC(new ABCInputStream(new com.jpexs.helpers.MemoryInputStream(bytes)),
                                           dependency, tag);
                        for (int i = 0; i < copy.script_info.size(); i++)
                            if (i != selected) copy.script_info.get(i).delete(copy, true);
                        copy.pack();
                        if (copy.script_info.size() != 1) throw new IOException("Dependency script selection failed");
                        tag.setABC(copy); tag.setModified(true);
                        dependency.addTag(position++, tag);
                        var script = doc.createElement("script");
                        script.setAttribute("name", tag.name);
                        script.setAttribute("mod", catalog.getAttribute("mod"));
                        for (var trait : copy.script_info.get(0).traits.traits) {
                            String name = catalogName(copy, trait.name_index);
                            if (!remaining.remove(name)) continue;
                            var def = doc.createElement("def"); def.setAttribute("id", name); script.appendChild(def);
                        }
                        var dep = doc.createElement("dep");
                        dep.setAttribute("id", "AS3"); dep.setAttribute("type", "n"); script.appendChild(dep);
                        catalog.getParentNode().insertBefore(script, catalog);
                    }
                    if (!remaining.isEmpty()) throw new IOException("Dependency split lost definitions: " + remaining);
                    catalog.getParentNode().removeChild(catalog);
                }
                dependency.clearAbcListCache();
                var output = new ByteArrayOutputStream();
                TransformerFactory.newInstance().newTransformer().transform(new DOMSource(doc), new StreamResult(output));
                entries.put("catalog.xml", output.toByteArray());
            } catch (Exception error) { throw new IOException("Cannot split dependency scripts", error); }
        }
        static void qualifyDependencyInterfaces(Path swc, Set<String> externalDefinitions) throws IOException, InterruptedException {
            Map<String, byte[]> entries = new LinkedHashMap<>();
            try (ZipInputStream zip = new ZipInputStream(Files.newInputStream(swc))) {
                for (ZipEntry entry; (entry = zip.getNextEntry()) != null; ) entries.put(entry.getName(), zip.readAllBytes());
            }
            byte[] library = entries.get("library.swf");
            if (library == null) throw new IOException("Dependency SWC has no library.swf");
            SWF dependency = new SWF(new ByteArrayInputStream(library), true);
            splitDependencyScripts(dependency, entries);
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
                    deps.removeAll(own);
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
            String ns = namespaceText(abc, abc.constants.getNamespace(m.namespace_index));
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
                command.addAll(List.of("-strict=true", "-debug=false", optimized ? "-optimize=true" : "-optimize=false", "-omit-trace-statements=false"));
            }
            if (libraryClass != null) {
                // ASC emits different self-class lookups for the entry script
                // and an imported source unit. Compile the requested class as
                // a dependency; retain the complete native output as evidence.
                if (!libraryClass.matches("[A-Za-z_$][A-Za-z0-9_$]*(\\.[A-Za-z_$][A-Za-z0-9_$]*)*")
                        || libraryClass.equals("__SRPCompileEntry"))
                    throw new IOException("Unsupported library class name");
                int sourcePath = command.indexOf("-source-path");
                if (sourcePath < 0 || sourcePath + 1 >= command.size())
                    throw new IOException("Missing native source path");
                Path root = Path.of(command.get(sourcePath + 1));
                Path source = root.resolve(libraryClass.replace('.', '/') + ".as");
                int sourceIndex = command.indexOf(source.toString());
                if (sourceIndex < 0 || command.lastIndexOf(source.toString()) != sourceIndex)
                    throw new IOException("Missing or ambiguous native source argument");
                Path entry = root.resolve("__SRPCompileEntry.as");
                String text = "package { import " + libraryClass + "; public class __SRPCompileEntry {"
                    + " public var reference:Class = " + libraryClass.substring(libraryClass.lastIndexOf('.') + 1)
                    + "; } }\n";
                Files.writeString(entry, text, StandardOpenOption.CREATE_NEW);
                Files.writeString(diagnostics.resolve("entry-source.as"), text);
                command.set(sourceIndex, entry.toString());
                command.remove("-include-inheritance-dependencies-only");
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
                Path nativeFile = Path.of(args.get(output + 1));
                Files.copy(nativeFile, diagnostics.resolve("native-compiler.swf"));
                if (libraryClass != null) {
                    // FFDec's importer expects the selected script last. Give
                    // it a container with that exact native ABC, excluding the
                    // temporary entry and linked dependency ABCs.
                    try {
                        selectNativeScript(nativeFile, diagnostics.resolve("native-script.swf"), libraryClass);
                    } catch (Exception error) {
                        throw new IOException("Cannot select native library script", error);
                    }
                    Files.copy(diagnostics.resolve("native-script.swf"), nativeFile, StandardCopyOption.REPLACE_EXISTING);
                }
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

    static void selectNativeScript(Path input, Path output, String cls) throws Exception {
        SWF movie = read(input);
        ScriptPack target = findScript(movie, cls);
        if (target.abc.script_info.size() > 1) {
            ABC selected = target.abc;
            for (var body : selected.bodies) exactCode(body);
            for (int i = 0; i < selected.script_info.size(); i++)
                if (i != target.scriptIndex) selected.script_info.get(i).delete(selected, true);
            selected.pack();
            for (var container : movie.getAbcList()) if (container.getABC() == selected)
                ((Tag)container).setModified(true);
            movie.clearAbcListCache();
            target = findScript(movie, cls);
        }
        if (target.scriptIndex != 0 || target.abc.script_info.size() != 1
                || target.abc.findClassByName(cls) < 0)
            throw new IOException("Native library ABC does not uniquely define the requested class");
        byte[] emitted = abcBytes(target.abc);
        for (MethodBody body : target.abc.bodies) exactCode(body);
        for (var tag : new ArrayList<>(movie.getAbcList()))
            if (tag.getABC() != target.abc) movie.removeTag((Tag)tag);
        try (OutputStream out = Files.newOutputStream(output)) { movie.saveTo(out); }
        SWF saved = read(output);
        if (saved.getAbcList().size() != 1
                || !Arrays.equals(emitted, abcBytes(findScript(saved, cls).abc)))
            throw new IOException("Selected native library ABC changed during serialization");
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
        retainDebugMetadata(original, compiled, before, after, false);
    }

    // With `additions`, a feature may append traits: the original traits keep
    // their order and records, and new traits follow in compiler order. A
    // removed trait (or one whose kind changed) is still refused.
    static void retainDebugMetadata(ABC original, ABC compiled, Traits before, Traits after, boolean additions)
            throws IOException {
        if (additions ? before.traits.size() > after.traits.size() : before.traits.size() != after.traits.size())
            throw new IOException("Trait count changed");
        Map<List<Object>, Trait> emitted = new HashMap<>();
        for (Trait trait : after.traits)
            if (emitted.put(List.of(trait.kindType, name(compiled, trait.name_index)), trait) != null)
                throw new IOException("Ambiguous compiled trait identity");
        List<Trait> ordered = new ArrayList<>();
        for (Trait trait : before.traits) {
            Trait found = emitted.remove(List.of(trait.kindType, name(original, trait.name_index)));
            if (found == null) throw new IOException(additions ? "Feature removed or re-kinded trait "
                + name(original, trait.name_index) : "Trait identities changed");
            ordered.add(found);
        }
        List<Trait> added = new ArrayList<>(after.traits);
        added.removeIf(trait -> !emitted.containsValue(trait));
        ordered.addAll(added);
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

    static Set<List<Object>> traitIdentities(ABC abc, Traits traits) {
        Set<List<Object>> result = new HashSet<>();
        for (Trait trait : traits.traits) result.add(List.of(trait.kindType, name(abc, trait.name_index)));
        return result;
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

    static String namespaceText(ABC abc, Namespace namespace) {
        return namespace.name_index == 0 ? "" : abc.constants.getString(namespace.name_index);
    }

    static String privateRole(ABC abc, int index, String className, String simpleName) {
        Namespace ns = abc.constants.getNamespace(index);
        if (ns.kind != 5) return null;
        String text = namespaceText(abc, ns);
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
                publicName |= ns.kind == 0x16 && namespaceText(abc, ns).isEmpty();
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

    static boolean unusedNamespace(ABC abc, int namespace) {
        if (!emptyFileNamespace(abc, namespace)) return false;
        for (int i = 1; i < abc.constants.getNamespaceSetCount(); i++)
            for (int member : abc.constants.getNamespaceSet(i).namespaces)
                if (member == namespace) return false;
        return true;
    }

    static List<Integer> nonPrivateNamespaces(ABC abc, int[] members) {
        return Arrays.stream(members).filter(n -> abc.constants.getNamespace(n).kind != 5)
            .sorted().boxed().toList();
    }

    static Set<Integer> privateNamespaces(ABC abc, int[] members) {
        Set<Integer> result = new TreeSet<>();
        for (int n : members) if (abc.constants.getNamespace(n).kind == 5) result.add(n);
        return result;
    }

    static List<String> privateDeclarations(ABC abc, int namespace) {
        List<String> result = new ArrayList<>();
        for (int i = 0; i < abc.instance_info.size(); i++) {
            String owner = publicName(abc, abc.instance_info.get(i).name_index);
            if (owner == null) continue;
            List<Traits> groups = List.of(abc.instance_info.get(i).instance_traits,
                abc.class_info.get(i).static_traits);
            for (int group = 0; group < groups.size(); group++) {
                for (var trait : groups.get(group).traits) {
                    var name = abc.constants.getMultiname(trait.name_index);
                    if (name.kind == 7 && name.namespace_index == namespace)
                        result.add(owner + "/" + group + "/" + trait.kindType + "/"
                            + abc.constants.getString(name.name_index));
                }
            }
        }
        Collections.sort(result);
        return result;
    }

    static boolean namespacesTogether(ABC abc, Set<Integer> group) {
        for (int i = 1; i < abc.constants.getNamespaceSetCount(); i++) {
            var members = privateNamespaces(abc, abc.constants.getNamespaceSet(i).namespaces);
            if (!Collections.disjoint(group, members) && !members.containsAll(group)) return false;
        }
        return true;
    }

    // Some published ABCs erase private namespace labels. Recover identities
    // from retained namespace-set anchors and owned declarations, never from
    // the erased text. Unbound identities can be interchangeable only when
    // every containing set includes the entire group on both sides.
    static Map<Integer, Integer> anonymousNamespaceLayout(ABC original, ABC compiled) {
        int retained = original.constants.getNamespaceCount();
        Map<Integer, Set<Integer>> candidates = new TreeMap<>();
        Set<Integer> empty = new HashSet<>();
        for (int ns = retained; ns < compiled.constants.getNamespaceCount(); ns++) {
            if (compiled.constants.getNamespace(ns).kind != 5 || unusedNamespace(compiled, ns)) continue;
            Set<Integer> possible = null;
            boolean valid = true;
            for (int set = 1; set < compiled.constants.getNamespaceSetCount(); set++) {
                int[] values = compiled.constants.getNamespaceSet(set).namespaces;
                var fresh = privateNamespaces(compiled, values);
                if (!fresh.contains(ns)) continue;
                var retainedMembers = new TreeSet<>(fresh);
                retainedMembers.removeIf(n -> n >= retained);
                fresh.removeIf(n -> n < retained);
                var anchor = nonPrivateNamespaces(compiled, values);
                Set<Set<Integer>> matches = new HashSet<>();
                for (int oldSet = 1; oldSet < original.constants.getNamespaceSetCount(); oldSet++) {
                    int[] oldValues = original.constants.getNamespaceSet(oldSet).namespaces;
                    if (!anchor.equals(nonPrivateNamespaces(original, oldValues))) continue;
                    var old = privateNamespaces(original, oldValues);
                    if (!old.containsAll(retainedMembers)) continue;
                    old.removeAll(retainedMembers);
                    if (old.size() != fresh.size()) continue;
                    boolean erasedOriginal = old.stream().allMatch(
                        n -> original.constants.getNamespace(n).name_index == 0);
                    boolean erasedCompiled = fresh.stream().allMatch(
                        n -> compiled.constants.getNamespace(n).name_index == 0);
                    if (!erasedOriginal && !erasedCompiled) continue;
                    matches.add(old);
                }
                if (matches.size() != 1) { valid = false; break; }
                var selected = matches.iterator().next();
                if (possible == null) possible = new TreeSet<>(selected);
                else possible.retainAll(selected);
            }
            if (!valid) continue;
            if (possible == null) {
                // A qualified-only private field may never occur in a namespace
                // set. A unique, nonempty set of owned declarations still
                // establishes its identity when either pool erased the label.
                var declarations = privateDeclarations(compiled, ns);
                if (declarations.isEmpty()) continue;
                possible = new TreeSet<>();
                for (int old = 1; old < retained; old++) {
                    var before = original.constants.getNamespace(old);
                    if (before.kind == 5 && (before.name_index == 0
                            || compiled.constants.getNamespace(ns).name_index == 0)
                            && declarations.equals(privateDeclarations(original, old))) possible.add(old);
                }
                if (possible.size() != 1) continue;
            }
            if (possible.isEmpty()) continue;
            if (emptyFileNamespace(compiled, ns)) {
                possible.removeIf(n -> !emptyFileNamespace(original, n));
                empty.add(ns);
            } else {
                var declarations = privateDeclarations(compiled, ns);
                if (declarations.isEmpty()) continue;
                possible.removeIf(n -> !declarations.equals(privateDeclarations(original, n)));
            }
            if (!possible.isEmpty()) candidates.put(ns, possible);
        }
        Map<Integer, Integer> result = new TreeMap<>();
        Set<Integer> used = new HashSet<>();
        boolean progress = true;
        while (progress) {
            progress = false;
            for (var entry : candidates.entrySet()) {
                if (result.containsKey(entry.getKey())) continue;
                entry.getValue().removeAll(used);
                if (entry.getValue().size() == 1) {
                    int selected = entry.getValue().iterator().next();
                    result.put(entry.getKey(), selected); used.add(selected); progress = true;
                }
            }
        }
        for (var entry : candidates.entrySet()) {
            if (result.containsKey(entry.getKey()) || !empty.contains(entry.getKey())) continue;
            entry.getValue().removeAll(used);
            if (entry.getValue().isEmpty()) continue;
            var oldGroup = new TreeSet<>(entry.getValue());
            Set<Integer> newGroup = new TreeSet<>();
            for (var other : candidates.entrySet())
                if (!result.containsKey(other.getKey()) && empty.contains(other.getKey())
                        && other.getValue().equals(oldGroup)) newGroup.add(other.getKey());
            if (newGroup.size() != oldGroup.size() || !namespacesTogether(original, oldGroup)
                    || !namespacesTogether(compiled, newGroup)) continue;
            var old = oldGroup.iterator();
            for (int fresh : newGroup) {
                int target = old.next(); result.put(fresh, target); used.add(target);
            }
        }
        return result;
    }

    static Map<Integer, Integer> namespaceLayout(ABC original, ABC compiled,
            Map<Integer, List<Integer>> perMethod) throws IOException {
        Map<Integer, Integer> mapping = new HashMap<>();
        int retained = original.constants.getNamespaceCount();
        for (int i = 0; i < retained; i++) mapping.put(i, i);
        mapping.putAll(anonymousNamespaceLayout(original, compiled));
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
                String pkg = namespaceText(original, original.constants.getNamespace(name.namespace_index));
                String qualified = pkg.isEmpty() ? simple : pkg + ":" + simple;
                for (String role : List.of("class", "script")) {
                    List<Integer> before = new ArrayList<>(), after = new ArrayList<>();
                    for (int i = 1; i < retained; i++)
                        if (role.equals(privateRole(original, i, qualified, simple))) before.add(i);
                    for (int i = retained; i < compiled.constants.getNamespaceCount(); i++)
                        if (role.equals(privateRole(compiled, i, qualified, simple))) after.add(i);
                    if (after.isEmpty()) continue;
                    if (before.isEmpty() && after.size() == 1 && mapping.containsKey(after.get(0))) continue;
                    // Flash can omit a class's private namespace when there
                    // are no private bindings. Flex still emits the unused
                    // constant. It needs no mapping; the normal pool cleanup
                    // removes it after checking every executable record.
                    if (before.isEmpty() && after.stream().allMatch(ns -> unusedNamespace(compiled, ns)))
                        continue;
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
          // A file-private helper class carries the file's generated identity
          // into its protected namespace, e.g. Tween.as$218:Prop. Relocate that
          // owned namespace only after the containing file has a unique map.
          for (int owner = 0; owner < original.instance_info.size(); owner++) {
            var before = original.instance_info.get(owner);
            var after = compiled.instance_info.get(owner);
            var oldName = original.constants.getMultiname(before.name_index);
            var newName = compiled.constants.getMultiname(after.name_index);
            if (oldName.kind != 7 || newName.kind != 7 || before.protectedNS == 0
                    || after.protectedNS < retained) continue;
            var oldFile = original.constants.getNamespace(oldName.namespace_index);
            var newFile = compiled.constants.getNamespace(newName.namespace_index);
            String simple = original.constants.getString(oldName.name_index);
            if (oldFile.kind != 5 || newFile.kind != 5
                    || !simple.equals(compiled.constants.getString(newName.name_index))
                    || !Objects.equals(mapping.get(newName.namespace_index), oldName.namespace_index)) continue;
            var oldProtected = original.constants.getNamespace(before.protectedNS);
            var newProtected = compiled.constants.getNamespace(after.protectedNS);
            String oldOwner = namespaceText(original, oldFile) + ":" + simple;
            String newOwner = namespaceText(compiled, newFile) + ":" + simple;
            if (oldProtected.kind != 0x18 || newProtected.kind != 0x18
                    || !oldOwner.equals(namespaceText(original, oldProtected))
                    || !newOwner.equals(namespaceText(compiled, newProtected))) continue;
            if (original.instance_info.stream().filter(i -> i.protectedNS == before.protectedNS).count() != 1
                    || compiled.instance_info.stream().filter(i -> i.protectedNS == after.protectedNS).count() != 1)
                throw new IOException("Ambiguous file-private protected namespace owner");
            Integer previous = mapping.putIfAbsent(after.protectedNS, before.protectedNS);
            if (previous != null && previous != before.protectedNS)
                throw new IOException("Conflicting file-private protected namespace mapping");
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

    // Duplicate generic type constants can have identical resolved identities.
    // A uniquely matched field declaration supplies its own original alias;
    // a changed type or a different/ambiguous field never inherits that index.
    static void restoreTraitTypeAliases(ABC original, ABC compiled, Traits before, Traits after,
            Map<Integer,Integer> namespaces) throws IOException {
        for (var trait : after.traits) if (trait instanceof TraitSlotConst slot) {
            var matches = before.traits.stream().filter(t -> t.name_index == trait.name_index
                && t.kindType == trait.kindType && t instanceof TraitSlotConst).toList();
            if (matches.size() != 1) continue;
            var old = (TraitSlotConst)matches.get(0);
            var oldKey = multinameKey(original,old.type_index,Map.of(),new HashMap<>(),new HashSet<>());
            var newKey = multinameKey(compiled,slot.type_index,namespaces,new HashMap<>(),new HashSet<>());
            if (oldKey.equals(newKey)) slot.type_index = old.type_index;
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
        if (original.instance_info.size() == compiled.instance_info.size()
                && original.class_info.size() == compiled.class_info.size()) {
            for (int i = 0; i < original.instance_info.size(); i++) {
                var before = original.instance_info.get(i); var after = compiled.instance_info.get(i);
                if (before.name_index != after.name_index) continue;
                restoreTraitTypeAliases(original,compiled,before.instance_traits,after.instance_traits,namespaces);
                restoreTraitTypeAliases(original,compiled,original.class_info.get(i).static_traits,
                    compiled.class_info.get(i).static_traits,namespaces);
            }
        }
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
        restoreMethodOrder(original, compiled, false);
    }

    // With `additions`, methods new to the compiled source (by owner) follow
    // every original method, in compiler order; each original method keeps
    // its index. A method of the original missing from the compiled source
    // is refused: method indices cannot have holes.
    static void restoreMethodOrder(ABC original, ABC compiled, boolean additions) throws IOException {
        // With additions, a missing method is named by the owner check below.
        if (!additions && original.method_info.size() != compiled.method_info.size())
            throw new IOException("Method count changed");
        Map<Integer, String> before = names(original), after = names(compiled);
        if (before.equals(after) && original.method_info.size() == compiled.method_info.size()) return;
        if (before.size() != original.method_info.size() || after.size() != compiled.method_info.size())
            throw new IOException("Anonymous method ownership is unresolved");
        Map<String, Integer> owners = new HashMap<>();
        for (var entry : before.entrySet())
            if (owners.put(entry.getValue(), entry.getKey()) != null) throw new IOException("Ambiguous method owner");
        Map<Integer, Integer> mapping = new HashMap<>();
        Set<Integer> assigned = new HashSet<>();
        int appended = original.method_info.size();
        for (var entry : after.entrySet()) {
            Integer index = owners.get(entry.getValue());
            if (index == null && additions) index = appended++;
            if (index == null || !assigned.add(index)) throw new IOException("Method identities changed");
            mapping.put(entry.getKey(), index);
        }
        if (additions) for (var entry : before.entrySet())
            if (!assigned.contains(entry.getKey())) throw new IOException("Feature removed method " + entry.getValue());
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
        if (additions) for (int index : new TreeSet<>(bodies.keySet()))
            if (index >= original.method_info.size()) ordered.add(bodies.remove(index));
        if (!bodies.isEmpty()) throw new IOException("Unexpected compiled method bodies");
        compiled.method_info = infos;
        compiled.bodies = ordered;
        compiled.resetMethodIndexing();
    }

    // Recognize only the straight-line class-definition initializer emitted by
    // ASC. Script entry uses its global object, containing the class trait.
    // The layout emitter can express that class binding as a property lookup.
    // This is an explicit code-generation rule, not body substitution.
    static boolean publicScriptClassBinding(ABC abc, ScriptInfo script, AVM2Instruction instruction) {
        if (instruction.definition.instructionCode != 0x5d || script.traits.traits.size() != 1
                || !(script.traits.traits.get(0) instanceof TraitClass trait)) return false;
        var binding = abc.constants.getMultiname(instruction.operands[0]);
        var declared = abc.constants.getMultiname(trait.name_index);
        if (binding.kind != 0x09 || declared.kind != 0x07 || binding.name_index != declared.name_index)
            return false;
        int[] namespaces = abc.constants.getNamespaceSet(binding.namespace_set_index).namespaces;
        return namespaces.length == 1 && namespaces[0] == declared.namespace_index
            && abc.constants.getNamespace(declared.namespace_index).kind == 0x16;
    }

    static int lexicalLookupWidth(List<AVM2Instruction> code, int index) {
        if (index >= code.size()) return 0;
        var first = code.get(index);
        if (first.definition.instructionCode == 0x60) return 1;
        return index + 1 < code.size() && first.definition.instructionCode == 0x5d
            && code.get(index + 1).definition.instructionCode == 0x66
            && first.operands[0] == code.get(index + 1).operands[0] ? 2 : 0;
    }

    static boolean classInitializer(ABC abc, ScriptInfo script, MethodBody body) {
        if (script.traits.traits.size() != 1 || !(script.traits.traits.get(0) instanceof TraitClass trait)
                || body == null || body.init_scope_depth < 0 || body.init_scope_depth > 1 || body.max_regs != 1
                || body.exceptions.length != 0 || !body.traits.traits.isEmpty()) return false;
        var code = body.getCode().code;
        int n = code.size();
        if (n < 7 || code.get(0).definition.instructionCode != 0xd0
                || code.get(1).definition.instructionCode != 0x30) return false;
        var binding = code.get(2);
        if (!(binding.definition.instructionCode == 0x65 && binding.operands[0] == 0)
                && !publicScriptClassBinding(abc, script, binding)) return false;
        if (abc.instance_info.get(trait.class_info).isInterface())
            return n == 7 && code.get(3).definition.instructionCode == 0x20
                && code.get(4).definition.instructionCode == 0x58 && code.get(4).operands[0] == trait.class_info
                && code.get(5).definition.instructionCode == 0x68 && code.get(5).operands[0] == trait.name_index
                && code.get(6).definition.instructionCode == 0x47;
        int i = 3, scopes = 0, width;
        while ((width = lexicalLookupWidth(code, i)) > 0 && i + width < n
                && code.get(i + width).definition.instructionCode == 0x30) {
            i += width + 1; scopes++;
        }
        width = lexicalLookupWidth(code, i);
        if (scopes == 0 || width == 0 || i + width >= n
                || code.get(i + width).definition.instructionCode != 0x58
                || code.get(i + width).operands[0] != trait.class_info) return false;
        i += width + 1;
        for (int j = 0; j < scopes; j++, i++)
            if (i >= n || code.get(i).definition.instructionCode != 0x1d) return false;
        return i + 2 == n && code.get(i).definition.instructionCode == 0x68
            && code.get(i).operands[0] == trait.name_index && code.get(i + 1).definition.instructionCode == 0x47;
    }

    static void emitScriptClassBindings(ABC original, ABC compiled) throws IOException {
        emitScriptClassBindings(original, compiled, false);
    }

    static void emitScriptClassBindings(ABC original, ABC compiled, boolean additions) throws IOException {
        if (additions ? original.script_info.size() > compiled.script_info.size()
                : original.script_info.size() != compiled.script_info.size())
            throw new IOException("Script count changed");
        for (int i = 0; i < original.script_info.size(); i++) {
            ScriptInfo script = compiled.script_info.get(i);
            MethodBody body = compiled.findBody(script.init_index);
            if (!classInitializer(compiled, script, body)) continue;
            MethodBody reference = original.findBody(original.script_info.get(i).init_index);
            if (reference == null) continue;
            var executable = reference.getCode().code.stream().filter(ins -> !debugInstruction(ins)).toList();
            if (executable.size() < 3) continue;
            var layout = executable.get(2);
            if (layout.definition.instructionCode == 0x65 && layout.operands[0] == 0) {
                var code = body.getCode();
                code.code.set(2, new AVM2Instruction(code.code.get(2).getAddress(), 0x65, new int[]{0}));
                code.markOffsets(); body.setCode(code);
                continue;
            }
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
        for (int i = 0; i < Math.min(original.script_info.size(), compiled.script_info.size()); i++) {
            var before = original.script_info.get(i).traits.traits;
            var after = compiled.script_info.get(i).traits.traits;
            // With exactly one script trait and no script inheritance, the
            // automatically assigned slot (0) is slot 1. Other slot layouts
            // require a full slot map and are deliberately left unchanged.
            if (before.size() == 1 && after.size() == 1 && before.get(0) instanceof TraitClass a
                    && after.get(0) instanceof TraitClass b && a.name_index == b.name_index
                    && a.class_info == b.class_info && a.slot_id <= 1 && b.slot_id <= 1) b.slot_id = a.slot_id;
            if (before.size() == 1 && after.size() == 1 && before.get(0) instanceof TraitSlotConst a
                    && after.get(0) instanceof TraitSlotConst b && a.name_index == b.name_index
                    && a.kindType == b.kindType && a.type_index == b.type_index
                    && a.value_index == b.value_index && a.value_kind == b.value_kind
                    && a.slot_id == 0 && b.slot_id == 1) b.slot_id = 0;
            // Preserve the release's dispatch hint for a sole global function.
            // Adobe's parser ignores this field and binds by the named trait.
            // Keep this narrower than a general vtable relocation: no classes,
            // other scripts, getters/setters or early-bound callmethod sites.
            if (original.script_info.size() == 1 && compiled.script_info.size() == 1
                    && original.instance_info.isEmpty() && compiled.instance_info.isEmpty()
                    && before.size() == 1 && after.size() == 1
                    && before.get(0) instanceof TraitMethodGetterSetter a
                    && after.get(0) instanceof TraitMethodGetterSetter b
                    && a.kindType == 1 && b.kindType == 1 && a.name_index == b.name_index
                    && a.method_info == b.method_info && a.disp_id == 1 && b.disp_id > 0
                    && java.util.stream.Stream.concat(original.bodies.stream(), compiled.bodies.stream())
                        .flatMap(body -> body.getCode().code.stream())
                        .noneMatch(instruction -> instruction.definition.instructionCode == 0x43))
                b.disp_id = a.disp_id;
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

    static int localRegister(AVM2Instruction instruction, boolean write) {
        int opcode = instruction.definition.instructionCode;
        if (opcode == (write ? 0x63 : 0x62) && instruction.operands.length == 1)
            return instruction.operands[0];
        int first = write ? 0xd4 : 0xd0;
        return opcode >= first && opcode <= first + 3 ? opcode - first : -1;
    }

    static boolean redundantNumericSelfAssignment(List<AVM2Instruction> code, int next, int desired) {
        // FFDec emits both stores for n = ++n. After numeric increment/decrement,
        // the duplicated value is primitive and both int/uint conversions yield
        // the same value. With the same local on both stores, the first store is
        // unobservable. Full CFG checks below reject entry/handlers in this group.
        if (next < 2 || next + 4 >= code.size() || (desired != 0x73 && desired != 0x74)
                || code.get(next).definition.instructionCode != 0x2a
                || code.get(next + 1).definition.instructionCode != desired
                || code.get(next + 3).definition.instructionCode != desired) return false;
        int register = localRegister(code.get(next - 2), false);
        int operation = code.get(next - 1).definition.instructionCode;
        return register >= 0 && (operation == 0x91 || operation == 0x93 || operation == 0xc0 || operation == 0xc1)
            && localRegister(code.get(next + 2), true) == register
            && localRegister(code.get(next + 4), true) == register;
    }

    static Set<Integer> unreachableJumps(MethodBody body) throws IOException {
        var instructions = body.getCode().code;
        Map<Integer, Integer> indices = new HashMap<>();
        for (int i = 0; i < instructions.size(); i++)
            indices.put((int)instructions.get(i).getAddress(), i);
        Set<Integer> reached = new HashSet<>();
        ArrayDeque<Integer> pending = new ArrayDeque<>();
        if (!instructions.isEmpty()) pending.add(0);
        // Every handler is a root, even for an unreachable protected region.
        // This deliberately overestimates reachability.
        for (var handler : body.exceptions) pending.add(boundary(indices, handler.target));
        while (!pending.isEmpty()) {
            int index = pending.removeFirst();
            if (!reached.add(index)) continue;
            var instruction = instructions.get(index);
            int opcode = instruction.definition.instructionCode;
            for (int target : branchTargets(instruction)) pending.add(boundary(indices, target));
            if (opcode != 0x03 && opcode != 0x47 && opcode != 0x48
                    && opcode != 0x10 && opcode != 0x1b && index + 1 < instructions.size())
                pending.add(index + 1);
        }
        Set<Integer> result = new HashSet<>();
        for (int i = 0; i < instructions.size(); i++) {
            var instruction = instructions.get(i);
            if (!reached.contains(i) && instruction.definition.instructionCode == 0x10)
                result.add((int)instruction.getAddress());
        }
        return result;
    }

    static boolean mutableInstanceSlot(ABC abc, MethodBody body, List<AVM2Instruction> code,
            int store) throws IOException {
        var instruction = code.get(store);
        if (instruction.operands.length != 1) return false;
        int name = instruction.operands[0];
        var multiname = abc.constants.getMultiname(name);
        if (multiname.kind != 7) return false;
        int kind = abc.constants.getNamespace(multiname.namespace_index).kind;
        // Private/protected own slots cannot be dynamic public properties.
        if (kind != 0x05 && kind != 0x18) return false;
        boolean mutable = false;
        for (var instance : abc.instance_info) {
            boolean constructor = instance.iinit_index == body.method_info;
            boolean instanceMethod = instance.instance_traits.traits.stream().anyMatch(t ->
                t instanceof TraitMethodGetterSetter m && m.method_info == body.method_info);
            if (!constructor && (!instanceMethod || instanceBindings(abc,body) == null)) continue;
            int matches = 0;
            for (var trait : instance.instance_traits.traits) if (trait.name_index == name) {
                matches++;
                mutable = trait instanceof TraitSlotConst && trait.kindType == 0;
            }
            if (matches != 1) return false;
        }
        if (!mutable) return false;
        Set<Integer> entries = new HashSet<>();
        var boundaries = executablePositions(body.getCode().code);
        for (var item : code) {
            if (localRegister(item, true) == 0) return false;
            if (localRegister(item, false) != 0)
                for (int j = 0; j < item.definition.operands.length; j++)
                    if (item.definition.operands[j] == AVM2Code.DAT_LOCAL_REG_INDEX && item.operands[j] == 0)
                        return false;
            for (int target : branchTargets(item)) entries.add(boundary(boundaries, target));
        }
        for (var handler : body.exceptions) entries.add(boundary(boundaries, handler.target));
        // Walk back through the receiver/value expression. Only a direct this
        // load proves the receiver; joins, dup/swap and non-linear control flow
        // are deliberately excluded. No stack value is guessed at a boundary.
        int needed = 2;
        for (int i = store; i > 0;) {
            if (entries.contains(i)) return false;
            var item = code.get(--i);
            int opcode = item.definition.instructionCode;
            if (opcode == 0x2a || opcode == 0x2b || opcode == 0x09
                    || item.definition.isExitInstruction() || branchTargets(item).length != 0) return false;
            int pushes = item.definition.getStackPushCount(item, abc);
            int pops = item.definition.getStackPopCount(item, abc);
            if (pushes < 0 || pops < 0) return false;
            if (pushes >= needed) return needed == 1 && localRegister(item, false) == 0;
            needed += pops - pushes;
        }
        return false;
    }

    static String publicName(ABC abc, int id) {
        if (id <= 0) return null;
        var name = abc.constants.getMultiname(id);
        if (name.kind != 7) return null;
        var namespace = abc.constants.getNamespace(name.namespace_index);
        if (namespace.kind != 0x16) return null;
        String pkg = namespaceText(abc, namespace);
        return (pkg.isEmpty() ? "" : pkg + ".") + abc.constants.getString(name.name_index);
    }

    // Ordinary instance scope resolves declared bindings, including ancestors;
    // unlike with-scope, it does not search dynamic properties or prototypes.
    // https://github.com/adobe/avmplus/blob/master/core/MethodEnv.cpp#L1221
    // Require a complete hierarchy and a uniquely owned instance method.
    static Set<String> instanceBindings(ABC abc, MethodBody body) throws IOException {
        int owner = -1;
        for (int i = 0; i < abc.instance_info.size(); i++) {
            var instance = abc.instance_info.get(i);
            if (instance.iinit_index == body.method_info || abc.class_info.get(i).cinit_index == body.method_info)
                return null;
            for (var trait : instance.instance_traits.traits)
                if (trait instanceof TraitMethodGetterSetter method && method.method_info == body.method_info) {
                    if (owner != -1) return null;
                    owner = i;
                }
            for (var trait : abc.class_info.get(i).static_traits.traits)
                if (trait instanceof TraitMethodGetterSetter method && method.method_info == body.method_info)
                    return null;
        }
        String identity = names(abc).get(body.method_info);
        if (identity == null || identity.contains("|") || !identity.contains("/instance/")) return null;
        if (owner < 0 || body.init_scope_depth == 0 || body.exceptions.length != 0
                || !body.traits.traits.isEmpty()) return null;
        String name = publicName(abc, abc.instance_info.get(owner).name_index);
        if (name == null) return null;
        var cls = abc.getSwf().getAbcIndex().findClass(new com.jpexs.decompiler.graph.TypeItem(name), abc, null);
        Set<String> bindings = new HashSet<>(), seen = new HashSet<>();
        while (cls != null) {
            var instance = cls.abc.instance_info.get(cls.index);
            String current = publicName(cls.abc, instance.name_index);
            if (current == null || !seen.add(current)) return null;
            for (var trait : instance.instance_traits.traits) {
                if (cls.abc.constants.getMultiname(trait.name_index).kind != 7) return null;
                String member = publicName(cls.abc, trait.name_index);
                if (member != null) bindings.add(member);
            }
            if (instance.super_index == 0) return bindings;
            String parent = publicName(cls.abc, instance.super_index);
            if (cls.parent == null || parent == null || !parent.equals(publicName(cls.parent.abc,
                    cls.parent.abc.instance_info.get(cls.parent.index).name_index))) return null;
            cls = cls.parent;
        }
        return null;
    }

    static boolean globalClass(ABC abc, int name, Set<String> bindings) {
        String requested = publicName(abc, name);
        if (requested == null || bindings.contains(requested)) return false;
        int found = 0;
        for (var tag : abc.getSwf().getAbcList()) {
            var dependency = tag.getABC();
            for (var script : dependency.script_info) for (var trait : script.traits.traits)
                if (trait instanceof TraitClass && requested.equals(publicName(dependency, trait.name_index))) found++;
        }
        return found == 1;
    }

    static boolean methodReturn(AVM2Instruction instruction) {
        int opcode = instruction.definition.instructionCode;
        return opcode == 0x47 || opcode == 0x48;
    }

    // Native ASC hoists implicit local defaults above a source-level hook.
    // A single global static call receiving only this cannot observe these
    // uncaptured local registers. Delay only its pure default stores; neither
    // the call nor any executable instruction comes from the reference.
    static List<AVM2Instruction> hookDefaultsVariant(ABC abc, MethodBody body,
            List<AVM2Instruction> expected, List<AVM2Instruction> emitted) throws IOException {
        if (emitted.size() < 8 || expected.size() < 8
                || localRegister(emitted.get(0), false) != 0
                || emitted.get(1).definition.instructionCode != 0x30
                || localRegister(expected.get(0), false) != 0
                || expected.get(1).definition.instructionCode != 0x30) return emitted;
        Set<String> bindings = instanceBindings(abc, body);
        if (bindings == null) return emitted;
        for (var instruction : emitted)
            if (Set.of(0x1c,0x40,0x57).contains(instruction.definition.instructionCode)) return emitted;
        int params = abc.method_info.get(body.method_info).param_types.length, end = 2;
        Set<Integer> initialized = new HashSet<>();
        while (end + 1 < emitted.size()) {
            int next = end;
            var value = emitted.get(next++);
            int op = value.definition.instructionCode;
            if (op == 0x20) {
                var cast = emitted.get(next++);
                int conversion = cast.definition.instructionCode;
                if (conversion != 0x85 && !(conversion == 0x80
                        && ("Object".equals(publicName(abc,cast.operands[0]))
                            || "Array".equals(publicName(abc,cast.operands[0]))))) break;
            } else if (op == 0x21) {
                if (emitted.get(next++).definition.instructionCode != 0x82) break;
            } else if (op == 0x24 && value.operands[0] == 0) {
                if (emitted.get(next).definition.instructionCode == 0x74) next++;
            } else if (op != 0x27) break;
            if (next >= emitted.size()) break;
            int local = localRegister(emitted.get(next), true);
            if (local <= params || local >= body.max_regs || !initialized.add(local)) break;
            end = next + 1;
        }
        if (end == 2 || end + 4 >= emitted.size()) return emitted;
        var lookup = emitted.get(end);
        if (lookup.definition.instructionCode != 0x5d || lookup.operands.length != 1
                || !globalClass(abc,lookup.operands[0],bindings)) return emitted;
        var property = emitted.get(end + 1); var call = emitted.get(end + 3);
        if (property.definition.instructionCode != 0x66
                || !Arrays.equals(lookup.operands,property.operands)
                || localRegister(emitted.get(end + 2),false) != 0
                || call.definition.instructionCode != 0x46 || call.operands.length != 2
                || call.operands[1] != 1 || publicName(abc,call.operands[0]) == null
                || emitted.get(end + 4).definition.instructionCode != 0x29) return emitted;
        // The reference must place this same default block immediately after
        // either the compact or expanded spelling of that one call.
        boolean located = false;
        for (int start : new int[]{5,7}) {
            if (start + end - 2 > expected.size()) continue;
            boolean same = true;
            for (int j = 2; j < end; j++) {
                var a = emitted.get(j); var b = expected.get(start + j - 2);
                if (a.definition.instructionCode != b.definition.instructionCode
                        || !Arrays.equals(a.operands,b.operands)) same = false;
            }
            located |= same;
        }
        if (!located) return emitted;
        var boundaries = executablePositions(body.getCode().code);
        for (var instruction : emitted) for (int target : branchTargets(instruction))
            if (boundary(boundaries,target) < end + 5) return emitted;
        var result = new ArrayList<AVM2Instruction>();
        result.addAll(emitted.subList(0,2));
        result.addAll(emitted.subList(end,end + 5));
        result.addAll(emitted.subList(2,end));
        result.addAll(emitted.subList(end + 5,emitted.size()));
        return result;
    }

    // Omit or defer only the actual compiler's instance-scope prologue, across
    // a prefix whose lookups provably resolve without it. No nested scopes,
    // activations, handlers or incoming edges may defeat that proof. The full
    // method comparison below must still match every operand and CFG edge.
    static List<AVM2Instruction> scopeVariant(ABC abc, MethodBody body, List<AVM2Instruction> expected,
            List<AVM2Instruction> emitted) throws IOException {
        if (emitted.size() < 3 || localRegister(emitted.get(0), false) != 0
                || emitted.get(1).definition.instructionCode != 0x30
                || expected.isEmpty()
                || (expected.size() == 1 && expected.get(0).definition.instructionCode != 0x47)
                || (expected.size() >= 2 && localRegister(expected.get(0), false) == 0
                    && expected.get(1).definition.instructionCode == 0x30)) return emitted;
        int returns = 0, expectedScope = -1;
        for (int i = 0; i + 1 < expected.size(); i++) {
            if (localRegister(expected.get(i), false) == 0 && expected.get(i + 1).definition.instructionCode == 0x30) {
                expectedScope = i; break;
            }
            if (methodReturn(expected.get(i))) returns++;
        }
        int limit = emitted.size();
        if (expectedScope >= 0) {
            if (returns == 0 || !methodReturn(expected.get(expectedScope - 1))) return emitted;
            for (int i = 2; i < emitted.size(); i++) if (methodReturn(emitted.get(i)) && --returns == 0) {
                limit = i + 1; break;
            }
            if (returns != 0 || limit == emitted.size()) return emitted;
        }
        Set<String> bindings = instanceBindings(abc, body);
        if (bindings == null) return emitted;
        var boundaries = executablePositions(body.getCode().code);
        int params = abc.method_info.get(body.method_info).param_types.length;
        // Native compilation hoists implicit local defaults before source
        // guards. Only zero and null-Array stores may move past those guards:
        // both are stack-neutral, non-throwing and cannot invoke user code.
        // QName coerce types are already resolved by the verifier:
        // https://github.com/adobe/avmplus/blob/master/core/Interpreter.cpp#L3403
        // The crossed prefix may use only receiver/parameter registers; no
        // local reads/writes, activation, closures or handlers can observe the
        // delayed stores. Every moved instruction comes from compiled output.
        int defaultEnd = 2, defaultTarget = -1;
        Set<Integer> initialized = new HashSet<>();
        while (defaultEnd + 1 < emitted.size()) {
            int next = defaultEnd;
            var value = emitted.get(next++);
            if (value.definition.instructionCode == 0x20) {
                var cast = emitted.get(next++);
                if (cast.definition.instructionCode != 0x80 || !"Array".equals(publicName(abc, cast.operands[0]))) break;
            } else if (value.definition.instructionCode != 0x24 || value.operands[0] != 0) break;
            if (next >= emitted.size()) break;
            int local = localRegister(emitted.get(next), true);
            if (local <= params || local >= body.max_regs || !initialized.add(local)) break;
            defaultEnd = next + 1;
        }
        if (defaultEnd > 2) {
            int prefixReturns = 0;
            for (int i = 0; i + defaultEnd - 2 <= expected.size(); i++) {
                if (i > 0 && prefixReturns > 0 && (methodReturn(expected.get(i - 1))
                        || (expectedScope >= 0 && i == expectedScope + 2))) {
                    boolean same = true;
                    for (int j = 2; j < defaultEnd; j++) {
                        var a = emitted.get(j); var b = expected.get(i + j - 2);
                        if (a.definition.instructionCode != b.definition.instructionCode
                                || !Arrays.equals(a.operands, b.operands)) same = false;
                    }
                    if (same) {
                        int left = prefixReturns;
                        for (int j = defaultEnd; j < emitted.size(); j++)
                            if (methodReturn(emitted.get(j)) && --left == 0) {
                                defaultTarget = j + 1; break;
                            }
                        break;
                    }
                }
                if (methodReturn(expected.get(i))) prefixReturns++;
            }
            if (defaultTarget <= defaultEnd || defaultTarget >= emitted.size()) return emitted;
        }
        for (int i = 0; i < emitted.size(); i++) {
            var instruction = emitted.get(i);
            for (int target : branchTargets(instruction)) {
                int targetIndex = boundary(boundaries, target);
                if (targetIndex < defaultEnd || (i >= limit && targetIndex < limit)
                        || (i < limit && targetIndex > limit)
                        || (defaultTarget >= 0 && ((i >= defaultTarget && targetIndex < defaultTarget)
                            || (i >= defaultEnd && i < defaultTarget && targetIndex > defaultTarget)))) return emitted;
            }
            if (i < defaultEnd || i >= Math.max(limit, defaultTarget)) continue;
            int op = instruction.definition.instructionCode;
            int local = localRegister(instruction, false);
            if (local >= 0) { if (local > params) return emitted; continue; }
            if (op == 0x5d || op == 0x60) {
                if (!globalClass(abc, instruction.operands[0], bindings)) return emitted;
                continue;
            }
            if (!Set.of(0x10,0x11,0x12,0x20,0x21,0x24,0x25,0x26,0x27,0x28,0x29,
                    0x2c,0x2d,0x2e,0x2f,0x46,0x47,0x48,0x4f,0x66,0x70,0x73,0x74,0x75,0x76,0x80,0x85).contains(op))
                return emitted;
        }
        List<AVM2Instruction> result = new ArrayList<>();
        for (int i = defaultEnd; i < emitted.size(); i++) {
            if (i == limit) result.addAll(emitted.subList(0, 2));
            if (i == defaultTarget) result.addAll(emitted.subList(2, defaultEnd));
            var old = emitted.get(i);
            var instruction = new AVM2Instruction(old.getAddress(), old.definition, old.operands == null ? null : old.operands.clone());
            if (limit < emitted.size() && i < limit) {
                int[] targets = branchTargets(instruction);
                if (targets.length == 1 && boundary(boundaries, targets[0]) == limit)
                    instruction.operands[0] = (int)emitted.get(0).getAddress()
                        - (int)instruction.getAddress() - instruction.getBytesLength();
            }
            if (defaultTarget >= 0 && i < defaultTarget) {
                int[] targets = branchTargets(old);
                if (targets.length == 1 && boundary(boundaries, targets[0]) == defaultTarget && defaultTarget != limit)
                    instruction.operands[0] = (int)emitted.get(2).getAddress()
                        - (int)instruction.getAddress() - instruction.getBytesLength();
            }
            result.add(instruction);
        }
        return result;
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
            MethodBody reference = body.method_info < original.method_info.size() ? original.findBody(body.method_info) : null;
            if (reference == null || reference.exceptions.length != body.exceptions.length) continue;
            var layout = reference.getCode().code;
            var expected = layout.stream().filter(i -> !debugInstruction(i)).toList();
            var emittedLayout = body.getCode().code;
            var emitted = emittedLayout.stream().filter(i -> !debugInstruction(i)).toList();
            var originalEmitted = emitted;
            emitted = scopeVariant(compiled, body, expected, emitted);
            emitted = hookDefaultsVariant(compiled, body, expected, emitted);
            ArrayList<AVM2Instruction> result = new ArrayList<>();
            List<int[]> targets = new ArrayList<>();
            Map<Integer, Integer> positions = new HashMap<>();
            Set<Integer> deadJumps = null;
            boolean matching = true, rewritten = emitted != originalEmitted;
            int next = 0, end = 0;
            for (int wantedIndex = 0; wantedIndex < expected.size(); wantedIndex++) {
                var wanted = expected.get(wantedIndex);
                if (next == emitted.size()) { matching = false; break; }
                var instruction = emitted.get(next);
                var operandSource = instruction;
                int opcode = instruction.definition.instructionCode, desired = wanted.definition.instructionCode;
                // Recreate the expanded spelling from the compiler's own
                // compact lookup/call. No reference operand is transferred,
                // and the final CFG checks forbid entry into the new interior.
                if (wantedIndex + 1 < expected.size()) {
                    var following = expected.get(wantedIndex + 1);
                    boolean lookupExpansion = opcode == 0x60 && desired == 0x5d
                        && instruction.operands.length == 1
                        && compiled.constants.getMultiname(instruction.operands[0]).kind == 7
                        && following.definition.instructionCode == 0x66
                        && Arrays.equals(instruction.operands,wanted.operands)
                        && Arrays.equals(instruction.operands,following.operands);
                    boolean callExpansion = ((opcode == 0x4f && desired == 0x46)
                            || (opcode == 0x4e && desired == 0x45))
                        && following.definition.instructionCode == 0x29
                        && Arrays.equals(instruction.operands,wanted.operands)
                        && (following.operands == null || following.operands.length == 0);
                    boolean stringExpansion = opcode == 0x2c && desired == 0x2c
                        && following.definition.instructionCode == 0x85
                        && !(next + 1 < emitted.size()
                            && emitted.get(next + 1).definition.instructionCode == 0x85)
                        && Arrays.equals(instruction.operands, wanted.operands);
                    if (lookupExpansion || callExpansion || stringExpansion) {
                        positions.put((int)instruction.getAddress(),result.size());
                        result.add(new AVM2Instruction(0,desired,instruction.operands.clone()));
                        result.add(new AVM2Instruction(0,lookupExpansion ? 0x66 : stringExpansion ? 0x85 : 0x29,
                            lookupExpansion ? instruction.operands.clone() : null));
                        targets.add(new int[0]); targets.add(new int[0]);
                        end = (int)instruction.getAddress() + instruction.getBytesLength();
                        next++; wantedIndex++; rewritten = true;
                        continue;
                    }
                }
                // A local store followed by a read of the same register can
                // keep its value on the stack with dup/store. No coercion or
                // user code occurs between them. The intermediate boundary is
                // deliberately absent from positions, so edges and handlers
                // cannot enter either spelling halfway through the pair.
                int storedRegister = localRegister(instruction, true);
                if (desired == 0x2a && storedRegister >= 0 && storedRegister < body.max_regs
                        && next + 1 < emitted.size() && wantedIndex + 1 < expected.size()
                        && localRegister(emitted.get(next + 1), false) == storedRegister
                        && expected.get(wantedIndex + 1).definition.instructionCode == opcode
                        && Arrays.equals(expected.get(wantedIndex + 1).operands, instruction.operands)) {
                    positions.put((int)instruction.getAddress(), result.size());
                    result.add(new AVM2Instruction(0, 0x2a, null));
                    result.add(new AVM2Instruction(0, opcode,
                        instruction.operands == null ? null : instruction.operands.clone()));
                    targets.add(new int[0]); targets.add(new int[0]);
                    var last = emitted.get(next + 1);
                    end = (int)last.getAddress() + last.getBytesLength();
                    next += 2; wantedIndex++; rewritten = true;
                    continue;
                }
                // The interpreter implements inclocal_i/declocal_i using the
                // same operation as increment_i/decrement_i on a stack value.
                // The extra convert_i is idempotent on that integer result.
                // Only expand when the existing stack bound accommodates the
                // reference encoding; never raise verifier limits to force it.
                if ((opcode == 0xc2 || opcode == 0xc3) && instruction.operands.length == 1
                        && body.max_stack >= reference.max_stack && wantedIndex + 3 < expected.size()) {
                    int register = instruction.operands[0];
                    var expanded = List.of(
                        new AVM2Instruction(0, register < 4 ? 0xd0 + register : 0x62,
                            register < 4 ? null : new int[]{register}),
                        new AVM2Instruction(0, opcode == 0xc2 ? 0xc0 : 0xc1, null),
                        new AVM2Instruction(0, 0x73, null),
                        new AVM2Instruction(0, register < 4 ? 0xd4 + register : 0x63,
                            register < 4 ? null : new int[]{register}));
                    boolean same = register >= 0 && register < body.max_regs;
                    for (int j = 0; j < expanded.size(); j++) {
                        var item = expanded.get(j);
                        var target = expected.get(wantedIndex + j);
                        if (item.definition.instructionCode != target.definition.instructionCode
                                || !Arrays.equals(item.operands, target.operands)) same = false;
                    }
                    if (same) {
                        positions.put((int)instruction.getAddress(), result.size());
                        for (var item : expanded) {
                            result.add(item);
                            targets.add(new int[0]);
                        }
                        end = (int)instruction.getAddress() + instruction.getBytesLength();
                        next++;
                        wantedIndex += 3;
                        rewritten = true;
                        continue;
                    }
                }
                // Some generators omit the redundant conversion before storing
                // an increment_i/decrement_i result. It cannot invoke user code.
                // Incoming edges still target the original store, and all CFG
                // and exception boundaries must agree below.
                if (desired == 0x73 && next > 0
                        && (emitted.get(next - 1).definition.instructionCode == 0xc0
                            || emitted.get(next - 1).definition.instructionCode == 0xc1)
                        && (opcode == 0x6d || localRegister(instruction, true) >= 0)
                        && wantedIndex + 1 < expected.size()
                        && expected.get(wantedIndex + 1).definition.instructionCode == opcode
                        && Arrays.equals(expected.get(wantedIndex + 1).operands, instruction.operands)) {
                    result.add(new AVM2Instruction(0, 0x73, null));
                    targets.add(new int[0]);
                    rewritten = true;
                    continue;
                }
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
                // A string literal is already a String. Its coerce_s cannot
                // invoke user code. Interior CFG/handler entries are rejected
                // by the same whole-method checks as the folds below.
                if (opcode == 0x2c && desired == opcode && next + 1 < emitted.size()
                        && emitted.get(next + 1).definition.instructionCode == 0x85
                        && !(wantedIndex + 1 < expected.size()
                            && expected.get(wantedIndex + 1).definition.instructionCode == 0x85)) {
                    consumed = 2;
                    rewritten = true;
                }
                // A successful coerce already has the type resolved by this
                // pool entry. Repeating that identical coercion is idempotent.
                // Interior branch/handler entries are rejected below.
                if (opcode == 0x80 && desired == opcode && next + 1 < emitted.size()
                        && instruction.operands.length == 1
                        && compiled.constants.getMultiname(instruction.operands[0]).kind == 7
                        && emitted.get(next + 1).definition.instructionCode == opcode
                        && Arrays.equals(instruction.operands, emitted.get(next + 1).operands)
                        && !(wantedIndex + 1 < expected.size()
                            && expected.get(wantedIndex + 1).definition.instructionCode == opcode
                            && Arrays.equals(wanted.operands, expected.get(wantedIndex + 1).operands))) {
                    consumed = 2;
                    rewritten = true;
                }
                if (opcode != desired) {
                    if (next + 1 >= emitted.size()) { matching = false; break; }
                    var following = emitted.get(next + 1);
                    boolean lookup = desired == 0x60 && opcode == 0x5d
                        && following.definition.instructionCode == 0x66
                        && Arrays.equals(instruction.operands, following.operands)
                        && instruction.operands.length == 1
                        && compiled.constants.getMultiname(instruction.operands[0]).kind == 0x07;
                    boolean discardedCall = ((desired == 0x4f && opcode == 0x46)
                        || (desired == 0x4e && opcode == 0x45))
                        && following.definition.instructionCode == 0x29;
                    // Both branch forms perform the same Boolean conversion.
                    // Logical negation only reverses the branch polarity.
                    int truthBranch = next + 1;
                    if (opcode == 0x96 && truthBranch < emitted.size()
                            && emitted.get(truthBranch).definition.instructionCode == 0x76) truthBranch++;
                    boolean compactTruth = (desired == 0x11 || desired == 0x12)
                        && (opcode == 0x96 || opcode == 0x76) && truthBranch < emitted.size()
                        && emitted.get(truthBranch).definition.instructionCode ==
                           (opcode == 0x96 ? (desired == 0x11 ? 0x12 : 0x11) : desired);
                    if (((opcode == 0x24 && desired == 0x25) || (opcode == 0x25 && desired == 0x24))
                            && instruction.operands.length == 1 && wanted.operands.length == 1
                            && instruction.operands[0] >= -128 && instruction.operands[0] <= 127
                            && instruction.operands[0] == wanted.operands[0]) {
                        // Signed byte and signed short immediates push the same
                        // integer in their common range. Keep the actual value;
                        // only its encoding width follows the reference.
                    } else if (opcode == 0x61 && desired == 0x68
                            && mutableInstanceSlot(compiled, body, emitted, next)) {
                        // initproperty and setproperty both assign an ordinary
                        // mutable slot. Const fields, setters, dynamic names,
                        // foreign receivers and inherited slots are excluded.
                    } else if (redundantNumericSelfAssignment(emitted, next, desired)) {
                        consumed = 4;
                        operandSource = emitted.get(next + 3);
                    } else if (lookup || discardedCall) consumed = 2;
                    else if (compactTruth) {
                        consumed = truthBranch - next + 1;
                        operandSource = emitted.get(truthBranch);
                    }
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
                // FFDec emits a jump past catch handlers even after return.
                // A proven unreachable jump can use the fixed zero displacement
                // emitted by older compilers. Do not borrow a reference target:
                // use the next compiled boundary and validate the entire CFG.
                if (opcode == 0x10 && desired == 0x10 && wanted.operands.length == 1
                        && wanted.operands[0] == 0 && instruction.operands[0] != 0) {
                    if (deadJumps == null) deadJumps = unreachableJumps(body);
                    if (deadJumps.contains((int)instruction.getAddress())) {
                        branches = new int[]{(int)instruction.getAddress() + instruction.getBytesLength()};
                        rewritten = true;
                    }
                }
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
            // Compiler debugger directives do not execute. A branch/handler at
            // such a directive belongs to its next executable boundary. Only
            // boundaries already retained above may acquire these aliases;
            // directives inside a folded group cannot bypass the entry checks.
            int sourceEnd = emittedLayout.isEmpty() ? 0 : (int)emittedLayout.get(emittedLayout.size() - 1).getAddress()
                + emittedLayout.get(emittedLayout.size() - 1).getBytesLength();
            positions.put(sourceEnd, result.size());
            for (var entry : executablePositions(emittedLayout).entrySet()) {
                int address = entry.getValue() == originalEmitted.size() ? sourceEnd
                    : (int)originalEmitted.get(entry.getValue()).getAddress();
                Integer target = positions.get(address);
                if (target != null) positions.put(entry.getKey(), target);
            }
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
            MethodBody before = body.method_info < original.method_info.size() ? original.findBody(body.method_info) : null;
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
        boolean feature = permissions != null;
        if (feature) FeatureGate.restoreClassOrder(original, compiled);
        restoreNameReferences(original, compiled);
        if (feature) relocateAddedPoolNames(original, compiled);
        restoreMethodOrder(original, compiled, feature);
        emitScriptClassBindings(original, compiled, feature);
        restoreScriptSlot(original, compiled);
        emitCompactOperations(original, compiled);
        retainDebugDirectives(original, compiled);
        if (compiled.metadata_info.size() < original.metadata_info.size()) throw new IOException("Metadata pool compacted");
        for (int i = 0; i < original.metadata_info.size(); i++) {
            MetadataInfo a = original.metadata_info.get(i), b = compiled.metadata_info.get(i);
            if (a.name_index != b.name_index || !Arrays.equals(a.keys, b.keys) || !Arrays.equals(a.values, b.values))
                throw new IOException("Existing metadata pool entry changed");
        }
        if (feature ? original.instance_info.size() > compiled.instance_info.size()
                    || original.script_info.size() > compiled.script_info.size()
                    || original.bodies.size() > compiled.bodies.size()
                : original.instance_info.size() != compiled.instance_info.size()
                    || original.script_info.size() != compiled.script_info.size()
                    || original.bodies.size() != compiled.bodies.size())
            throw new IOException("ABC record counts changed");
        if (feature) FeatureGate.checkScriptOrder(original, compiled);
        for (int i = 0; i < original.instance_info.size(); i++) {
            retainDebugMetadata(original, compiled, original.instance_info.get(i).instance_traits, compiled.instance_info.get(i).instance_traits, feature);
            retainDebugMetadata(original, compiled, original.class_info.get(i).static_traits, compiled.class_info.get(i).static_traits, feature);
        }
        for (int i = 0; i < original.script_info.size(); i++)
            retainDebugMetadata(original, compiled, original.script_info.get(i).traits, compiled.script_info.get(i).traits, feature);
        for (int i = 0; i < original.bodies.size(); i++) {
            if (original.bodies.get(i).method_info != compiled.bodies.get(i).method_info) throw new IOException("Method body ordering changed");
            Traits before = original.bodies.get(i).traits, after = compiled.bodies.get(i).traits;
            // A feature's edited method may declare different activation
            // locals; its whole body record is then checked by the gate.
            if (feature && !traitIdentities(original, before).equals(traitIdentities(compiled, after))) continue;
            retainDebugMetadata(original, compiled, before, after, feature);
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
        // Every original metadata record was checked above; a feature's new
        // traits may reference records appended after them.
        if (!feature || compiled.metadata_info.size() == original.metadata_info.size())
            compiled.metadata_info = original.metadata_info;
        byte[] rebuilt = abcBytes(compiled), expected = abcBytes(original);
        int classId = original.findClassByName(cls);
        String ownerPrefix = classId >= 0 ? name(original, original.instance_info.get(classId).name_index) + "/"
            : "script:" + original.script_info.get(source.scriptIndex).traits.traits.stream()
                .map(t -> name(original, t.name_index)).sorted().toList() + "/";
        String featureReport = permissions == null ? null : FeatureGate.check(original, compiled, ownerPrefix, permissions);
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
        if (a.length < 3) throw new IllegalArgumentException("export|snapshot|compile INPUT OUTPUT [CLASS SOURCE COMPILER SDK]");
        Configuration.parallelSpeedUp.set(false);
        if (a[0].equals("reserve")) {
            if (a.length != 6) throw new IllegalArgumentException("reserve INPUT OUTPUT CLASS PLAN SNAPSHOT");
            CompilerReserves.apply(Path.of(a[1]), Path.of(a[2]), a[3], Path.of(a[4]), Path.of(a[5]));
            return;
        }
        if (a[0].equals("link-feature")) {
            if (a.length != 6) throw new IllegalArgumentException("link-feature ORIGINAL COMPILED OUTPUT CLASS PERMISSIONS.tsv");
            linkLayout(Path.of(a[1]), Path.of(a[2]), Path.of(a[3]), a[4], Path.of(a[5]));
            return;
        }
        if (a[0].equals("add-script")) {
            if (a.length != 6) throw new IllegalArgumentException("add-script INPUT OUTPUT ANCHOR_CLASS SOURCE SNAPSHOT");
            FeatureGate.addScript(Path.of(a[1]), Path.of(a[2]), a[3], Path.of(a[4]));
            snapshot(read(Path.of(a[2])), Path.of(a[5]));
            return;
        }
        if (a[0].equals("remove-script")) {
            if (a.length != 6) throw new IllegalArgumentException("remove-script INPUT OUTPUT CLASS REPORT.tsv SNAPSHOT");
            FeatureGate.removeScript(Path.of(a[1]), Path.of(a[2]), a[3], Path.of(a[4]));
            snapshot(read(Path.of(a[2])), Path.of(a[5]));
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
        if (a[0].equals("compile") && a.length >= 7 && a[6].equals("flex-mixed")) {
            if (a.length != 9) throw new IllegalArgumentException("Mixed compilation requires a selection file");
            Path snapshot = Path.of(a[5]);
            Path diagnostics = snapshot.resolveSibling(snapshot.getFileName() + "-mixed");
            Files.createDirectory(diagnostics);
            Path source = diagnostics.resolve("source.as"), selection = diagnostics.resolve("composition.tsv");
            byte[] sourceBytes = Files.readAllBytes(Path.of(a[4]));
            Files.write(source, sourceBytes, StandardOpenOption.CREATE_NEW);
            Files.copy(Path.of(a[8]), selection);
            String secondaryCompiler = Files.readAllLines(selection).contains("ffdec_compiler=ffdec-source")
                ? "ffdec-source" : "ffdec";
            for (String compiler : List.of("flex-library", secondaryCompiler)) {
                var arguments = new ArrayList<>(List.of("compile", a[1], diagnostics.resolve(compiler + ".swf").toString(),
                    a[3], source.toString(), diagnostics.resolve(compiler + "-snapshot").toString(), compiler, a[7]));
                if (compiler.equals("ffdec-source")) arguments.add(selection.toString());
                run(arguments.toArray(String[]::new));
                if (!Arrays.equals(sourceBytes, Files.readAllBytes(source)))
                    throw new IOException("Mixed compiler source changed during compilation");
            }
            MixedCompiler.compose(new String[]{a[1], diagnostics.resolve("flex-library.swf").toString(),
                diagnostics.resolve(secondaryCompiler + ".swf").toString(), a[3], selection.toString(), a[2]});
            snapshot(read(Path.of(a[2])), snapshot);
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
                if (a.length != 8 && !(a.length == 9 && a[6].equals("ffdec-source")))
                    throw new IllegalArgumentException("compile INPUT OUTPUT CLASS SOURCE COMPILER SDK [SOURCE_PLAN]");
                ScriptPack found = null;
                for (ScriptPack pack : swf.getAS3Packs())
                    if (pack.getClassPath().toRawString().equals(a[3])) {
                        if (found != null) throw new IOException("Ambiguous script " + a[3]);
                        found = pack;
                    }
                if (found == null) throw new IOException("Missing script " + a[3]);
                Configuration.flexSdkLocation.set(a[7]);
                As3ScriptReplacerInterface compiler;
                if (a[6].equals("flex") || a[6].equals("flex-strict") || a[6].equals("flex-library") || a[6].equals("flex-optimized"))
                    compiler = new FlexCompiler(a[7], Path.of(a[5]).resolveSibling(Path.of(a[5]).getFileName() + "-flex"),
                        !a[6].equals("flex"), (a[6].equals("flex-library") || a[6].equals("flex-optimized")) ? a[3] : null,
                        a[6].equals("flex-optimized"));
                else if (a[6].equals("ffdec")) compiler = As3ScriptReplacerFactory.createFFDec();
                else if (a[6].equals("ffdec-source")) compiler = new SourceCompiler(a.length == 9
                    ? MixedCompiler.hiddenStaticMembers(Files.readAllLines(Path.of(a[8]))) : List.of());
                else throw new IllegalArgumentException("Unknown compiler " + a[6]);
                try {
                    if (!found.abc.replaceScriptPack(compiler, found, Files.readString(Path.of(a[4])), List.of()))
                        throw new IOException("Compiler refused " + a[3]);
                    ((Tag)found.abc.parentTag).setModified(true);
                    if (compiler instanceof FlexCompiler flex)
                        swf = importNative(Path.of(a[1]), flex.diagnostics.resolve(
                            flex.libraryClass == null ? "native-compiler.swf" : "native-script.swf"), a[3]);
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

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
                for (int i = 0; i < scripts.getLength(); i++) {
                    var script = (org.w3c.dom.Element)scripts.item(i);
                    var defs = script.getElementsByTagName("def");
                    Set<String> deps = new TreeSet<>();
                    for (int j = 0; j < defs.getLength(); j++)
                        deps.addAll(inheritance.getOrDefault(((org.w3c.dom.Element)defs.item(j)).getAttribute("id"), List.of()));
                    for (String base : deps) {
                        var dep = doc.createElement("dep"); dep.setAttribute("id", base); dep.setAttribute("type", "i");
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

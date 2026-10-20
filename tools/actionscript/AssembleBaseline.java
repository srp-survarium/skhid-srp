import java.io.*;
import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.abc.ABC;
import com.jpexs.decompiler.flash.abc.ScriptPack;
import com.jpexs.decompiler.flash.abc.types.*;
import com.jpexs.decompiler.flash.abc.types.traits.*;
import com.jpexs.decompiler.flash.tags.Tag;

/** Assemble every script's records from its own authenticated linked compilation. */
public final class AssembleBaseline {
    static final class Owned {
        final ABC abc;
        final Set<Integer> methods = new TreeSet<>(), classes = new TreeSet<>();
        Owned(ABC abc, int script) throws Exception {
            this.abc = abc;
            ScriptInfo info = abc.script_info.get(script);
            method(info.init_index);
            traits(info.traits);
        }
        void traits(Traits traits) throws Exception {
            for (Trait trait : traits.traits) {
                if (trait instanceof TraitClass c) clazz(c.class_info);
                else if (trait instanceof TraitMethodGetterSetter m) method(m.method_info);
                else if (trait instanceof TraitFunction f) method(f.method_info);
            }
        }
        void clazz(int id) throws Exception {
            if (id < 0 || id >= abc.class_info.size()) throw new IOException("Invalid class reference");
            if (!classes.add(id)) return;
            InstanceInfo instance = abc.instance_info.get(id);
            ClassInfo cls = abc.class_info.get(id);
            method(instance.iinit_index);
            method(cls.cinit_index);
            traits(instance.instance_traits);
            traits(cls.static_traits);
        }
        void method(int id) throws Exception {
            if (id < 0 || id >= abc.method_info.size()) throw new IOException("Invalid method reference");
            if (!methods.add(id)) return;
            MethodBody body = abc.findBody(id);
            if (body == null) return; // Interface declarations have signatures but no body.
            traits(body.traits);
            for (var instruction : MovieSource.exactCode(body).code) {
                int op = instruction.definition.instructionCode;
                if (op == 0x40) method(instruction.operands[0]); // newfunction
                if (op == 0x58) clazz(instruction.operands[0]);  // newclass
            }
        }
    }

    static <T> List<T> empty(int size) {
        return new ArrayList<>(Collections.nCopies(size, null));
    }
    static <T> void put(List<T> records, int index, T value) throws IOException {
        if (value == null || records.get(index) != null)
            throw new IOException("Missing or multiply owned compiled record at " + index);
        records.set(index, value);
    }
    static void complete(List<?> records, String kind, int block) throws IOException {
        if (records.contains(null)) throw new IOException("Uncompiled " + kind + " in ABC " + block);
    }

    static void assemble(Path referencePath, Path plan, Path output, Path coverage) throws Exception {
        var reference = MovieSource.read(referencePath);
        var destination = MovieSource.read(referencePath);
        Map<String, ScriptPack> packs = new LinkedHashMap<>();
        for (ScriptPack pack : reference.getAS3Packs())
            if (packs.put(pack.getClassPath().toRawString(), pack) != null)
                throw new IOException("Ambiguous exported script");
        ABC[] assembled = new ABC[reference.getAbcList().size()];
        Set<String> seen = new HashSet<>();
        StringBuilder evidence = new StringBuilder("class\tabc\tscript\tmethods\tbodies\tclasses\tlinked_sha256\n");
        for (String line : Files.readAllLines(plan)) {
            String[] row = line.split("\t", -1);
            if (row.length != 3 || !seen.add(row[0]) || !packs.containsKey(row[0]))
                throw new IOException("Unknown or duplicate assembly input");
            byte[] bytes = Files.readAllBytes(Path.of(row[1]));
            if (!MovieSource.hash(bytes).equals(row[2])) throw new IOException("Linked artifact changed");
            var movie = new com.jpexs.decompiler.flash.SWF(new ByteArrayInputStream(bytes), true);
            if (movie.getAbcList().size() != assembled.length) throw new IOException("ABC count changed");
            ScriptPack pack = packs.get(row[0]);
            int block = -1;
            for (int i = 0; i < assembled.length; i++)
                if (reference.getAbcList().get(i).getABC() == pack.abc) block = i;
            if (block < 0) throw new IOException("Script has no ABC");
            ABC source = movie.getAbcList().get(block).getABC();
            if (!Arrays.equals(MovieSource.abcBytes(pack.abc), MovieSource.abcBytes(source)))
                throw new IOException("Linked ABC is not an exact baseline: " + row[0]);
            ABC target = assembled[block];
            if (target == null) {
                target = new ABC(destination.getAbcList().get(block));
                target.version = source.version;
                target.constants = source.constants;
                target.metadata_info = source.metadata_info;
                // Start with no executable definitions. No original body or
                // another script's retained body can fill an uncompiled slot.
                target.method_info = empty(source.method_info.size());
                target.instance_info = empty(source.instance_info.size());
                target.class_info = empty(source.class_info.size());
                target.script_info = empty(source.script_info.size());
                target.bodies = empty(source.bodies.size());
                assembled[block] = target;
            }
            Owned owned = new Owned(source, pack.scriptIndex);
            ScriptInfo script = source.script_info.get(pack.scriptIndex);
            script.clearPacksCache();
            put(target.script_info, pack.scriptIndex, script);
            for (int id : owned.classes) {
                put(target.instance_info, id, source.instance_info.get(id));
                put(target.class_info, id, source.class_info.get(id));
            }
            int bodies = 0;
            for (int id : owned.methods) {
                put(target.method_info, id, source.method_info.get(id));
                int position = source.findBodyIndex(id);
                if (position >= 0) {
                    MethodBody body = source.bodies.get(position);
                    body.setAbc(target);
                    put(target.bodies, position, body);
                    bodies++;
                }
            }
            evidence.append(row[0]).append('\t').append(block).append('\t').append(pack.scriptIndex)
                .append('\t').append(owned.methods.size()).append('\t').append(bodies)
                .append('\t').append(owned.classes.size()).append('\t').append(row[2]).append('\n');
        }
        if (!seen.equals(packs.keySet())) throw new IOException("Assembly does not cover every exported script");
        for (int i = 0; i < assembled.length; i++) {
            ABC abc = assembled[i];
            if (abc == null) throw new IOException("Uncompiled ABC " + i);
            complete(abc.method_info, "method signature", i);
            complete(abc.bodies, "method body", i);
            complete(abc.instance_info, "instance", i);
            complete(abc.class_info, "class", i);
            complete(abc.script_info, "script", i);
            if (!Arrays.equals(MovieSource.abcBytes(abc), MovieSource.abcBytes(reference.getAbcList().get(i).getABC())))
                throw new IOException("Assembled ABC differs at " + i);
            var tag = destination.getAbcList().get(i);
            tag.setABC(abc);
            ((Tag)tag).setModified(true);
        }
        ByteArrayOutputStream serialized = new ByteArrayOutputStream();
        destination.saveTo(serialized);
        var saved = new com.jpexs.decompiler.flash.SWF(new ByteArrayInputStream(serialized.toByteArray()), true);
        for (int i = 0; i < assembled.length; i++)
            if (!Arrays.equals(MovieSource.abcBytes(assembled[i]), MovieSource.abcBytes(saved.getAbcList().get(i).getABC())))
                throw new IOException("Serialization changed assembled ABC " + i);
        Files.write(output, serialized.toByteArray(), StandardOpenOption.CREATE_NEW);
        Files.writeString(coverage, evidence, StandardOpenOption.CREATE_NEW);
        System.out.println("Assembled " + seen.size() + " compiled scripts across " + assembled.length + " ABCs");
    }

    public static void main(String[] args) throws Exception {
        assemble(Path.of(args[0]), Path.of(args[1]), Path.of(args[2]), Path.of(args[3]));
        System.exit(0);
    }
}

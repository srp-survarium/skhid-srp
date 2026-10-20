import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.HashSet;
import com.jpexs.decompiler.flash.tags.Tag;

/** Explicit compiler capacity reservations, independent of a reference movie. */
public final class CompilerReserves {
    static int capacity(String value) throws IOException {
        if (!value.matches("0|[1-9][0-9]{0,4}")) throw new IOException("Invalid reserved capacity");
        int result = Integer.parseInt(value);
        if (result > 65535) throw new IOException("Reserved capacity exceeds 65535");
        return result;
    }

    public static void apply(Path input, Path output, String cls, Path plan, Path snapshot) throws Exception {
        var requests = new LinkedHashMap<String, int[]>();
        for (String line : Files.readAllLines(plan)) {
            String[] fields = line.split("\t", -1);
            if (fields.length != 3 || fields[0].isEmpty() || !fields[0].equals(fields[0].strip())
                    || fields[0].contains("|") || requests.containsKey(fields[0]))
                throw new IOException("Invalid or duplicate method reservation");
            int stack = capacity(fields[1]), locals = capacity(fields[2]);
            if (stack == 0 && locals == 0) throw new IOException("Empty method reservation");
            requests.put(fields[0], new int[]{stack, locals});
        }
        if (requests.isEmpty()) throw new IOException("No method reservations");
        var movie = MovieSource.read(input);
        var abc = MovieSource.findScript(movie, cls).abc;
        int classId = abc.findClassByName(cls);
        if (classId < 0) throw new IOException("Reservations require a class");
        String prefix = MovieSource.name(abc, abc.instance_info.get(classId).name_index) + "/";
        var owners = MovieSource.names(abc);
        var seen = new HashSet<String>();
        StringBuilder changes = new StringBuilder("method\tstack_before\tstack_after\tlocals_before\tlocals_after\n");
        for (var body : abc.bodies) {
            String owner = owners.get(body.method_info);
            int[] reserve = requests.get(owner);
            if (reserve == null) continue;
            if (!owner.startsWith(prefix) || !seen.add(owner)) throw new IOException("Ambiguous reserved method");
            int stack = Math.max(body.max_stack, reserve[0]), locals = Math.max(body.max_regs, reserve[1]);
            changes.append(owner).append('\t').append(body.max_stack).append('\t').append(stack)
                .append('\t').append(body.max_regs).append('\t').append(locals).append('\n');
            body.max_stack = stack;
            body.max_regs = locals;
        }
        if (!seen.equals(requests.keySet())) throw new IOException("Unknown reserved method");
        ((Tag)abc.parentTag).setModified(true);
        try (var stream = Files.newOutputStream(output)) { movie.saveTo(stream); }
        MovieSource.snapshot(MovieSource.read(output), snapshot);
        Files.writeString(snapshot.resolve("reservations.tsv"), changes);
    }
}

// Compile SrpPlayerGear into LobbyMenu's ABC block and add two calls to it:
//   LobbyMenu.fill_victory_screen_player start  if (SrpPlayerGear.take(this, value)) return;
//   PlayerRanking.onActivate             end    SrpPlayerGear.ranking(this)
// Every other method of the blocks is checked byte for byte.
import java.io.*;
import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.abc.ABC;
import com.jpexs.decompiler.flash.abc.avm2.AVM2Code;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.abc.avm2.parser.script.ActionScript3Parser;
import com.jpexs.decompiler.flash.abc.types.MethodBody;
import com.jpexs.decompiler.flash.tags.Tag;

public class PatchPlayerGear {
    private static final int GETLEX = 0x60, GETLOCAL0 = 0xd0, GETLOCAL1 = 0xd1, CALLPROPVOID = 0x4f,
        CALLPROPERTY = 0x46, IFFALSE = 0x12, RETURNVOID = 0x47;
    private static final String HELPER = "SrpPlayerGear";

    private static AVM2Instruction ins(int opcode, int... operands) {
        return new AVM2Instruction(0, opcode, operands);
    }

    // `SrpPlayerGear.<name>(this)` before the method's one returnvoid, its last.
    private static void callAtEnd(ABC abc, MethodBody body, String name) throws Exception {
        AVM2Code code = body.getCode();
        int end = code.code.size() - 1;
        int returns = 0;
        for (AVM2Instruction op : code.code)
            if (op.definition.instructionCode == RETURNVOID) returns++;
        if (returns != 1 || code.code.get(end).definition.instructionCode != RETURNVOID
                || body.exceptions.length != 0)
            throw new IOException("Unexpected method layout before " + name);
        code.insertInstruction(end++, ins(GETLEX, abc.constants.getPublicQnameId(HELPER, true)), body);
        code.insertInstruction(end++, ins(GETLOCAL0), body);
        code.insertInstruction(end, ins(CALLPROPVOID, abc.constants.getPublicQnameId(name, true), 1), body);
        body.max_stack = Math.max(body.max_stack, 2);
        body.setCode(code);
        body.getCode().checkValidOffsets(body);
    }

    // `if (SrpPlayerGear.<name>(this, arg1)) return;` before the first instruction.
    private static void guard(ABC abc, MethodBody body, String name) throws Exception {
        if (body.exceptions.length != 0)
            throw new IOException("Unexpected exceptions in the method of " + name);
        AVM2Code code = body.getCode();
        code.insertInstruction(0, ins(RETURNVOID), body);
        code.insertInstruction(0, ins(IFFALSE, 1), body);
        code.insertInstruction(0, ins(CALLPROPERTY, abc.constants.getPublicQnameId(name, true), 2), body);
        code.insertInstruction(0, ins(GETLOCAL1), body);
        code.insertInstruction(0, ins(GETLOCAL0), body);
        code.insertInstruction(0, ins(GETLEX, abc.constants.getPublicQnameId(HELPER, true)), body);
        body.max_stack = Math.max(body.max_stack, 3);
        body.setCode(code);
        body.getCode().checkValidOffsets(body);
    }

    private static MethodBody method(ABC abc, String cls, String name) throws IOException {
        String simple = cls.substring(cls.lastIndexOf('.') + 1);
        int named = 0;
        for (var info : abc.instance_info)
            if (info.getName(abc.constants).getName(new HashSet<>(), abc, abc.constants, new ArrayList<>(), false, false)
                    .equals(simple))
                named++;
        if (named != 1) throw new IOException(named + " classes named " + simple + " in the block");
        MethodBody body = abc.findBodyByClassAndName(simple, name);
        if (body == null) throw new IOException("Missing " + cls + "." + name);
        return body;
    }

    private static int abcWith(SWF swf, String cls) throws IOException {
        int found = -1;
        for (int i = 0; i < swf.getAbcList().size(); i++)
            if (swf.getAbcList().get(i).getABC().findClassByName(cls) >= 0) {
                if (found >= 0) throw new IOException("Multiple " + cls + " classes");
                found = i;
            }
        if (found < 0) throw new IOException("Missing " + cls);
        return found;
    }

    private static List<byte[]> codeOf(ABC abc) {
        List<byte[]> code = new ArrayList<>();
        for (MethodBody body : abc.bodies) code.add(body.getCodeBytes().clone());
        return code;
    }

    public static void main(String[] args) throws Exception {
        SWF swf;
        try (InputStream in = Files.newInputStream(Path.of(args[0]))) { swf = new SWF(in, true); }
        for (int i = 0; i < swf.getAbcList().size(); i++)
            if (swf.getAbcList().get(i).getABC().findClassByName(HELPER) != -1)
                throw new IOException("Already patched");
        String lobby = "LobbyMenu";
        String ranking = "survarium.controls.PlayerRanking";
        int home = abcWith(swf, lobby);
        Map<Integer, ABC> blocks = new LinkedHashMap<>();
        Map<Integer, List<byte[]>> originals = new LinkedHashMap<>();
        Map<Integer, Set<MethodBody>> changed = new LinkedHashMap<>();
        for (String cls : List.of(lobby, ranking)) {
            int index = abcWith(swf, cls);
            if (!blocks.containsKey(index)) {
                ABC abc = swf.getAbcList().get(index).getABC();
                blocks.put(index, abc);
                originals.put(index, codeOf(abc));
                changed.put(index, new HashSet<>());
            }
        }
        ABC abc = blocks.get(home);
        swf.getAbcIndex().selectAbc(abc);
        ActionScript3Parser.compile(Files.readString(Path.of(args[1])), abc,
            swf.getAbcIndex(), "SrpPlayerGear.as", abc.instance_info.size(),
            abc.script_info.size(), false, "");
        List<byte[]> before = originals.get(home);
        for (int i = 0; i < before.size(); i++)
            if (!Arrays.equals(before.get(i), abc.bodies.get(i).getCodeBytes()))
                throw new IOException("Compiler changed existing method " + i);

        MethodBody fill = method(abc, lobby, "fill_victory_screen_player");
        changed.get(home).add(fill);
        guard(abc, fill, "take");

        int rankingIndex = abcWith(swf, ranking);
        ABC rankingAbc = blocks.get(rankingIndex);
        MethodBody activate = method(rankingAbc, ranking, "onActivate");
        if (!changed.get(rankingIndex).add(activate)) throw new IOException("Two hooks share a body");
        callAtEnd(rankingAbc, activate, "ranking");

        for (ABC block : blocks.values()) ((Tag) block.parentTag).setModified(true);
        try (OutputStream out = Files.newOutputStream(Path.of(args[2]))) { swf.saveTo(out); }

        try (InputStream in = Files.newInputStream(Path.of(args[2]))) {
            SWF saved = new SWF(in, true);
            if (saved.getAbcList().get(home).getABC().findClassByName(HELPER) < 0)
                throw new IOException("Missing " + HELPER);
            for (int index : blocks.keySet()) {
                ABC edited = blocks.get(index);
                ABC result = saved.getAbcList().get(index).getABC();
                if (edited.bodies.size() != result.bodies.size()) throw new IOException("Method count changed");
                List<byte[]> was = originals.get(index);
                for (int i = 0; i < edited.bodies.size(); i++) {
                    if (!Arrays.equals(edited.bodies.get(i).getCodeBytes(), result.bodies.get(i).getCodeBytes()))
                        throw new IOException("Serialization changed method " + i);
                    if (i < was.size() && !changed.get(index).contains(edited.bodies.get(i))
                            && !Arrays.equals(was.get(i), result.bodies.get(i).getCodeBytes()))
                        throw new IOException("Unrelated method changed " + i);
                }
            }
        }
        System.out.println(HELPER + " compiled into " + blocks.size() + " ABC block(s); all unrelated methods preserved");
    }
}

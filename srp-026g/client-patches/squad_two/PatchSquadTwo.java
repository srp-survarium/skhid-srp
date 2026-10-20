// Squads of two (the owner's of 2026-10-20: "there shouldn't even be a button
// to accept a third person"). survarium.friends.SquadList only:
//   set_squad_player  `_squadList.length = 3` becomes `= 2`: the squad panel
//       has two places (it had three); with both taken there is no empty row,
//       so no click opens its player search.
//   updateSize        `PlayerInfo.isSquadFull = count == 3` becomes
//       `count >= 2`. The friends list's and the chat's right-click menus
//       (FriendListRender.showContextMenu, LobbyChat) list "Invite to squad"
//       only while it is false, so a squad of two lists it nowhere.
//   addFriend         the panel's player search (opened on the empty row
//       before the second member came) invites only while the squad is not
//       full: `if (itemData)` becomes `if (itemData & !PlayerInfo.isSquadFull)`;
//       the search is closed either way, as before.
// Two instructions keep their length; five are added (no jump crosses
// them). Every other method stays byte for byte.
import java.io.*;
import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.abc.ABC;
import com.jpexs.decompiler.flash.abc.avm2.AVM2Code;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.abc.types.MethodBody;
import com.jpexs.decompiler.flash.abc.types.Multiname;
import com.jpexs.decompiler.flash.tags.Tag;

public class PatchSquadTwo {
    private static final int PUSHBYTE = 0x24, EQUALS = 0xab, GREATEREQUALS = 0xb0, SETPROPERTY = 0x61,
        GETPROPERTY = 0x66, FINDPROPSTRICT = 0x5d, NOT = 0x96, BITAND = 0xa8, CONVERT_B = 0x76,
        GETLOCAL2 = 0xd2, GETLOCAL3 = 0xd3, IFFALSE = 0x12;
    private static final String CLASS = "SquadList";

    private static String name(ABC abc, int multiname) {
        Multiname m = abc.constants.getMultiname(multiname);
        return m.name_index > 0 ? abc.constants.getString(m.name_index) : null;
    }

    private static int code(AVM2Instruction ins) {
        return ins.definition.instructionCode;
    }

    private static boolean is(ABC abc, AVM2Instruction ins, int opcode, String property) {
        return code(ins) == opcode && property.equals(name(abc, ins.operands[0]));
    }

    private static int blockOf(SWF swf, String cls) throws IOException {
        int found = -1;
        for (int i = 0; i < swf.getAbcList().size(); i++)
            if (swf.getAbcList().get(i).getABC().findClassByName(cls) >= 0) {
                if (found >= 0) throw new IOException("Two " + cls + " classes");
                found = i;
            }
        if (found < 0) throw new IOException("Missing " + cls);
        return found;
    }

    // FFDec finds a body by the class's own name: that name is the block's once.
    private static MethodBody body(ABC abc, String method) throws IOException {
        int named = 0;
        for (var info : abc.instance_info)
            if (info.getName(abc.constants).getName(new HashSet<>(), abc, abc.constants, new ArrayList<>(), false, false).equals(CLASS))
                named++;
        if (named != 1) throw new IOException(named + " classes named " + CLASS + " in the block");
        MethodBody body = abc.findBodyByClassAndName(CLASS, method);
        if (body == null) throw new IOException("Missing " + CLASS + "." + method);
        return body;
    }

    // The one `pushbyte 3` that sets `property` (`next` SETPROPERTY: at
    // once; EQUALS: `== 3` first) in the body: its index.
    private static int three(ABC abc, AVM2Code code, int next, String property, String method) throws IOException {
        int at = -1;
        for (int i = 0; i + 1 < code.code.size(); i++) {
            AVM2Instruction ins = code.code.get(i);
            if (code(ins) != PUSHBYTE || ins.operands[0] != 3) continue;
            boolean match = next == EQUALS
                ? code(code.code.get(i + 1)) == EQUALS && i + 2 < code.code.size()
                    && is(abc, code.code.get(i + 2), SETPROPERTY, property)
                : is(abc, code.code.get(i + 1), SETPROPERTY, property);
            if (!match) continue;
            if (at >= 0) throw new IOException(method + " sets " + property + " from 3 more than once");
            at = i;
        }
        if (at < 0) throw new IOException(method + " does not set " + property + " from 3 (already patched?)");
        return at;
    }

    public static void main(String[] args) throws Exception {
        SWF swf;
        try (InputStream in = Files.newInputStream(Path.of(args[0]))) { swf = new SWF(in, true); }
        int blocks = swf.getAbcList().size();
        List<List<byte[]>> originals = new ArrayList<>();
        for (int i = 0; i < blocks; i++) {
            List<byte[]> code = new ArrayList<>();
            for (MethodBody b : swf.getAbcList().get(i).getABC().bodies) code.add(b.getCodeBytes().clone());
            originals.add(code);
        }
        int block = blockOf(swf, "survarium.friends.SquadList");
        ABC abc = swf.getAbcList().get(block).getABC();
        MethodBody setPlayer = body(abc, "set_squad_player");
        MethodBody updateSize = body(abc, "updateSize");
        MethodBody addFriend = body(abc, "addFriend");
        int setLength = setPlayer.getCodeBytes().length, sizeLength = updateSize.getCodeBytes().length;

        // set_squad_player: `_squadList.length = 3` -> 2.
        AVM2Code code = setPlayer.getCode();
        code.code.get(three(abc, code, SETPROPERTY, "length", "set_squad_player")).operands[0] = 2;
        setPlayer.setCode(code);

        // updateSize: `isSquadFull = count == 3` -> `count >= 2`; it also
        // gives the multinames of PlayerInfo and of isSquadFull.
        code = updateSize.getCode();
        int full = three(abc, code, EQUALS, "isSquadFull", "updateSize");
        if (code(code.code.get(full - 1)) != GETLOCAL3) throw new IOException("updateSize: no count before == 3");
        AVM2Instruction findInfo = code.code.get(full - 3), getInfo = code.code.get(full - 2);
        if (!is(abc, findInfo, FINDPROPSTRICT, "PlayerInfo") || !is(abc, getInfo, GETPROPERTY, "PlayerInfo"))
            throw new IOException("updateSize: isSquadFull is not PlayerInfo's");
        int info = findInfo.operands[0], infoGet = getInfo.operands[0];
        int isFull = code.code.get(full + 2).operands[0];
        code.code.get(full).operands[0] = 2;
        AVM2Instruction equals = code.code.get(full + 1);
        code.code.set(full + 1, new AVM2Instruction(equals.getAddress(), GREATEREQUALS, new int[]{}));
        updateSize.setCode(code);

        // addFriend: `if (itemData)` -> `if (itemData & !PlayerInfo.isSquadFull)`.
        code = addFriend.getCode();
        int test = -1;
        for (int i = 0; i + 2 < code.code.size(); i++)
            if (code(code.code.get(i)) == GETLOCAL2 && code(code.code.get(i + 1)) == CONVERT_B
                && code(code.code.get(i + 2)) == IFFALSE) {
                if (test >= 0) throw new IOException("addFriend tests itemData more than once");
                test = i + 2;
            }
        if (test < 0) throw new IOException("addFriend does not test itemData");
        boolean invites = false;
        for (AVM2Instruction ins : code.code)
            if (ins.definition.instructionCode == 0x2c && "invite_to_squad".equals(abc.constants.getString(ins.operands[0])))
                invites = true;
        if (!invites) throw new IOException("addFriend does not invite to a squad");
        for (AVM2Instruction ins : code.code)
            if (is(abc, ins, GETPROPERTY, "isSquadFull")) throw new IOException("addFriend already reads isSquadFull");
        int[][] added = {{FINDPROPSTRICT, info}, {GETPROPERTY, infoGet}, {GETPROPERTY, isFull}, {NOT}, {BITAND}};
        for (int k = 0; k < added.length; k++)
            code.insertInstruction(test + k, new AVM2Instruction(0, added[k][0],
                Arrays.copyOfRange(added[k], 1, added[k].length)), addFriend);
        addFriend.max_stack = Math.max(addFriend.max_stack, 3);
        addFriend.setCode(code);
        addFriend.getCode().checkValidOffsets(addFriend);

        if (setPlayer.getCodeBytes().length != setLength) throw new IOException("set_squad_player changed length");
        if (updateSize.getCodeBytes().length != sizeLength) throw new IOException("updateSize changed length");
        ((Tag) abc.parentTag).setModified(true);
        try (OutputStream out = Files.newOutputStream(Path.of(args[1]))) { swf.saveTo(out); }

        SWF saved;
        try (InputStream in = Files.newInputStream(Path.of(args[1]))) { saved = new SWF(in, true); }
        List<MethodBody> mine = List.of(setPlayer, updateSize, addFriend);
        int changed = 0;
        for (int b = 0; b < blocks; b++) {
            List<MethodBody> bodies = saved.getAbcList().get(b).getABC().bodies;
            if (bodies.size() != originals.get(b).size()) throw new IOException("Method count changed in block " + b);
            for (int i = 0; i < bodies.size(); i++)
                if (!Arrays.equals(originals.get(b).get(i), bodies.get(i).getCodeBytes())) {
                    boolean expected = false;
                    for (MethodBody m : mine) expected |= b == block && i == abc.bodies.indexOf(m);
                    if (!expected) throw new IOException("Unrelated method changed: block " + b + " method " + i);
                    if (!Arrays.equals(abc.bodies.get(i).getCodeBytes(), bodies.get(i).getCodeBytes()))
                        throw new IOException("Serialization changed method " + i);
                    changed++;
                }
        }
        if (changed != 3) throw new IOException("Expected three methods changed, got " + changed);
        System.out.println("Squads of two: two places in the squad panel, no invite once full; three methods changed");
    }
}

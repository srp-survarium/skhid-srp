// Compile SrpFriendRoom into the friend list row's ABC block, and:
//   FriendListRender.showContextMenu  the menu of a player not ignored,
//       `SrvContextMenu.show(items)` (findpropstrict, getproperty
//       SrvContextMenu, getlocal2, callproperty show 1, pop), is followed by
//       `SrpFriendRoom.opened(items, this)`: four instructions after its pop,
//       no jump added (the next instruction, SrvContextMenu.getInstance(), is
//       no jump's target: the two calls are one block). The ignored player's
//       menu (Remove from ignore, its array made in place) is left alone.
// Every other method of every block stays byte for byte.
import java.io.*;
import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.SWF;
import com.jpexs.decompiler.flash.abc.ABC;
import com.jpexs.decompiler.flash.abc.avm2.AVM2Code;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.abc.avm2.parser.script.ActionScript3Parser;
import com.jpexs.decompiler.flash.abc.types.MethodBody;
import com.jpexs.decompiler.flash.abc.types.Multiname;
import com.jpexs.decompiler.flash.tags.Tag;

public class PatchFriendRoom {
    private static final int GETLEX = 0x60, FINDPROPSTRICT = 0x5d, GETPROPERTY = 0x66, GETLOCAL = 0x62,
        GETLOCAL0 = 0xd0, GETLOCAL1 = 0xd1, GETLOCAL3 = 0xd3, CALLPROPERTY = 0x46, CALLPROPVOID = 0x4f,
        POP = 0x29;
    private static final String CLASS = "survarium.friends.FriendListRender";

    private static String name(ABC abc, int multiname) {
        Multiname m = abc.constants.getMultiname(multiname);
        return m.name_index > 0 ? abc.constants.getString(m.name_index) : null;
    }

    private static boolean isGetLocal(AVM2Instruction ins) {
        int op = ins.definition.instructionCode;
        return op == GETLOCAL || (op >= GETLOCAL1 && op <= GETLOCAL3);
    }

    public static void main(String[] args) throws Exception {
        SWF swf;
        try (InputStream in = Files.newInputStream(Path.of(args[0]))) { swf = new SWF(in, true); }
        int blocks = swf.getAbcList().size(), block = -1;
        boolean rooms = false;
        for (int i = 0; i < blocks; i++) {
            ABC abc = swf.getAbcList().get(i).getABC();
            if (abc.findClassByName("SrpFriendRoom") >= 0) throw new IOException("Already patched");
            if (abc.findClassByName("SrpRooms") >= 0) rooms = true;
            if (abc.findClassByName(CLASS) >= 0) {
                if (block >= 0) throw new IOException("Two " + CLASS + " classes");
                block = i;
            }
        }
        if (block < 0) throw new IOException("Missing " + CLASS);
        if (!rooms) throw new IOException("Missing SrpRooms: the rooms step comes first");
        List<List<byte[]>> originals = new ArrayList<>();
        for (int i = 0; i < blocks; i++) {
            List<byte[]> code = new ArrayList<>();
            for (MethodBody b : swf.getAbcList().get(i).getABC().bodies) code.add(b.getCodeBytes().clone());
            originals.add(code);
        }
        ABC abc = swf.getAbcList().get(block).getABC();
        // FFDec finds bodies by a class's own name: it must be the block's once.
        int named = 0;
        for (var info : abc.instance_info)
            if (info.getName(abc.constants).getName(new HashSet<>(), abc, abc.constants, new ArrayList<>(), false, false)
                    .equals("FriendListRender"))
                named++;
        if (named != 1) throw new IOException(named + " classes named FriendListRender in the block");
        MethodBody menu = abc.findBodyByClassAndName("FriendListRender", "showContextMenu");
        if (menu == null) throw new IOException("Missing FriendListRender.showContextMenu");
        if (menu.exceptions.length != 0) throw new IOException("Unexpected exception handlers");

        // `SrvContextMenu.show(<local>)`: the friend's menu, once. The
        // ignored player's passes an array made in place.
        AVM2Code code = menu.getCode();
        int shown = -1;
        for (int i = 3; i + 1 < code.code.size(); i++) {
            AVM2Instruction ins = code.code.get(i);
            if (ins.definition.instructionCode != CALLPROPERTY || ins.operands[1] != 1
                    || !"show".equals(name(abc, ins.operands[0]))
                    || code.code.get(i + 1).definition.instructionCode != POP)
                continue;
            AVM2Instruction find = code.code.get(i - 3), get = code.code.get(i - 2);
            if (!isGetLocal(code.code.get(i - 1))
                    || find.definition.instructionCode != FINDPROPSTRICT
                    || !"SrvContextMenu".equals(name(abc, find.operands[0]))
                    || get.definition.instructionCode != GETPROPERTY
                    || !"SrvContextMenu".equals(name(abc, get.operands[0])))
                continue;
            if (shown >= 0) throw new IOException("showContextMenu shows a menu of a local more than once");
            shown = i;
        }
        if (shown < 0) throw new IOException("showContextMenu does not show the friend's menu");
        AVM2Instruction items = code.code.get(shown - 1);
        int[] local = items.operands == null ? new int[0] : items.operands.clone();

        swf.getAbcIndex().selectAbc(abc);
        ActionScript3Parser.compile(Files.readString(Path.of(args[1])), abc, swf.getAbcIndex(), "SrpFriendRoom.as",
            abc.instance_info.size(), abc.script_info.size(), false, "");
        for (int i = 0; i < originals.get(block).size(); i++)
            if (!Arrays.equals(originals.get(block).get(i), abc.bodies.get(i).getCodeBytes()))
                throw new IOException("Compiler changed existing method " + i);

        code = menu.getCode();
        int helper = abc.instance_info.get(abc.findClassByName("SrpFriendRoom")).name_index;
        int opened = abc.constants.getPublicQnameId("opened", true);
        AVM2Instruction[] added = {
            new AVM2Instruction(0, GETLEX, new int[]{helper}),
            new AVM2Instruction(0, items.definition.instructionCode, local),
            new AVM2Instruction(0, GETLOCAL0, new int[]{}),
            new AVM2Instruction(0, CALLPROPVOID, new int[]{opened, 2}),
        };
        // After the pop of show's result.
        for (int k = 0; k < added.length; k++)
            code.insertInstruction(shown + 2 + k, added[k], menu);
        menu.max_stack = Math.max(menu.max_stack, 3);
        menu.setCode(code);
        menu.getCode().checkValidOffsets(menu);
        ((Tag) abc.parentTag).setModified(true);
        try (OutputStream out = Files.newOutputStream(Path.of(args[2]))) { swf.saveTo(out); }

        SWF saved;
        try (InputStream in = Files.newInputStream(Path.of(args[2]))) { saved = new SWF(in, true); }
        if (saved.getAbcList().get(block).getABC().findClassByName("SrpFriendRoom") < 0)
            throw new IOException("Missing SrpFriendRoom");
        List<MethodBody> edited = abc.bodies, result = saved.getAbcList().get(block).getABC().bodies;
        if (edited.size() != result.size()) throw new IOException("Method count changed");
        for (int i = 0; i < edited.size(); i++)
            if (!Arrays.equals(edited.get(i).getCodeBytes(), result.get(i).getCodeBytes()))
                throw new IOException("Serialization changed method " + i);
        int index = abc.bodies.indexOf(menu), changed = 0;
        for (int b = 0; b < blocks; b++) {
            List<MethodBody> bodies = saved.getAbcList().get(b).getABC().bodies;
            if (b != block && bodies.size() != originals.get(b).size())
                throw new IOException("Method count changed in block " + b);
            for (int i = 0; i < originals.get(b).size(); i++)
                if (!Arrays.equals(originals.get(b).get(i), bodies.get(i).getCodeBytes())) {
                    if (b != block || i != index)
                        throw new IOException("Unrelated method changed: block " + b + " method " + i);
                    changed++;
                }
        }
        if (changed != 1) throw new IOException("Expected showContextMenu changed, got " + changed);
        System.out.println("A friend's menu offers Join room; all unrelated methods preserved");
    }
}

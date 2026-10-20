// Compile SrpCraftMenu into the inventory list's ABC block, and:
//   InventoryList.showContextMenu  `this._inventoryLocked` becomes
//       `SrpCraftMenu.locked(this._inventoryLocked, this, activeItem)`: five
//       instructions after its getproperty, no jump added;
//   MessageCraftItem.fillCraftedItem  ends with
//       `SrpCraftMenu.result(this, result)`.
// Every other method stays byte for byte.
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

public class PatchCraftMenu {
    private static final int GETPROPERTY = 0x66, IFFALSE = 0x12, GETLEX = 0x60, SWAP = 0x2b,
        GETLOCAL0 = 0xd0, GETLOCAL1 = 0xd1, GETLOCAL2 = 0xd2, CALLPROPERTY = 0x46, CALLPROPVOID = 0x4f,
        RETURNVOID = 0x47;
    private static final String CLASS = "survarium.controls.InventoryList";

    private static String name(ABC abc, int multiname) {
        Multiname m = abc.constants.getMultiname(multiname);
        return m.name_index > 0 ? abc.constants.getString(m.name_index) : null;
    }

    public static void main(String[] args) throws Exception {
        SWF swf;
        try (InputStream in = Files.newInputStream(Path.of(args[0]))) { swf = new SWF(in, true); }
        int blocks = swf.getAbcList().size(), block = -1;
        for (int i = 0; i < blocks; i++) {
            ABC abc = swf.getAbcList().get(i).getABC();
            if (abc.findClassByName("SrpCraftMenu") >= 0) throw new IOException("Already patched");
            if (abc.findClassByName(CLASS) >= 0) {
                if (block >= 0) throw new IOException("Two " + CLASS + " classes");
                block = i;
            }
        }
        if (block < 0) throw new IOException("Missing " + CLASS);
        List<List<byte[]>> originals = new ArrayList<>();
        for (int i = 0; i < blocks; i++) {
            List<byte[]> code = new ArrayList<>();
            for (MethodBody b : swf.getAbcList().get(i).getABC().bodies) code.add(b.getCodeBytes().clone());
            originals.add(code);
        }
        ABC abc = swf.getAbcList().get(block).getABC();
        int named = 0;
        for (var info : abc.instance_info)
            if (info.getName(abc.constants).getName(new HashSet<>(), abc, abc.constants, new ArrayList<>(), false, false)
                    .equals("InventoryList"))
                named++;
        if (named != 1) throw new IOException(named + " classes named InventoryList in the block");
        MethodBody menu = abc.findBodyByClassAndName("InventoryList", "showContextMenu");
        if (menu == null) throw new IOException("Missing InventoryList.showContextMenu");

        // The lock's read: once, and tested at once.
        AVM2Code code = menu.getCode();
        int read = -1;
        for (int i = 0; i < code.code.size(); i++) {
            AVM2Instruction ins = code.code.get(i);
            if (ins.definition.instructionCode == GETPROPERTY && "_inventoryLocked".equals(name(abc, ins.operands[0]))) {
                if (read >= 0) throw new IOException("showContextMenu reads _inventoryLocked more than once");
                read = i;
            }
        }
        if (read < 0) throw new IOException("showContextMenu does not read _inventoryLocked");
        if (code.code.get(read + 1).definition.instructionCode != IFFALSE)
            throw new IOException("showContextMenu does not test _inventoryLocked at once");

        swf.getAbcIndex().selectAbc(abc);
        ActionScript3Parser.compile(Files.readString(Path.of(args[1])), abc, swf.getAbcIndex(), "SrpCraftMenu.as",
            abc.instance_info.size(), abc.script_info.size(), false, "");
        for (int i = 0; i < originals.get(block).size(); i++)
            if (!Arrays.equals(originals.get(block).get(i), abc.bodies.get(i).getCodeBytes()))
                throw new IOException("Compiler changed existing method " + i);

        code = menu.getCode();
        int helper = abc.constants.getPublicQnameId("SrpCraftMenu", true);
        int locked = abc.constants.getPublicQnameId("locked", true);
        int[][] added = {{GETLEX, helper}, {SWAP}, {GETLOCAL0}, {GETLOCAL1}, {CALLPROPERTY, locked, 3}};
        for (int k = 0; k < added.length; k++)
            code.insertInstruction(read + 1 + k, new AVM2Instruction(0, added[k][0],
                Arrays.copyOfRange(added[k], 1, added[k].length)), menu);
        menu.max_stack += 3;
        menu.setCode(code);
        menu.getCode().checkValidOffsets(menu);
        ((Tag) abc.parentTag).setModified(true);

        // MessageCraftItem.fillCraftedItem ends with SrpCraftMenu.result(this, result).
        String window = "survarium.messages.MessageCraftItem";
        int windowBlock = -1;
        for (int i = 0; i < blocks; i++)
            if (swf.getAbcList().get(i).getABC().findClassByName(window) >= 0) {
                if (windowBlock >= 0) throw new IOException("Two " + window + " classes");
                windowBlock = i;
            }
        if (windowBlock < 0) throw new IOException("Missing " + window);
        ABC windowAbc = swf.getAbcList().get(windowBlock).getABC();
        MethodBody filled = windowAbc.findBodyByClassAndName("MessageCraftItem", "fillCraftedItem");
        if (filled == null) throw new IOException("Missing MessageCraftItem.fillCraftedItem");
        AVM2Code fill = filled.getCode();
        int end = fill.code.size() - 1;
        if (fill.code.get(end).definition.instructionCode != RETURNVOID || filled.exceptions.length != 0)
            throw new IOException("Unexpected fillCraftedItem layout");
        int[][] call = {{GETLEX, windowAbc.constants.getPublicQnameId("SrpCraftMenu", true)}, {GETLOCAL0}, {GETLOCAL2},
            {CALLPROPVOID, windowAbc.constants.getPublicQnameId("result", true), 2}};
        for (int[] ins : call)
            fill.insertInstruction(end++, new AVM2Instruction(0, ins[0], Arrays.copyOfRange(ins, 1, ins.length)), filled);
        filled.max_stack = Math.max(filled.max_stack, 3);
        filled.setCode(fill);
        filled.getCode().checkValidOffsets(filled);
        ((Tag) windowAbc.parentTag).setModified(true);
        try (OutputStream out = Files.newOutputStream(Path.of(args[2]))) { swf.saveTo(out); }

        SWF saved;
        try (InputStream in = Files.newInputStream(Path.of(args[2]))) { saved = new SWF(in, true); }
        if (saved.getAbcList().get(block).getABC().findClassByName("SrpCraftMenu") < 0)
            throw new IOException("Missing SrpCraftMenu");
        int index = abc.bodies.indexOf(menu), fillIndex = windowAbc.bodies.indexOf(filled), changed = 0;
        for (int b = 0; b < blocks; b++) {
            List<MethodBody> bodies = saved.getAbcList().get(b).getABC().bodies;
            if (b != block && bodies.size() != originals.get(b).size())
                throw new IOException("Method count changed in block " + b);
            for (int i = 0; i < originals.get(b).size(); i++)
                if (!Arrays.equals(originals.get(b).get(i), bodies.get(i).getCodeBytes())) {
                    boolean expected = (b == block && i == index) || (b == windowBlock && i == fillIndex);
                    if (!expected) throw new IOException("Unrelated method changed: block " + b + " method " + i);
                    changed++;
                }
        }
        if (changed != 2) throw new IOException("Expected showContextMenu and fillCraftedItem changed, got " + changed);
        System.out.println("Craft window: right-click gives Buy and Sell; a result offers Continue and Exit");
    }
}

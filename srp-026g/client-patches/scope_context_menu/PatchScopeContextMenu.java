// Only InventoryList's eligibility predicate and the upgrade menu label change.
// Its callback, ownership checks, inventory locks and all scope editor code stay intact.
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

public class PatchScopeContextMenu {
    private static final int GETLEX = 0x60, GETLOCAL0 = 0xd0, GETLOCAL1 = 0xd1,
        PUSHSCOPE = 0x30, PUSHSTRING = 0x2c, GETPROPERTY = 0x66,
        CALLPROPERTY = 0x46, RETURNVALUE = 0x48;

    private static AVM2Instruction ins(int opcode, int... operands) {
        return new AVM2Instruction(0, opcode, operands);
    }

    public static void main(String[] args) throws Exception {
        SWF swf;
        try (InputStream in = Files.newInputStream(Path.of(args[0]))) { swf = new SWF(in, true); }
        ABC abc = null;
        int abcIndex = -1;
        boolean scopes = false;
        for (int i = 0; i < swf.getAbcList().size(); i++) {
            ABC candidate = swf.getAbcList().get(i).getABC();
            if (candidate.findClassByName("SrpScopeContextMenu") >= 0)
                throw new IOException("Already patched");
            if (candidate.findClassByName("survarium.upgrade.UpgradeWindow") >= 0)
                scopes = candidate.findBodyByClassAndName("UpgradeWindow", "hasScopes") != null;
            if (candidate.findClassByName("survarium.controls.InventoryList") >= 0) {
                if (abc != null) throw new IOException("Multiple InventoryList classes");
                abc = candidate;
                abcIndex = i;
            }
        }
        if (abc == null || !scopes) throw new IOException("Missing scope-enabled lobby classes");
        // FFDec finds bodies by a class's own name: it must be the block's once.
        for (String name : List.of("InventoryList", "UpgradeWindow")) {
            int named = 0;
            for (var info : abc.instance_info)
                if (info.getName(abc.constants).getName(new HashSet<>(), abc, abc.constants, new ArrayList<>(), false, false)
                        .equals(name))
                    named++;
            if (named > 1) throw new IOException(named + " classes named " + name + " in the block");
        }
        List<byte[]> originals = new ArrayList<>();
        for (MethodBody body : abc.bodies) originals.add(body.getCodeBytes().clone());
        MethodBody eligible = abc.findBodyByClassAndName("InventoryList", "isUpgradeable");
        MethodBody menu = abc.findBodyByClassAndName("InventoryList", "showContextMenu");
        if (eligible == null || menu == null || eligible == menu)
            throw new IOException("Missing context menu methods");
        if (eligible.exceptions.length != 0 || menu.exceptions.length != 0)
            throw new IOException("Unexpected exception handlers");

        swf.getAbcIndex().selectAbc(abc);
        ActionScript3Parser.compile(Files.readString(Path.of(args[1])), abc,
            swf.getAbcIndex(), "SrpScopeContextMenu.as", abc.instance_info.size(),
            abc.script_info.size(), false, "");
        for (int i = 0; i < originals.size(); i++)
            if (!Arrays.equals(originals.get(i), abc.bodies.get(i).getCodeBytes()))
                throw new IOException("Compiler changed existing method " + i);
        int helper = abc.instance_info.get(abc.findClassByName("SrpScopeContextMenu")).name_index;

        AVM2Code predicate = new AVM2Code();
        predicate.code.addAll(List.of(ins(GETLOCAL0), ins(PUSHSCOPE), ins(GETLEX, helper),
            ins(GETLOCAL1), ins(CALLPROPERTY, abc.constants.getPublicQnameId("isUpgradeable", true), 1),
            ins(RETURNVALUE)));
        eligible.setCode(predicate);
        eligible.max_stack = 2;
        eligible.max_regs = 2;
        eligible.max_scope_depth = eligible.init_scope_depth + 1;
        eligible.getCode().checkValidOffsets(eligible);

        // Replace only the upgrade label's string expression. local1 is the
        // clicked DragSlot argument, so the label describes the clicked item.
        AVM2Code code = menu.getCode();
        int label = -1;
        for (int i = 0; i < code.code.size(); i++) {
            AVM2Instruction op = code.code.get(i);
            if (op.definition.instructionCode == PUSHSTRING
                    && abc.constants.getString(op.operands[0]).equals("btn_upgrade_label")) {
                if (label >= 0) throw new IOException("Multiple upgrade labels");
                label = i;
            }
        }
        if (label < 0) throw new IOException("Missing upgrade label");
        code.replaceInstruction(label++, ins(GETLEX, helper), menu);
        code.insertInstruction(label++, ins(GETLOCAL1), menu);
        code.insertInstruction(label++, ins(GETPROPERTY, abc.constants.getPublicQnameId("data", true)), menu);
        code.insertInstruction(label, ins(CALLPROPERTY, abc.constants.getPublicQnameId("labelKey", true), 1), menu);
        menu.max_stack += 1;
        menu.setCode(code);
        menu.getCode().checkValidOffsets(menu);
        ((Tag) abc.parentTag).setModified(true);
        try (OutputStream out = Files.newOutputStream(Path.of(args[2]))) { swf.saveTo(out); }

        try (InputStream in = Files.newInputStream(Path.of(args[2]))) {
            SWF saved = new SWF(in, true);
            ABC result = saved.getAbcList().get(abcIndex).getABC();
            if (abc.bodies.size() != result.bodies.size()) throw new IOException("Method count changed");
            for (int i = 0; i < abc.bodies.size(); i++) {
                if (!Arrays.equals(abc.bodies.get(i).getCodeBytes(), result.bodies.get(i).getCodeBytes()))
                    throw new IOException("Serialization changed method " + i);
                if (i < originals.size() && abc.bodies.get(i) != eligible && abc.bodies.get(i) != menu
                        && !Arrays.equals(originals.get(i), result.bodies.get(i).getCodeBytes()))
                    throw new IOException("Unrelated method changed " + i);
            }
        }
        System.out.println("Scope context menu compiled; all unrelated methods preserved");
    }
}

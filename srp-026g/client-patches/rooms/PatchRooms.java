// Compile SrpRooms into the bottom panel's ABC block and add calls to it;
// nothing else changes. The hooks:
//   BottomPanel.createMatchesTab  end    SrpRooms.attach(this)
//   BottomPanel.placeMatchesTab   end    SrpRooms.place(this)
//   BottomPanel.resizeTab         end    SrpRooms.resize(this)
//   PlayerRanking.fillPlayers     start  if (SrpRooms.take(rows)) return;
//   LobbyMenu.set_play_button_enabled end SrpRooms.playState(this)
//   LobbyMenu.set_equipment_level end    SrpRooms.gear(value)
//   StatusPanel.play_button_clicked end  SrpRooms.played(this)
//   LobbyMenu.has_active_window   start  if (SrpRooms.escapable()) return true;
//   LobbyMenu.close_active_window start  if (SrpRooms.closeWindow()) return;
//   TopMenu.fill_windows          end    SrpRooms.topMenu(this)
//   TopMenu.changeActiveMenu      end    SrpRooms.topChanged(this)
//   StatusPanel.play_button_clicked start if (SrpRooms.playPressed()) return;
//   LobbyMenu.show_victory_screen  start  SrpRooms.afterMatch()
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

public class PatchRooms {
    private static final int GETLEX = 0x60, GETLOCAL0 = 0xd0, GETLOCAL1 = 0xd1, CALLPROPVOID = 0x4f,
        CALLPROPERTY = 0x46, IFFALSE = 0x12, RETURNVOID = 0x47, PUSHTRUE = 0x26, RETURNVALUE = 0x48;

    // Inserts `SrpRooms.<name>(<argument>)` before the final returnvoid.
    private static void call(ABC abc, MethodBody body, int helper, String name, int argument)
            throws Exception {
        AVM2Code code = body.getCode();
        int end = code.code.size() - 1;
        if (code.code.get(end).definition.instructionCode != RETURNVOID || body.exceptions.length != 0)
            throw new IOException("Unexpected method layout before " + name);
        code.insertInstruction(end++, new AVM2Instruction(0, GETLEX, new int[]{helper}), body);
        code.insertInstruction(end++, new AVM2Instruction(0, argument, new int[]{}), body);
        code.insertInstruction(end, new AVM2Instruction(0, CALLPROPVOID,
            new int[]{abc.constants.getPublicQnameId(name, true), 1}), body);
        body.max_stack = Math.max(body.max_stack, 2);
        body.setCode(code);
        body.getCode().checkValidOffsets(body);
    }

    // Puts `if (SrpRooms.<name>(arg1)) return;` before the method's first
    // instruction (the instructions go in from the last, each at 0).
    private static void guard(ABC abc, MethodBody body, int helper, String name) throws Exception {
        if (body.exceptions.length != 0)
            throw new IOException("Unexpected exceptions in the method of " + name);
        AVM2Code code = body.getCode();
        code.insertInstruction(0, new AVM2Instruction(0, RETURNVOID, new int[]{}), body);
        code.insertInstruction(0, new AVM2Instruction(0, IFFALSE, new int[]{1}), body);
        code.insertInstruction(0, new AVM2Instruction(0, CALLPROPERTY,
            new int[]{abc.constants.getPublicQnameId(name, true), 1}), body);
        code.insertInstruction(0, new AVM2Instruction(0, GETLOCAL1, new int[]{}), body);
        code.insertInstruction(0, new AVM2Instruction(0, GETLEX, new int[]{helper}), body);
        body.max_stack = Math.max(body.max_stack, 2);
        body.setCode(code);
        body.getCode().checkValidOffsets(body);
    }

    // Puts `if (SrpRooms.<name>()) return [true];` before the method's first
    // instruction: `returnsTrue` for a method that returns a Boolean.
    private static void guard0(ABC abc, MethodBody body, int helper, String name, boolean returnsTrue)
            throws Exception {
        if (body.exceptions.length != 0)
            throw new IOException("Unexpected exceptions in the method of " + name);
        AVM2Code code = body.getCode();
        if (returnsTrue) {
            code.insertInstruction(0, new AVM2Instruction(0, RETURNVALUE, new int[]{}), body);
            code.insertInstruction(0, new AVM2Instruction(0, PUSHTRUE, new int[]{}), body);
            code.insertInstruction(0, new AVM2Instruction(0, IFFALSE, new int[]{2}), body);
        } else {
            code.insertInstruction(0, new AVM2Instruction(0, RETURNVOID, new int[]{}), body);
            code.insertInstruction(0, new AVM2Instruction(0, IFFALSE, new int[]{1}), body);
        }
        code.insertInstruction(0, new AVM2Instruction(0, CALLPROPERTY,
            new int[]{abc.constants.getPublicQnameId(name, true), 0}), body);
        code.insertInstruction(0, new AVM2Instruction(0, GETLEX, new int[]{helper}), body);
        body.max_stack = Math.max(body.max_stack, 1);
        body.setCode(code);
        body.getCode().checkValidOffsets(body);
    }

    // Puts `SrpRooms.<name>();` before the method's first instruction.
    private static void prepend(ABC abc, MethodBody body, int helper, String name) throws Exception {
        if (body.exceptions.length != 0)
            throw new IOException("Unexpected exceptions in the method of " + name);
        AVM2Code code = body.getCode();
        code.insertInstruction(0, new AVM2Instruction(0, CALLPROPVOID,
            new int[]{abc.constants.getPublicQnameId(name, true), 0}), body);
        code.insertInstruction(0, new AVM2Instruction(0, GETLEX, new int[]{helper}), body);
        body.max_stack = Math.max(body.max_stack, 1);
        body.setCode(code);
        body.getCode().checkValidOffsets(body);
    }

    // FFDec finds bodies by the class's own name, without its package.
    private static MethodBody method(ABC abc, String cls, String name) throws IOException {
        String simple = cls.substring(cls.lastIndexOf('.') + 1);
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

    // Every method of `saved` but `changed` (and the new ones past
    // `originals`) is byte for byte `originals`.
    private static void verify(ABC edited, ABC saved, List<byte[]> originals, Set<MethodBody> changed)
            throws IOException {
        if (edited.bodies.size() != saved.bodies.size()) throw new IOException("Method count changed");
        for (int i = 0; i < edited.bodies.size(); i++)
            if (!Arrays.equals(edited.bodies.get(i).getCodeBytes(), saved.bodies.get(i).getCodeBytes()))
                throw new IOException("Serialization changed method " + i);
        for (int i = 0; i < originals.size(); i++)
            if (!changed.contains(edited.bodies.get(i))
                && !Arrays.equals(originals.get(i), saved.bodies.get(i).getCodeBytes()))
                throw new IOException("Unrelated method changed " + i);
    }

    public static void main(String[] args) throws Exception {
        SWF swf;
        try (InputStream in = Files.newInputStream(Path.of(args[0]))) { swf = new SWF(in, true); }
        for (int i = 0; i < swf.getAbcList().size(); i++)
            if (swf.getAbcList().get(i).getABC().findClassByName("SrpRooms") != -1)
                throw new IOException("Already patched");
        String panel = "survarium.controls.BottomPanel";
        String ranking = "survarium.controls.PlayerRanking";
        String lobby = "LobbyMenu";
        String status = "survarium.controls.StatusPanel";
        String top = "survarium.controls.TopMenu";
        int home = abcWith(swf, panel);
        Map<Integer, ABC> blocks = new LinkedHashMap<>();
        Map<Integer, List<byte[]>> originals = new LinkedHashMap<>();
        Map<Integer, Set<MethodBody>> changed = new LinkedHashMap<>();
        for (String cls : List.of(panel, ranking, lobby, status, top)) {
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
            swf.getAbcIndex(), "SrpRooms.as", abc.instance_info.size(),
            abc.script_info.size(), false, "");
        List<byte[]> before = originals.get(home);
        for (int i = 0; i < before.size(); i++)
            if (!Arrays.equals(before.get(i), abc.bodies.get(i).getCodeBytes()))
                throw new IOException("Compiler changed existing method " + i);

        // The hooks, each in its class's block, naming SrpRooms by its qname.
        Object[][] hooks = {
            {panel, "createMatchesTab", "attach", GETLOCAL0},
            {panel, "placeMatchesTab", "place", GETLOCAL0},
            {panel, "resizeTab", "resize", GETLOCAL0},
            {lobby, "set_play_button_enabled", "playState", GETLOCAL0},
            {lobby, "set_equipment_level", "gear", GETLOCAL1},
            {status, "play_button_clicked", "played", GETLOCAL0},
            {top, "fill_windows", "topMenu", GETLOCAL0},
            {top, "changeActiveMenu", "topChanged", GETLOCAL0},
        };
        for (Object[] hook : hooks) {
            int index = abcWith(swf, (String) hook[0]);
            ABC block = blocks.get(index);
            int helper = block.constants.getPublicQnameId("SrpRooms", true);
            MethodBody body = method(block, (String) hook[0], (String) hook[1]);
            if (!changed.get(index).add(body)) throw new IOException("Two hooks share a body");
            call(block, body, helper, (String) hook[2], (Integer) hook[3]);
        }
        int rankingIndex = abcWith(swf, ranking);
        ABC rankingAbc = blocks.get(rankingIndex);
        MethodBody fill = method(rankingAbc, ranking, "fillPlayers");
        changed.get(rankingIndex).add(fill);
        guard(rankingAbc, fill, rankingAbc.constants.getPublicQnameId("SrpRooms", true), "take");

        int lobbyIndex = abcWith(swf, lobby);
        ABC lobbyAbc = blocks.get(lobbyIndex);
        int lobbyHelper = lobbyAbc.constants.getPublicQnameId("SrpRooms", true);
        MethodBody active = method(lobbyAbc, lobby, "has_active_window");
        MethodBody close = method(lobbyAbc, lobby, "close_active_window");
        if (!changed.get(lobbyIndex).add(active) || !changed.get(lobbyIndex).add(close))
            throw new IOException("Two hooks share a body");
        guard0(lobbyAbc, active, lobbyHelper, "escapable", true);
        guard0(lobbyAbc, close, lobbyHelper, "closeWindow", false);
        // A match's results: the room's window is opened first, so the
        // results screen goes over it and closing them shows the room.
        MethodBody results = method(lobbyAbc, lobby, "show_victory_screen");
        if (!changed.get(lobbyIndex).add(results)) throw new IOException("Two hooks share a body");
        prepend(lobbyAbc, results, lobbyHelper, "afterMatch");

        // Play with rooms on: straight into a room, the game's own Play (its
        // search) not run. The same body has the `played` call at its end.
        int statusIndex = abcWith(swf, status);
        ABC statusAbc = blocks.get(statusIndex);
        MethodBody playClick = method(statusAbc, status, "play_button_clicked");
        changed.get(statusIndex).add(playClick);
        guard0(statusAbc, playClick, statusAbc.constants.getPublicQnameId("SrpRooms", true), "playPressed", false);
        // The game's search panel, not shown for a room's match.
        MethodBody search = method(statusAbc, status, "show_match_making");
        if (!changed.get(statusIndex).add(search)) throw new IOException("Two hooks share a body");
        guard(statusAbc, search, statusAbc.constants.getPublicQnameId("SrpRooms", true), "searchHidden");

        for (ABC block : blocks.values()) ((Tag) block.parentTag).setModified(true);
        try (OutputStream out = Files.newOutputStream(Path.of(args[2]))) { swf.saveTo(out); }

        try (InputStream in = Files.newInputStream(Path.of(args[2]))) {
            SWF saved = new SWF(in, true);
            if (saved.getAbcList().get(home).getABC().findClassByName("SrpRooms") < 0)
                throw new IOException("Missing SrpRooms");
            for (int index : blocks.keySet())
                verify(blocks.get(index), saved.getAbcList().get(index).getABC(),
                    originals.get(index), changed.get(index));
        }
        System.out.println("Rooms compiled into " + blocks.size() + " ABC block(s); all unrelated methods preserved");
    }
}

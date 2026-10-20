import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.abc.*;
import com.jpexs.decompiler.flash.abc.types.*;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.tags.Tag;

/** Generate the reference from native compilation; exercise unsafe interior entries. */
public class CompactTruthFixture {
    static ABC abc(Path input)throws Exception {
        return MovieSource.findScript(MovieSource.read(input),"CompactTruth").abc;
    }
    static MethodBody method(ABC abc)throws Exception {
        int id=MovieSource.names(abc).entrySet().stream()
            .filter(e->e.getValue().endsWith(",\"test\")")).findFirst().orElseThrow().getKey();
        var body=abc.findBody(id);MovieSource.exactCode(body);return body;
    }
    static int pair(MethodBody body,String kind) {
        var code=body.getCode().code;
        for(int i=0;i+1<code.size();i++) {
            int a=code.get(i).definition.instructionCode,b=code.get(i+1).definition.instructionCode;
            if(kind.equals("string") && a==0x2c && b==0x85
                    || kind.equals("super") && a==0x45 && b==0x29
                    || kind.equals("negation") && a==0x96 && (b==0x11 || b==0x12)
                    || kind.equals("truth") && a==0x76 && (b==0x11 || b==0x12))return i;
        }
        throw new AssertionError("Missing native "+kind+" encoding");
    }
    static void compact(ABC abc)throws Exception {
        var body=method(abc);var code=body.getCode();
        for(String kind:List.of("string","super","negation","truth"))pair(body,kind);
        for(int i=code.code.size()-2;i>=0;i--) {
            var a=code.code.get(i);var b=code.code.get(i+1);
            int op=a.definition.instructionCode,next=b.definition.instructionCode;
            if(op==0x2c && next==0x85)code.removeInstruction(i+1,body);
            else if(op==0x45 && next==0x29) {
                code.replaceInstruction(i,new AVM2Instruction(0,0x4e,a.operands.clone()),body);
                code.removeInstruction(i+1,body);
            } else if((op==0x96 || op==0x76) && (next==0x11 || next==0x12)) {
                if(op==0x96)code.replaceInstruction(i+1,
                    new AVM2Instruction(0,next==0x11?0x12:0x11,b.operands.clone()),body);
                code.removeInstruction(i,body);
            }
        }
        body.setCode(code);
    }
    static void guards(Path input)throws Exception {
        for(String kind:List.of("string","super","negation","truth"))
            for(String mutation:List.of("valid","target","handler","operand")) {
                var reference=abc(input);compact(reference);
                var candidate=abc(input);var body=method(candidate);var code=body.getCode();
                int index=pair(body,kind);
                var first=code.code.get(index);var second=code.code.get(index+1);
                int interior=(int)second.getAddress();
                if(mutation.equals("target")) {
                    var jump=code.code.stream().filter(i->i.definition.instructionCode==0x10)
                        .findFirst().orElseThrow();
                    jump.operands[0]=interior-(int)jump.getAddress()-jump.getBytesLength();
                }
                if(mutation.equals("handler"))body.exceptions[0].end=interior;
                if(mutation.equals("operand")) {
                    if(kind.equals("super"))first.operands[1]++;
                    else if(kind.equals("string"))first.operands[0]=candidate.constants.getStringId("changed",true);
                    else code.replaceInstruction(index+1,new AVM2Instruction(0,
                        second.definition.instructionCode==0x11?0x12:0x11,second.operands.clone()),body);
                }
                body.setCode(code);
                byte[] before=body.getCodeBytes().clone();
                MovieSource.emitCompactOperations(reference,candidate);
                if(mutation.equals("valid")) {
                    if(!Arrays.equals(method(reference).getCodeBytes(),body.getCodeBytes()))
                        throw new AssertionError("Failed valid "+kind);
                } else if(!Arrays.equals(before,body.getCodeBytes()))
                    throw new AssertionError("Rewrote unsafe "+kind+" "+mutation);
            }
    }
    public static void main(String[] args)throws Exception {
        Path input=Path.of(args[0]);guards(input);
        var swf=MovieSource.read(input);var abc=MovieSource.findScript(swf,"CompactTruth").abc;
        compact(abc);((Tag)abc.parentTag).setModified(true);
        try(var out=Files.newOutputStream(Path.of(args[1]))){swf.saveTo(out);}
        System.exit(0);
    }
}

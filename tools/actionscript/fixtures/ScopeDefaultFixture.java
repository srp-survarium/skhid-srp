import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.abc.*;
import com.jpexs.decompiler.flash.abc.types.*;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.tags.Tag;

/** Reference layout derived solely from a project-authored native compilation. */
public class ScopeDefaultFixture {
    static ABC abc(Path path)throws Exception {
        return MovieSource.findScript(MovieSource.read(path),"fixture.DefaultProbe").abc;
    }
    static MethodBody method(ABC abc)throws Exception {
        int id=MovieSource.names(abc).entrySet().stream().filter(e->e.getValue().endsWith(",\"test\")"))
            .findFirst().orElseThrow().getKey();
        var body=abc.findBody(id);MovieSource.exactCode(body);return body;
    }
    static List<AVM2Instruction> executable(MethodBody body) {
        return body.getCode().code.stream().filter(i->!MovieSource.debugInstruction(i)).toList();
    }
    static AVM2Instruction copy(AVM2Instruction instruction) {
        return new AVM2Instruction(0,instruction.definition,
            instruction.operands==null?null:instruction.operands.clone());
    }
    static void normalize(ABC abc,boolean together)throws Exception {
        var body=method(abc);var code=body.getCode();
        for(int i=code.code.size()-1;i>=0;i--)if(MovieSource.debugInstruction(code.code.get(i)))code.removeInstruction(i,body);
        int[] shape={0xd0,0x30,0x20,0x80,0xd6,0x24,0xd7};
        for(int i=0;i<shape.length;i++)if(code.code.get(i).definition.instructionCode!=shape[i])
            throw new AssertionError("Unexpected native default prologue at "+i);
        var prologue=code.code.subList(0,7).stream().map(ScopeDefaultFixture::copy).toList();
        for(int i=6;i>=0;i--)code.removeInstruction(i,body);
        int first=-1,last=-1;
        for(int i=0,returns=0;i<code.code.size();i++)if(code.code.get(i).definition.instructionCode==0x47) {
            if(++returns==1)first=i+1;
            if(returns==2){last=i+1;break;}
        }
        if(first<0 || last<0)throw new AssertionError("Missing source guards");
        if(together)last=first;
        for(int j=2;j<7;j++)code.insertInstruction(last+j-2,prologue.get(j),false,body);
        code.insertInstruction(first,prologue.get(0),false,body);
        code.insertInstruction(first+1,prologue.get(1),false,body);
        body.setCode(code);
    }
    static void guards(Path input)throws Exception {
        for(String mutation:List.of("valid","valid-together","early-read","early-write","parameter","duplicate-local",
                "nonzero","wrong-type","dynamic-type","handler","activation","nested",
                "old-initializer-entry","interior-entry","skip-defaults","backedge","different-reference")) {
            var reference=abc(input);normalize(reference,mutation.equals("valid-together"));
            var candidate=abc(input);var body=method(candidate);var code=body.getCode();
            var actual=executable(body);var expected=executable(method(reference));
            int prefix=code.code.indexOf(actual.get(7));
            if(mutation.equals("early-read")) {
                code.insertInstruction(prefix,new AVM2Instruction(0,0xd2,null),true,body);
                code.insertInstruction(prefix+1,new AVM2Instruction(0,0x29,null),true,body);
            }
            if(mutation.equals("early-write")) {
                code.insertInstruction(prefix,new AVM2Instruction(0,0x24,new int[]{0}),true,body);
                code.insertInstruction(prefix+1,new AVM2Instruction(0,0xd6,null),true,body);
            }
            if(mutation.equals("parameter"))actual.get(4).definition=new AVM2Instruction(0,0xd5,null).definition;
            if(mutation.equals("duplicate-local"))actual.get(6).definition=new AVM2Instruction(0,0xd6,null).definition;
            if(mutation.equals("nonzero"))actual.get(5).operands[0]=1;
            if(mutation.equals("wrong-type"))actual.get(3).operands[0]=candidate.constants.getMultinameId(
                Multiname.createQName(false,candidate.constants.getStringId("Object",true),
                    candidate.constants.getNamespaceId(0x16,"",0,true)),true);
            if(mutation.equals("dynamic-type"))candidate.constants.getMultiname(actual.get(3).operands[0]).kind=9;
            if(mutation.equals("handler"))body.exceptions=new ABCException[]{new ABCException()};
            if(mutation.equals("activation"))code.insertInstruction(prefix,new AVM2Instruction(0,0x57,null),true,body);
            if(mutation.equals("nested"))code.insertInstruction(prefix,new AVM2Instruction(0,0x40,new int[]{body.method_info}),true,body);
            var branches=actual.stream().filter(i->i.definition.instructionCode==0x12).toList();
            if(mutation.equals("old-initializer-entry") || mutation.equals("interior-entry")) {
                var jump=branches.get(0);var target=actual.get(mutation.equals("old-initializer-entry")?2:3);
                jump.operands[0]=(int)target.getAddress()-(int)jump.getAddress()-4;
            }
            if(mutation.equals("skip-defaults")) {
                var jump=branches.get(1);
                jump.operands[0]=(int)actual.get(actual.size()-1).getAddress()-(int)jump.getAddress()-4;
            }
            if(mutation.equals("backedge")) {
                var jump=new AVM2Instruction(0,0x10,new int[]{0});code.replaceInstruction(code.code.size()-1,jump,body);
                jump.operands[0]=(int)actual.get(7).getAddress()-(int)jump.getAddress()-4;
            }
            if(mutation.equals("different-reference"))
                expected.stream().filter(i->i.definition.instructionCode==0x24).findFirst().orElseThrow().operands[0]=1;
            body.setCode(code);var emitted=executable(body);
            if((MovieSource.scopeVariant(candidate,body,expected,emitted)!=emitted)!=mutation.startsWith("valid"))
                throw new AssertionError("Default scheduling guard: "+mutation);
            if(mutation.startsWith("valid")) {
                MovieSource.emitCompactOperations(reference,candidate);MovieSource.retainDebugDirectives(reference,candidate);
                if(!Arrays.equals(method(reference).getCodeBytes(),body.getCodeBytes()))
                    throw new AssertionError("Default scheduling did not match");
            }
        }
    }
    public static void main(String[] args)throws Exception {
        Path input=Path.of(args[0]);guards(input);
        var swf=MovieSource.read(input);var abc=MovieSource.findScript(swf,"fixture.DefaultProbe").abc;
        normalize(abc,false);
        int disabledId=MovieSource.names(abc).entrySet().stream()
            .filter(e->e.getValue().endsWith(",\"disabled\")")).findFirst().orElseThrow().getKey();
        var disabled=abc.findBody(disabledId);
        var code=MovieSource.exactCode(disabled);
        if(code.code.size()!=3 || code.code.get(0).definition.instructionCode!=0xd0
                || code.code.get(1).definition.instructionCode!=0x30
                || code.code.get(2).definition.instructionCode!=0x47)
            throw new AssertionError("Unexpected native empty-method prologue");
        // Released patches can disable an instance handler by leaving only
        // returnvoid while retaining its original verifier capacities.
        code.removeInstruction(0,disabled);code.removeInstruction(0,disabled);
        disabled.setCode(code);
        ((Tag)abc.parentTag).setModified(true);
        try(var out=Files.newOutputStream(Path.of(args[1]))){swf.saveTo(out);}
        System.exit(0);
    }
}

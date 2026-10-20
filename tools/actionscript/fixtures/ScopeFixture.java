import java.nio.file.*;
import java.util.*;
import com.jpexs.decompiler.flash.abc.*;
import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
import com.jpexs.decompiler.flash.abc.types.traits.*;
import com.jpexs.decompiler.flash.tags.Tag;
import com.jpexs.decompiler.graph.TypeItem;

/** Mutations of actual compiler records, used only by test_scope_reserves.py. */
public class ScopeFixture {
    static ABC abc(Path path)throws Exception {
        return MovieSource.findScript(MovieSource.read(path),"fixture.ScopeProbe").abc;
    }
    static com.jpexs.decompiler.flash.abc.types.MethodBody method(ABC abc,String name)throws Exception {
        int id=MovieSource.names(abc).entrySet().stream().filter(e->e.getValue().endsWith(",\""+name+"\")"))
            .findFirst().orElseThrow().getKey();
        var body=abc.findBody(id);MovieSource.exactCode(body);return body;
    }
    static void normalize(ABC abc,String name)throws Exception {
        var body=method(abc,name);var code=body.getCode();
        for(int i=code.code.size()-1;i>=0;i--)if(MovieSource.debugInstruction(code.code.get(i)))code.removeInstruction(i,body);
        if(MovieSource.localRegister(code.code.get(0),false)!=0 || code.code.get(1).definition.instructionCode!=0x30)
            throw new AssertionError("Missing compiler scope prologue");
        code.removeInstruction(1,body);code.removeInstruction(0,body);
        if(!name.equals("omit")) {
            int site=0;while(!MovieSource.methodReturn(code.code.get(site)))site++;
            code.insertInstruction(site+1,new AVM2Instruction(0,0xd0,null),false,body);
            code.insertInstruction(site+2,new AVM2Instruction(0,0x30,null),false,body);
        }
        body.setCode(code);
    }
    static List<AVM2Instruction> executable(com.jpexs.decompiler.flash.abc.types.MethodBody body) {
        return body.getCode().code.stream().filter(i->!MovieSource.debugInstruction(i)).toList();
    }
    static void guards(Path input)throws Exception {
        for(String name:List.of("omit","defer","value"))for(String mutation:List.of("valid","shadow","inherited-shadow",
                "missing-parent","duplicate-class","alias","scope-read","activation","nested","with",
                "extra-local","prologue-entry","dynamic-name","handler","backedge","skip")) {
            if((mutation.equals("backedge") || mutation.equals("skip")) && name.equals("omit"))continue;
            var reference=abc(input);normalize(reference,name);
            var candidate=abc(input);var body=method(candidate,name);var code=body.getCode();
            var original=executable(body);var expected=executable(method(reference,name));
            var owner=candidate.instance_info.get(candidate.findClassByName("fixture.ScopeProbe"));
            var lookup=original.stream().filter(i->i.definition.instructionCode==0x60).findFirst().orElseThrow();
            if(mutation.equals("shadow") || mutation.equals("inherited-shadow")) {
                var trait=new TraitSlotConst();trait.name_index=lookup.operands[0];trait.kindType=0;
                var target=mutation.equals("shadow")?candidate
                    :MovieSource.findScript(candidate.getSwf(),"fixture.ScopeBase").abc;
                int index=target.findClassByName(mutation.equals("shadow")?"fixture.ScopeProbe":"fixture.ScopeBase");
                if(target!=candidate) {
                    // Pool indices are local to each ABC.
                    trait.name_index=target.constants.getMultinameId(
                        com.jpexs.decompiler.flash.abc.types.Multiname.createQName(false,
                            target.constants.getStringId("ScopeHelper",true),
                            target.constants.getNamespaceId(0x16,"",0,true)),true);
                }
                target.instance_info.get(index).instance_traits.traits.add(trait);
            }
            if(mutation.equals("missing-parent"))candidate.getSwf().getAbcIndex()
                .findClass(new TypeItem("fixture.ScopeProbe"),candidate,null).parent=null;
            if(mutation.equals("duplicate-class")) {
                var target=MovieSource.findScript(candidate.getSwf(),"ScopeHelper").abc;
                var trait=target.script_info.stream().flatMap(s->s.traits.traits.stream())
                    .filter(t->t instanceof TraitClass && "ScopeHelper".equals(MovieSource.publicName(target,t.name_index)))
                    .findFirst().orElseThrow();target.script_info.get(0).traits.traits.add(trait);
            }
            if(mutation.equals("alias"))owner.instance_traits.traits.add(owner.instance_traits.traits.stream()
                .filter(t->t instanceof TraitMethodGetterSetter m && m.method_info==body.method_info).findFirst().orElseThrow());
            int insert=code.code.indexOf(original.get(2));
            var extra=switch(mutation) {
                case "scope-read" -> new AVM2Instruction(0,0x65,new int[]{0});
                case "activation" -> new AVM2Instruction(0,0x57,null);
                case "nested" -> new AVM2Instruction(0,0x40,new int[]{body.method_info});
                case "with" -> new AVM2Instruction(0,0x1c,null);
                case "extra-local" -> new AVM2Instruction(0,0x62,new int[]{candidate.method_info.get(body.method_info).param_types.length+1});
                default -> null;
            };
            if(extra!=null)code.insertInstruction(insert,extra,true,body);
            if(mutation.equals("prologue-entry")) {
                var jump=new AVM2Instruction(0,0x10,new int[]{0});code.insertInstruction(insert,jump,true,body);
                jump.operands[0]=(int)original.get(0).getAddress()-(int)jump.getAddress()-4;
            }
            if(mutation.equals("dynamic-name"))candidate.constants.getMultiname(lookup.operands[0]).kind=9;
            if(mutation.equals("handler"))body.exceptions=new com.jpexs.decompiler.flash.abc.types.ABCException[]{
                new com.jpexs.decompiler.flash.abc.types.ABCException()};
            if(mutation.equals("backedge")) {
                int last=code.code.size()-1;var jump=new AVM2Instruction(0,0x10,new int[]{0});
                code.replaceInstruction(last,jump,body);jump.operands[0]=(int)original.get(2).getAddress()-(int)jump.getAddress()-4;
            }
            if(mutation.equals("skip")) {
                var jump=original.stream().filter(i->i.definition.instructionCode==0x12).findFirst().orElseThrow();
                jump.operands[0]=(int)original.get(original.size()-1).getAddress()-(int)jump.getAddress()-4;
            }
            body.setCode(code);var emitted=executable(body);
            var variant=MovieSource.scopeVariant(candidate,body,expected,emitted);
            if((variant!=emitted)!=mutation.equals("valid"))throw new AssertionError(name+" scope guard: "+mutation);
            if(mutation.equals("valid")) {
                MovieSource.emitCompactOperations(reference,candidate);MovieSource.retainDebugDirectives(reference,candidate);
                if(!Arrays.equals(method(reference,name).getCodeBytes(),body.getCodeBytes()))
                    throw new AssertionError("Scope encoding did not match: "+name);
            }
        }
    }
    static void checkReserved(Path before,Path after)throws Exception {
        var original=MovieSource.read(before);var changed=MovieSource.read(after);
        if(original.getAbcList().size()!=changed.getAbcList().size())throw new AssertionError("ABC count changed");
        int increases=0;
        for(int i=0;i<original.getAbcList().size();i++) {
            var a=original.getAbcList().get(i).getABC();var b=changed.getAbcList().get(i).getABC();
            for(var body:a.bodies) {
                var other=b.findBody(body.method_info);
                if(other.max_stack<body.max_stack || other.max_regs<body.max_regs)throw new AssertionError("Reduced capacity");
                if(other.max_stack!=body.max_stack || other.max_regs!=body.max_regs)increases++;
                other.max_stack=body.max_stack;other.max_regs=body.max_regs;
            }
            if(!Arrays.equals(MovieSource.abcBytes(a),MovieSource.abcBytes(b)))throw new AssertionError("Reservation changed other records");
        }
        if(increases!=2)throw new AssertionError("Expected exactly two reserved methods");
    }
    public static void main(String[] args)throws Exception {
        if(args[0].equals("check"))checkReserved(Path.of(args[1]),Path.of(args[2]));
        else {
            Path input=Path.of(args[0]);guards(input);
            var swf=MovieSource.read(input);var abc=MovieSource.findScript(swf,"fixture.ScopeProbe").abc;
            normalize(abc,"omit");normalize(abc,"defer");normalize(abc,"value");((Tag)abc.parentTag).setModified(true);
            try(var out=Files.newOutputStream(Path.of(args[1]))){swf.saveTo(out);}
        }
        System.exit(0);
    }
}

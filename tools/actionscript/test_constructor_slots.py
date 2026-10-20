"""Real compiler coverage for constructor slot and repeated-coercion encodings."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source


@unittest.skipUnless(os.environ.get('SRP_AS_TOOLS'), 'set SRP_AS_TOOLS to run real compiler integration')
class ConstructorSlotIntegration(unittest.TestCase):
    def test_compiler_records_and_conservative_encoding_guards(self):
        with tempfile.TemporaryDirectory(prefix='srp-slot-test-') as temp:
            root = Path(temp)
            config_path = Path(os.environ['SRP_AS_TOOLS']).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config['flex'])
            tools = movie_source.Toolchain(config_path, root / 'bridge')
            source = root / 'fixture' / 'SlotProbe.as'
            source.parent.mkdir()
            source.write_text('''package fixture {
                import flash.display.Sprite;
                import flash.display.MovieClip;
                public class SlotProbe extends Sprite {
                    private var clip:MovieClip;
                    protected var items:Array;
                    public function assign(value:MovieClip):void { this.clip=value; }
                    public function SlotProbe() {
                        super();
                        var chosen:MovieClip=MovieClip(this.getChildByName("original-child"));
                        this.clip=chosen;
                        this.items=[this.clip,this];
                    }
                }
            }''')
            seed = root / 'seed.swf'
            result = subprocess.run([
                config['java'], '-jar', str(sdk / 'lib/mxmlc.jar'),
                '+flexlib=' + str(sdk / 'frameworks'),
                '-external-library-path=' + str(sdk / 'frameworks/libs/player/32.0/playerglobal.swc'),
                '-library-path=', '-source-path=' + str(root),
                '-debug=false', '-optimize=false', '-output=' + str(seed), str(source),
            ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout)

            def run(*args, name, success=True):
                log = root / (name + '.log')
                result = tools.run(*args, log=log)
                self.assertEqual(result.returncode == 0, success, log.read_text())
                return log.read_text()

            def compile_from(reference, selected, name):
                output = root / (name + '.swf')
                run('compile', reference, output, 'fixture.SlotProbe', selected, root / (name + '-snapshot'),
                    'ffdec-source', sdk, name=name)
                return output

            emitted = compile_from(seed, source, 'emitted')
            helper = root / 'SlotFixture.java'
            helper.write_text('''import java.nio.file.*;
                import java.util.*;
                import com.jpexs.decompiler.flash.abc.*;
                import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
                import com.jpexs.decompiler.flash.abc.types.*;
                import com.jpexs.decompiler.flash.tags.Tag;
                public class SlotFixture {
                    static ABC abc(Path path)throws Exception {
                        return MovieSource.findScript(MovieSource.read(path),"fixture.SlotProbe").abc;
                    }
                    static MethodBody constructor(ABC abc)throws Exception {
                        var body=abc.findBody(abc.instance_info.get(abc.findClassByName("fixture.SlotProbe")).iinit_index);
                        MovieSource.exactCode(body);return body;
                    }
                    static MethodBody assignment(ABC abc)throws Exception {
                        int id=MovieSource.names(abc).entrySet().stream()
                            .filter(e->e.getValue().endsWith(",\\"assign\\")")).findFirst().orElseThrow().getKey();
                        var body=abc.findBody(id);MovieSource.exactCode(body);return body;
                    }
                    static List<AVM2Instruction> executable(MethodBody body) {
                        return body.getCode().code.stream().filter(i->!MovieSource.debugInstruction(i)).toList();
                    }
                    static int slot(List<AVM2Instruction> code) {
                        for(int i=0;i<code.size();i++)if(code.get(i).definition.instructionCode==0x61)return i;
                        throw new AssertionError("Missing constructor write");
                    }
                    static int cast(List<AVM2Instruction> code) {
                        for(int i=0;i+1<code.size();i++)if(code.get(i).definition.instructionCode==0x80
                            && code.get(i+1).definition.instructionCode==0x80
                            && Arrays.equals(code.get(i).operands,code.get(i+1).operands))return i;
                        throw new AssertionError("Missing compiler repeated coercion");
                    }
                    static void normalize(ABC abc)throws Exception {
                        var body=constructor(abc);var code=body.getCode();int writes=0;
                        for(int i=0;i<code.code.size();i++)if(code.code.get(i).definition.instructionCode==0x61) {
                            var old=code.code.get(i);
                            code.replaceInstruction(i,new AVM2Instruction(0,0x68,old.operands.clone()),body);writes++;
                        }
                        if(writes!=2)throw new AssertionError("Unexpected field writes");
                        code.removeInstruction(cast(code.code)+1,body);body.setCode(code);
                        body=assignment(abc);code=body.getCode();
                        int index=slot(code.code);var write=code.code.get(index);
                        code.replaceInstruction(index,new AVM2Instruction(0,0x68,write.operands.clone()),body);
                        body.setCode(code);
                    }
                    static void slotGuards(Path path)throws Exception {
                        for(boolean ordinary:List.of(false,true))
                        for(String mutation:List.of("valid","const","setter","missing","duplicate","public",
                                "foreign","register-zero","increment-zero","kill-zero","hasnext-zero",
                                "ordinary","static-method","aliased-method","branch","debug-branch","handler","dup","swap")) {
                            if(!ordinary && (mutation.equals("static-method") || mutation.equals("aliased-method")))continue;
                            var abc=abc(path);var body=ordinary?assignment(abc):constructor(abc);var full=body.getCode();
                            var code=executable(body);int site=slot(code);int name=code.get(site).operands[0];
                            var traits=abc.instance_info.get(abc.findClassByName("fixture.SlotProbe")).instance_traits.traits;
                            var trait=traits.stream().filter(t->t.name_index==name).findFirst().orElseThrow();
                            if(mutation.equals("static-method") || mutation.equals("aliased-method")) {
                                var method=traits.stream().filter(t->t instanceof
                                    com.jpexs.decompiler.flash.abc.types.traits.TraitMethodGetterSetter m
                                    && m.method_info==body.method_info).findFirst().orElseThrow();
                                abc.class_info.get(abc.findClassByName("fixture.SlotProbe")).static_traits.traits.add(method);
                                if(mutation.equals("static-method"))traits.remove(method);
                            }
                            int receiver=-1;
                            for(int i=site-1;i>=0;i--)if(MovieSource.localRegister(code.get(i),false)==0) {
                                receiver=i;break;
                            }
                            if(receiver<0 || MovieSource.localRegister(code.get(receiver),false)!=0)
                                throw new AssertionError("Missing direct receiver");
                            var receiverInstruction=code.get(receiver);var interior=code.get(receiver+1);
                            int interiorIndex=full.code.indexOf(interior);
                            if(mutation.equals("const"))trait.kindType=6;
                            if(mutation.equals("setter"))trait.kindType=3;
                            if(mutation.equals("missing"))traits.remove(trait);
                            if(mutation.equals("duplicate"))traits.add(trait);
                            if(mutation.equals("public"))abc.constants.getNamespace(
                                abc.constants.getMultiname(name).namespace_index).kind=0x16;
                            if(mutation.equals("foreign"))full.replaceInstruction(full.code.indexOf(receiverInstruction),
                                new AVM2Instruction(0,0xd1,null),body);
                            if(mutation.equals("register-zero"))full.insertInstruction(0,
                                new AVM2Instruction(0,0xd4,null),true,body);
                            if(mutation.equals("increment-zero"))full.insertInstruction(0,
                                new AVM2Instruction(0,0x92,new int[]{0}),true,body);
                            if(mutation.equals("kill-zero"))full.insertInstruction(0,
                                new AVM2Instruction(0,0x08,new int[]{0}),true,body);
                            if(mutation.equals("hasnext-zero"))full.insertInstruction(0,
                                new AVM2Instruction(0,0x32,new int[]{1,0}),true,body);
                            if(mutation.equals("ordinary")) {
                                if(ordinary)traits.removeIf(t->t instanceof
                                    com.jpexs.decompiler.flash.abc.types.traits.TraitMethodGetterSetter m
                                    && m.method_info==body.method_info);
                                else abc.instance_info.get(abc.findClassByName("fixture.SlotProbe")).iinit_index=-1;
                            }
                            if(mutation.equals("dup") || mutation.equals("swap"))full.insertInstruction(interiorIndex,
                                new AVM2Instruction(0,mutation.equals("dup")?0x2a:0x2b,null),true,body);
                            if(mutation.equals("branch") || mutation.equals("debug-branch")) {
                                if(mutation.equals("debug-branch")) {
                                    var debug=new AVM2Instruction(0,0xf0,new int[]{123});
                                    full.insertInstruction(interiorIndex,debug,true,body);interior=debug;
                                }
                                full.insertInstruction(0,new AVM2Instruction(0,0x10,new int[]{0}),true,body);
                                full.code.get(0).operands[0]=(int)interior.getAddress()-4;
                            }
                            if(mutation.equals("handler")) {
                                var handler=new ABCException();handler.start=0;handler.end=(int)interior.getAddress();
                                handler.target=(int)interior.getAddress();body.exceptions=new ABCException[]{handler};
                            }
                            body.setCode(full);code=executable(body);site=slot(code);
                            boolean accepted=MovieSource.mutableInstanceSlot(abc,body,code,site);
                            if(accepted!=mutation.equals("valid"))throw new AssertionError("Slot guard: "+mutation);
                        }
                    }
                    static void coercionGuards(Path path)throws Exception {
                        for(String mutation:List.of("valid","retained","type","opcode","branch","handler")) {
                            var reference=abc(path);var candidate=abc(path);
                            normalize(reference);
                            var before=constructor(reference);var after=constructor(candidate);
                            var expected=before.getCode();var actual=after.getCode();
                            int site=cast(actual.code);var first=actual.code.get(site);var second=actual.code.get(site+1);
                            var target=expected.code.stream().filter(i->i.definition.instructionCode==0x80
                                && Arrays.equals(i.operands,first.operands)).findFirst().orElseThrow();
                            // An already matching pair must survive while the
                            // independent constructor-write encoding is applied.
                            if(mutation.equals("retained"))expected.insertInstruction(expected.code.indexOf(target)+1,
                                new AVM2Instruction(0,0x80,target.operands.clone()),false,before);
                            if(mutation.equals("type"))second.operands[0]=candidate.instance_info.get(
                                candidate.findClassByName("fixture.SlotProbe")).name_index;
                            if(mutation.equals("opcode"))actual.replaceInstruction(site+1,
                                new AVM2Instruction(0,0x86,second.operands.clone()),after);
                            if(mutation.equals("branch")) {
                                actual.insertInstruction(0,new AVM2Instruction(0,0x10,new int[]{0}),true,after);
                                expected.insertInstruction(0,new AVM2Instruction(0,0x10,new int[]{0}),true,before);
                                actual.code.get(0).operands[0]=(int)second.getAddress()-4;
                                expected.code.get(0).operands[0]=(int)target.getAddress()-4;
                            }
                            if(mutation.equals("handler")) {
                                var a=new ABCException();var b=new ABCException();
                                a.start=b.start=0;a.end=a.target=(int)target.getAddress();
                                b.end=b.target=(int)second.getAddress();
                                before.exceptions=new ABCException[]{a};after.exceptions=new ABCException[]{b};
                            }
                            before.setCode(expected);after.setCode(actual);byte[] saved=after.getCodeBytes().clone();
                            MovieSource.emitCompactOperations(reference,candidate);
                            if(mutation.equals("valid") || mutation.equals("retained")) {
                                MovieSource.retainDebugDirectives(reference,candidate);
                                if(!Arrays.equals(before.getCodeBytes(),after.getCodeBytes()))
                                    throw new AssertionError("Valid constructor did not match");
                            } else if(!Arrays.equals(saved,after.getCodeBytes()))
                                throw new AssertionError("Accepted coercion mutation: "+mutation);
                        }
                    }
                    public static void main(String[] args)throws Exception {
                        Path path=Path.of(args[0]);slotGuards(path);coercionGuards(path);
                        var swf=MovieSource.read(path);var abc=MovieSource.findScript(swf,"fixture.SlotProbe").abc;
                        normalize(abc);((Tag)abc.parentTag).setModified(true);
                        try(var output=Files.newOutputStream(Path.of(args[1]))){swf.saveTo(output);}
                        System.exit(0);
                    }
                }''')
            cp = str(tools.work) + os.pathsep + config['ffdec']
            subprocess.run([config['javac'], '-cp', cp, '-d', str(root), str(helper)], check=True)
            reference = root / 'reference.swf'
            result = subprocess.run([config['java'], '-cp', str(root) + os.pathsep + cp, 'SlotFixture',
                                     str(emitted), str(reference)], stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout)
            baseline = compile_from(reference, source, 'baseline')
            linked = root / 'baseline-linked.swf'
            run('link-layout', reference, baseline, linked, 'fixture.SlotProbe', name='baseline-link')
            self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))
            edited = root / 'edited.as'
            edited.write_text(source.read_text().replace('original-child', 'edited-child'))
            candidate = compile_from(reference, edited, 'candidate')
            rejected = root / 'rejected.swf'
            run('link-layout', reference, candidate, rejected, 'fixture.SlotProbe', name='rejected', success=False)
            self.assertFalse(rejected.exists())
            owner = 'QName(PackageNamespace("fixture"),"SlotProbe")/instance-init'
            permissions = root / 'permissions.tsv'
            permissions.write_text(owner + '\t-\t-\n')
            feature = root / 'feature.swf'
            log = run('link-feature', reference, candidate, feature, 'fixture.SlotProbe', permissions,
                      name='feature-unreviewed', success=False)
            self.assertIn('Unreviewed calls/casts changed', log)
            self.assertFalse(feature.exists())
            # A changed method cannot use the whole-method encoding match.
            # Its actual compiler output retains the second MovieClip coercion;
            # the ordinary feature gate must require that explicit review.
            permissions.write_text(
                owner + '\tbd46e6d5a5a279e6bea4b4c9203a59fa31660f204aa86768861a19aa313212d5'
                '\t33f5ae81a272bbda1dbf6dbdc1608ff176be0c050c251fa5fde5ff0514ccb50d\n')
            run('link-feature', reference, candidate, feature, 'fixture.SlotProbe', permissions, name='feature')
            run('snapshot', feature, root / 'feature-records', name='feature-records')
            assembly = '\n'.join(p.read_text() for p in (root / 'feature-records').glob('abc-*/method-*.asm'))
            self.assertIn('edited-child', assembly)


if __name__ == '__main__':
    unittest.main()

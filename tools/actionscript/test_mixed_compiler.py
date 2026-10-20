"""Real mixed-compiler source, nested-function and feature-gate regression."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source
import source_patches


@unittest.skipUnless(os.environ.get('SRP_AS_TOOLS'), 'set SRP_AS_TOOLS to run real compiler integration')
class MixedCompilerIntegration(unittest.TestCase):
    def test_explicit_composition_preserves_source_edits_and_rejects_invalid_inputs(self):
        with tempfile.TemporaryDirectory(prefix='srp-mixed-test-') as temp:
            root = Path(temp)
            config_path = Path(os.environ['SRP_AS_TOOLS']).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config['flex'])
            tools = movie_source.Toolchain(config_path, root / 'bridge')
            source = root / 'MixedProbe.as'
            source.write_text('''package {
                import flash.display.Sprite;
                public class MixedProbe extends Sprite {
                    private var secret:int=7;
                    public function nativeValue(value:int):int { return value+secret; }
                    public function selfUint(value:uint):uint { value = ++value; return value; }
                    public function selfInt(value:int):int { value = --value; return value; }
                    public function mixed(value:int=2):String {
                        var callback:Function=function(delta:int):String {
                            var object:Object={oldKey:"old-marker"};
                            return object.oldKey + String(value+delta+123456);
                        };
                        try { if(value<0) throw new Error("negative"); return callback(3); }
                        catch(error:Error) { return callback(4); }
                        return "fallthrough";
                    }
                }
            }''')
            owner = 'QName(PackageNamespace(""),"MixedProbe")/instance/1/QName(PackageNamespace(""),"mixed")'
            self_owners = [owner.replace('"mixed")', '"' + name + '")') for name in ('selfUint', 'selfInt')]
            entry = {'compiler': 'flex-mixed', 'composition': {
                'ffdec_methods': [owner] + self_owners, 'script_traits': 'ffdec'}}
            plan = root / 'composition.tsv'
            plan.write_bytes(source_patches.composition_plan(entry))
            asc = root / 'asc.swf'
            result = subprocess.run([
                config['java'], '-jar', str(sdk / 'lib/mxmlc.jar'),
                '+flexlib=' + str(sdk / 'frameworks'),
                '-external-library-path=' + str(sdk / 'frameworks/libs/player/32.0/playerglobal.swc'),
                '-library-path=', '-debug=false', '-optimize=false', '-output=' + str(asc), str(source),
            ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout)

            def run(*args, name, success=True):
                log = root / (name + '.log')
                result = tools.run(*args, log=log)
                if success:
                    self.assertEqual(result.returncode, 0, log.read_text())
                else:
                    self.assertNotEqual(result.returncode, 0, log.read_text())
                return log.read_text()

            secondary_backend = 'ffdec'

            def compile_from(reference, selected_source, name):
                output = root / (name + '.swf')
                run('compile', reference, output, 'MixedProbe', selected_source,
                    root / (name + '-snapshot'), 'flex-mixed', sdk, plan, name=name)
                retained = root / (name + '-snapshot-mixed')
                self.assertEqual((retained / 'source.as').read_bytes(), selected_source.read_bytes())
                self.assertEqual((retained / 'composition.tsv').read_bytes(), plan.read_bytes())
                self.assertTrue((retained / 'flex-library.swf').is_file())
                self.assertTrue((retained / (secondary_backend + '.swf')).is_file())
                return output

            # Construct the mixed reference from real compilers, then rebuild it
            # independently. No original executable record is a compiler input substitute.
            reference = compile_from(asc, source, 'reference')
            baseline = compile_from(reference, source, 'baseline')
            linked = root / 'baseline-linked.swf'
            run('link-layout', reference, baseline, linked, 'MixedProbe', name='baseline-link')
            self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))

            changed = root / 'MixedProbe-edited.as'
            changed.write_text(source.read_text().replace('oldKey', 'newMixedFixtureKey')
                               .replace('old-marker', 'new-mixed-fixture-marker').replace('123456', '87654321'))
            candidate = compile_from(reference, changed, 'edited')
            rejected = root / 'edited-baseline-linked.swf'
            run('link-layout', reference, candidate, rejected, 'MixedProbe', name='edited-baseline-link', success=False)
            self.assertFalse(rejected.exists())
            permissions = root / 'permissions.tsv'
            permissions.write_text(owner + '\t-\t-\n' + owner + '/function[0]\t-\t-\n')
            feature = root / 'feature-linked.swf'
            run('link-feature', reference, candidate, feature, 'MixedProbe', permissions, name='feature-link')
            run('snapshot', reference, root / 'reference-records', name='reference-records')
            run('snapshot', feature, root / 'feature-records', name='feature-records')
            comparison = movie_source.compare(reference, feature, root / 'reference-records', root / 'feature-records')
            self.assertEqual({row['owner'] for row in comparison['method_changes']}, {owner + '/function[0]'})
            assembly = '\n'.join(path.read_text() for path in (root / 'feature-records').glob('abc-*/method-*.asm'))
            for value in ('newMixedFixtureKey', 'new-mixed-fixture-marker', '87654321'):
                self.assertIn(value, assembly)

            # An edit outside the selected/authorized method must survive compilation
            # and then fail the untouched-method gate.
            unselected = root / 'MixedProbe-unselected.as'
            unselected.write_text(changed.read_text().replace('return value+secret;', 'return value+secret+1;'))
            unauthorized = compile_from(reference, unselected, 'unselected')
            log = run('link-feature', reference, unauthorized, root / 'unselected-feature.swf',
                      'MixedProbe', permissions, name='unselected-link', success=False)
            self.assertIn('Untouched method changed', log)
            self.assertFalse((root / 'unselected-feature.swf').exists())

            # Exercise composition rejection directly using already compiled inputs.
            # This helper also corrupts a retained constant to verify prefix authentication.
            helper = root / 'MixedFixture.java'
            helper.write_text('''import java.nio.file.*;
                import java.util.*;
                import com.jpexs.decompiler.flash.abc.avm2.AVM2Code;
                import com.jpexs.decompiler.flash.abc.avm2.instructions.AVM2Instruction;
                import com.jpexs.decompiler.flash.abc.types.ABCException;
                import com.jpexs.decompiler.flash.tags.Tag;
                public class MixedFixture {
                    static void numericAssignments(Path path)throws Exception {
                        for(String name:List.of("selfUint","selfInt")) {
                            for(String mutation:List.of("valid","register","conversion","non-numeric","branch","handler")) {
                                var reference=MovieSource.findScript(MovieSource.read(path),"MixedProbe").abc;
                                var candidate=MovieSource.findScript(MovieSource.read(path),"MixedProbe").abc;
                                int id=MovieSource.names(reference).entrySet().stream()
                                    .filter(e->e.getValue().endsWith(","+(char)34+name+(char)34+")")).findFirst().orElseThrow().getKey();
                                var before=reference.findBody(id);
                                var after=candidate.findBody(id);
                                MovieSource.exactCode(before);
                                MovieSource.exactCode(after);
                                var source=new ArrayList<>(after.getCode().code);
                                int duplicate=-1;
                                for(int i=0;i+4<source.size();i++) {
                                    int conversion=source.get(i+1).definition.instructionCode;
                                    if(source.get(i).definition.instructionCode==0x2a
                                            && (conversion==0x73||conversion==0x74)
                                            && source.get(i+3).definition.instructionCode==conversion) {
                                        if(duplicate>=0)throw new AssertionError("Ambiguous compiler fixture");
                                        duplicate=i;
                                    }
                                }
                                if(duplicate<2)throw new AssertionError("Missing real compiler self assignment");
                                var expected=new ArrayList<>(before.getCode().code);
                                var expectedConversion=expected.get(duplicate+3);
                                var actualConversion=source.get(duplicate+3);
                                expected.subList(duplicate,duplicate+3).clear();
                                if(mutation.equals("register")) {
                                    source.set(duplicate+2,new AVM2Instruction(0,0x63,new int[]{7}));
                                } else if(mutation.equals("conversion")) {
                                    int other=source.get(duplicate+1).definition.instructionCode==0x73?0x74:0x73;
                                    source.set(duplicate+1,new AVM2Instruction(0,other,null));
                                } else if(mutation.equals("non-numeric")) {
                                    // A coercion on a duplicated object may invoke user code twice.
                                    source.set(duplicate-1,new AVM2Instruction(0,0x82,null));
                                    expected.set(duplicate-1,new AVM2Instruction(0,0x82,null));
                                }
                                AVM2Instruction expectedJump=null,actualJump=null;
                                if(mutation.equals("branch")) {
                                    expectedJump=new AVM2Instruction(0,0x10,new int[]{0});
                                    actualJump=new AVM2Instruction(0,0x10,new int[]{0});
                                    expected.addAll(0,List.of(new AVM2Instruction(0,0x24,new int[]{0}),expectedJump));
                                    source.addAll(0,List.of(new AVM2Instruction(0,0x24,new int[]{0}),actualJump));
                                }
                                var expectedCode=new AVM2Code(expected);
                                var actualCode=new AVM2Code(source);
                                expectedCode.markOffsets();
                                actualCode.markOffsets();
                                if(expectedJump!=null) {
                                    expectedJump.operands[0]=(int)(expectedConversion.getAddress()-expectedJump.getAddress()-expectedJump.getBytesLength());
                                    actualJump.operands[0]=(int)(actualConversion.getAddress()-actualJump.getAddress()-actualJump.getBytesLength());
                                }
                                before.setCode(expectedCode);
                                after.setCode(actualCode);
                                if(mutation.equals("handler")) {
                                    var oldHandler=new ABCException();
                                    var newHandler=new ABCException();
                                    oldHandler.start=newHandler.start=0;
                                    oldHandler.end=(int)expectedConversion.getAddress();
                                    newHandler.end=(int)actualConversion.getAddress();
                                    oldHandler.target=(int)expected.get(expected.size()-1).getAddress();
                                    newHandler.target=(int)source.get(source.size()-1).getAddress();
                                    before.exceptions=new ABCException[]{oldHandler};
                                    after.exceptions=new ABCException[]{newHandler};
                                }
                                byte[] untouched=after.getCodeBytes().clone();
                                MovieSource.emitCompactOperations(reference,candidate);
                                if(mutation.equals("valid")) {
                                    MovieSource.retainDebugDirectives(reference,candidate);
                                    if(!Arrays.equals(before.getCodeBytes(),after.getCodeBytes()))
                                        throw new AssertionError("Numeric assignment was not folded with debugger directives");
                                } else if(!Arrays.equals(untouched,after.getCodeBytes())) {
                                    throw new AssertionError("Unsafe numeric fold accepted: "+mutation);
                                }
                            }
                        }
                    }
                    public static void main(String[] args)throws Exception {
                        if(args[0].equals("compose")) {
                            MixedCompiler.compose(java.util.Arrays.copyOfRange(args,1,args.length));
                        } else if(args[0].equals("numeric")) {
                            numericAssignments(Path.of(args[1]));
                        } else {
                            var swf=MovieSource.read(Path.of(args[1]));
                            var abc=MovieSource.findScript(swf,"MixedProbe").abc;
                            abc.constants.setString(1,"corrupted-retained-entry");
                            ((Tag)abc.parentTag).setModified(true);
                            try(var out=Files.newOutputStream(Path.of(args[2]))){swf.saveTo(out);}
                        }
                        System.exit(0);
                    }
                }''')
            cp = str(tools.work) + os.pathsep + config['ffdec']
            subprocess.run([config['javac'], '-cp', cp, '-d', str(root), str(helper)], check=True)
            java = [config['java'], '-cp', str(root) + os.pathsep + cp, 'MixedFixture']
            result = subprocess.run(java + ['numeric', str(reference)], stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout)
            retained = root / 'baseline-snapshot-mixed'
            for name, text, expected in [
                ('unknown', owner + 'Missing\n', 'Unknown or ambiguous'),
                ('duplicate', owner + '\n' + owner + '\n', 'duplicate selection'),
            ]:
                bad_plan = root / (name + '.tsv')
                bad_plan.write_text(text)
                output = root / (name + '.swf')
                result = subprocess.run(java + ['compose', str(reference), str(retained / 'flex-library.swf'),
                                        str(retained / 'ffdec.swf'), 'MixedProbe', str(bad_plan), str(output)],
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(expected, result.stdout)
                self.assertFalse(output.exists())
            corrupted = root / 'corrupted.swf'
            subprocess.run(java + ['corrupt', str(retained / 'ffdec.swf'), str(corrupted)], check=True, timeout=30)
            output = root / 'corrupted-composed.swf'
            result = subprocess.run(java + ['compose', str(reference), str(retained / 'flex-library.swf'),
                                    str(corrupted), 'MixedProbe', str(plan), str(output)],
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn('Changed pool prefix', result.stdout)
            self.assertFalse(output.exists())

            # The corrected source-call profile is also an explicit, retained
            # secondary compiler choice in mixed builds.
            secondary_backend = 'ffdec-source'
            entry['composition']['ffdec_compiler'] = secondary_backend
            plan.write_bytes(source_patches.composition_plan(entry))
            source_reference = compile_from(asc, source, 'source-reference')
            source_baseline = compile_from(source_reference, source, 'source-baseline')
            source_linked = root / 'source-baseline-linked.swf'
            run('link-layout', source_reference, source_baseline, source_linked, 'MixedProbe', name='source-baseline-link')
            self.assertEqual(movie_source.uncompressed(source_reference), movie_source.uncompressed(source_linked))
            assembly = '\n'.join(path.read_text() for path in (root / 'source-baseline-snapshot').glob('abc-*/method-*.asm'))
            self.assertIn('convert_s', assembly)


if __name__ == '__main__':
    unittest.main()

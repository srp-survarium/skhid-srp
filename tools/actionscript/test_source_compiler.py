"""Real source-call conversion regression, including shadowing and constructors."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source


@unittest.skipUnless(os.environ.get('SRP_AS_TOOLS'), 'set SRP_AS_TOOLS to run real compiler integration')
class SourceCompilerIntegration(unittest.TestCase):
    def test_explicit_string_calls_preserve_other_coercions_and_source_edits(self):
        with tempfile.TemporaryDirectory(prefix='srp-source-compiler-') as temp:
            root = Path(temp)
            config_path = Path(os.environ['SRP_AS_TOOLS']).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config['flex'])
            tools = movie_source.Toolchain(config_path, root / 'bridge')
            source = root / 'SourceProbe.as'
            source.write_text('''package {
                import flash.display.Sprite;
                public class SourceProbe extends Sprite {
                    private var object:Object = {};
                    public function explicit(value:*):String { return String(value); }
                    public function nullValue():String { return String(null); }
                    public function undefinedValue():String { return String(undefined); }
                    public function number(value:*):String { return String(Number(value)); }
                    public function assigned(value:*):String { var result:String=value; return result; }
                    public function shadow(value:*,String:Function):* { return String(value); }
                    public function construct(value:*):String { return new String(value); }
                    public function constructClass(kind:Class):Object { return new kind(); }
                    private function holder():Object { return object; }
                    public function branches(flag:Boolean,value:*):void {
                        if(flag) holder().text=String(value);
                        else holder().text=String(undefined);
                    }
                    public function nested(value:*):Function {
                        return function():String { return String(value); };
                    }
                }
            }''')
            seed = root / 'seed.swf'
            result = subprocess.run([
                config['java'], '-jar', str(sdk / 'lib/mxmlc.jar'), '+flexlib=' + str(sdk / 'frameworks'),
                '-external-library-path=' + str(sdk / 'frameworks/libs/player/32.0/playerglobal.swc'),
                '-library-path=', '-debug=false', '-optimize=false', '-output=' + str(seed), str(source),
            ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout)

            def run(*args, name, success=True):
                log = root / (name + '.log')
                result = tools.run(*args, log=log)
                if success:
                    self.assertEqual(result.returncode, 0, log.read_text())
                else:
                    self.assertNotEqual(result.returncode, 0, log.read_text())

            def compile_from(reference, selected_source, name, compiler='ffdec-source'):
                output = root / (name + '.swf')
                run('compile', reference, output, 'SourceProbe', selected_source, root / (name + '-snapshot'),
                    compiler, sdk, name=name)
                return output

            regular = compile_from(seed, source, 'regular', 'ffdec')
            reference = compile_from(seed, source, 'source')
            comparison = movie_source.compare(regular, reference, root / 'regular-snapshot', root / 'source-snapshot')
            prefix = 'QName(PackageNamespace(""),"SourceProbe")/instance/1/QName(PackageNamespace(""),'

            def owner(name):
                return prefix + '"' + name + '")'

            expected = {owner(name) for name in ('explicit', 'nullValue', 'undefinedValue', 'number', 'branches')}
            expected.add(owner('nested') + '/function[0]')
            self.assertEqual({entry['owner'] for entry in comparison['method_changes']}, expected)

            def assembly(snapshot, method_owner):
                records = movie_source.methods(root / snapshot)
                (abc, _), row = next((key, value) for key, value in records.items() if key[1] == method_owner)
                return (root / snapshot / f'abc-{int(abc):04d}' / f'method-{int(row["method"]):05d}.asm').read_text()

            for name, count in [('explicit', 1), ('nullValue', 1), ('undefinedValue', 1), ('number', 1), ('branches', 2)]:
                self.assertEqual(assembly('source-snapshot', owner(name)).count('convert_s'), count)
            self.assertIn('pushnull', assembly('source-snapshot', owner('nullValue')))
            self.assertIn('pushundefined', assembly('source-snapshot', owner('undefinedValue')))
            self.assertIn('coerce_s', assembly('source-snapshot', owner('assigned')))
            self.assertIn('convert_d', assembly('source-snapshot', owner('number')))
            for name in ('assigned', 'shadow', 'construct', 'constructClass'):
                self.assertEqual(assembly('regular-snapshot', owner(name)), assembly('source-snapshot', owner(name)))

            baseline = compile_from(reference, source, 'baseline')
            linked = root / 'baseline-linked.swf'
            run('link-layout', reference, baseline, linked, 'SourceProbe', name='baseline-link')
            self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))

            changed = root / 'edited.as'
            changed.write_text(source.read_text().replace(
                'explicit(value:*):String { return String(value); }',
                'explicit(value:*):String { return String(value+1); }'))
            candidate = compile_from(reference, changed, 'edited')
            rejected = root / 'edited-baseline.swf'
            run('link-layout', reference, candidate, rejected, 'SourceProbe', name='edited-baseline', success=False)
            self.assertFalse(rejected.exists())
            permissions = root / 'permissions.tsv'
            permissions.write_text(owner('explicit') + '\t-\t-\n')
            feature = root / 'feature.swf'
            run('link-feature', reference, candidate, feature, 'SourceProbe', permissions, name='feature')
            run('snapshot', feature, root / 'feature-snapshot', name='feature-snapshot')
            changed_methods = movie_source.compare(reference, feature, root / 'source-snapshot', root / 'feature-snapshot')
            self.assertEqual({entry['owner'] for entry in changed_methods['method_changes']}, {owner('explicit')})
            self.assertIn('add', assembly('feature-snapshot', owner('explicit')))
            self.assertIn('convert_s', assembly('feature-snapshot', owner('explicit')))

            broken = root / 'broken.as'
            broken.write_text('package { public class SourceProbe { public function broken( }')
            rejected = root / 'broken.swf'
            run('compile', reference, rejected, 'SourceProbe', broken, root / 'broken-snapshot',
                'ffdec-source', sdk, name='broken', success=False)
            self.assertFalse(rejected.exists())


if __name__ == '__main__':
    unittest.main()

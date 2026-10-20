"""Native local defaults may move only across guards that cannot observe them."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source


@unittest.skipUnless(os.environ.get('SRP_AS_TOOLS'), 'set SRP_AS_TOOLS to run real compiler integration')
class ScopeDefaultIntegration(unittest.TestCase):
    def test_native_defaults_guards_and_source_edits(self):
        with tempfile.TemporaryDirectory(prefix='srp-default-scope-test-') as temp:
            root = Path(temp)
            config_path = Path(os.environ['SRP_AS_TOOLS']).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config['flex'])
            tools = movie_source.Toolchain(config_path, root / 'bridge')
            (root / 'fixture').mkdir()
            (root / 'ScopeHelper.as').write_text('''package {
                public class ScopeHelper {
                    public static function take(value:Object):Boolean { return value==null; }
                }
            }''')
            source = root / 'fixture/DefaultProbe.as'
            source.write_text('''package fixture {
                import flash.display.Sprite;
                public class DefaultProbe extends Sprite {
                    private var marker:int=1;
                    public function disabled(value:Object):void { return; }
                    public function test(value:Object):void {
                        if(ScopeHelper.take(value)) return;
                        if(ScopeHelper.take(this)) return;
                        this.alpha=0.5;
                        var items:Array;
                        var index:int;
                        this.name=String(items)+String(index)+String(this.marker);
                    }
                }
            }''')
            seed = root / 'seed.swf'
            result = subprocess.run([
                config['java'], '-jar', str(sdk / 'lib/mxmlc.jar'), '+flexlib=' + str(sdk / 'frameworks'),
                '-external-library-path=' + str(sdk / 'frameworks/libs/player/32.0/playerglobal.swc'),
                '-library-path=', '-source-path=' + str(root), '-debug=false', '-optimize=false',
                '-output=' + str(seed), str(source),
            ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout)

            def run(*args, name, success=True):
                log = root / (name + '.log')
                result = tools.run(*args, log=log)
                self.assertEqual(result.returncode == 0, success, log.read_text())
                return log.read_text()

            def compile_from(reference, selected, name):
                output = root / (name + '.swf')
                run('compile', reference, output, 'fixture.DefaultProbe', selected, root / (name + '-snapshot'),
                    'flex-strict', sdk, name=name)
                return output

            emitted = seed
            cp = str(tools.work) + os.pathsep + config['ffdec']
            helper = Path(__file__).parent / 'fixtures/ScopeDefaultFixture.java'
            subprocess.run([config['javac'], '-cp', cp, '-d', str(root), str(helper)], check=True)
            reference = root / 'reference.swf'
            result = subprocess.run([
                config['java'], '-cp', str(root) + os.pathsep + cp, 'ScopeDefaultFixture',
                str(emitted), str(reference),
            ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout)
            baseline = compile_from(reference, source, 'baseline')
            linked = root / 'linked.swf'
            run('link-layout', reference, baseline, linked, 'fixture.DefaultProbe', name='baseline-link')
            self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))

            changed_empty = root / 'changed-empty.as'
            changed_empty.write_text(source.read_text().replace(
                'public function disabled(value:Object):void { return; }',
                'public function disabled(value:Object):void { trace(value); }'))
            changed_empty_movie = compile_from(reference, changed_empty, 'changed-empty')
            run('link-layout', reference, changed_empty_movie, root / 'changed-empty-linked.swf',
                'fixture.DefaultProbe', name='changed-empty-link', success=False)

            edited = root / 'edited.as'
            edited.write_text(source.read_text().replace('this.alpha=0.5', 'this.alpha=0.75'))
            candidate = compile_from(reference, edited, 'candidate')
            rejected = root / 'rejected.swf'
            run('link-layout', reference, candidate, rejected, 'fixture.DefaultProbe', name='rejected', success=False)
            self.assertFalse(rejected.exists())
            owner = 'QName(PackageNamespace("fixture"),"DefaultProbe")/instance/1/QName(PackageNamespace(""),"test")'
            permissions = root / 'permissions.tsv'
            permissions.write_text(owner + '\t-\t-\n')
            feature = root / 'feature.swf'
            log = run('link-feature', reference, candidate, feature, 'fixture.DefaultProbe', permissions,
                      name='feature-unreviewed', success=False)
            self.assertIn('Unreviewed calls/casts changed', log)
            self.assertFalse(feature.exists())
            # The changed method retains the compiler's original placement of
            # null's Array coercion before the helper calls. That changes the
            # ordered call/cast fingerprint and requires explicit review.
            permissions.write_text(
                owner + '\t5376961e37472a61b2845ba355eb7fb84111c1f4a0b2382106b8d896dd173805'
                '\t2b6f873c28737060a19cb515890c0f897e3b833d50fe3208f71617cda60571d6\n')
            run('link-feature', reference, candidate, feature, 'fixture.DefaultProbe', permissions, name='feature')
            run('snapshot', reference, root / 'reference-records', name='reference-records')
            run('snapshot', feature, root / 'feature-records', name='feature-records')
            comparison = movie_source.compare(reference, feature, root / 'reference-records', root / 'feature-records')
            self.assertEqual({r['owner'] for r in comparison['method_changes']}, {owner})
            assembly = '\n'.join(p.read_text() for p in (root / 'feature-records').glob('abc-*/method-*.asm'))
            self.assertIn('0.75', assembly)


if __name__ == '__main__':
    unittest.main()

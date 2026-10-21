"""Real compiler scope encodings and explicit capacity reservation provenance."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import movie_source
import source_patches


@unittest.skipUnless(os.environ.get('SRP_AS_TOOLS'), 'set SRP_AS_TOOLS to run real compiler integration')
class ScopeReserveIntegration(unittest.TestCase):
    def test_scope_proof_capacity_reservations_and_source_edits(self):
        with tempfile.TemporaryDirectory(prefix='srp-scope-test-') as temp:
            root = Path(temp)
            config_path = Path(os.environ['SRP_AS_TOOLS']).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config['flex'])
            tools = movie_source.Toolchain(config_path, root / 'bridge')
            (root / 'fixture').mkdir()
            (root / 'fixture/ScopeBase.as').write_text('''package fixture {
                import flash.display.Sprite; public class ScopeBase extends Sprite {}
            }''')
            (root / 'ScopeHelper.as').write_text('''package {
                public class ScopeHelper {
                    public static function accept(receiver:Object,value:Object):void { receiver.name=String(value); }
                    public static function take(value:Object):Boolean { return value==null; }
                }
            }''')
            source = root / 'fixture/ScopeProbe.as'
            source.write_text('''package fixture {
                public class ScopeProbe extends ScopeBase {
                    public function omit(value:Object):void { ScopeHelper.accept(this,value); }
                    public function defer(value:Object):void {
                        if(ScopeHelper.take(value)) return;
                        this.alpha=0.5;
                    }
                    public function value(input:Object):Boolean {
                        if(ScopeHelper.take(input)) return true;
                        return this.alpha>0.2;
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
                run('compile', reference, output, 'fixture.ScopeProbe', selected, root / (name + '-snapshot'),
                    'ffdec-source', sdk, name=name)
                return output

            emitted = compile_from(seed, source, 'emitted')
            cp = str(tools.work) + os.pathsep + config['ffdec']
            helper = Path(__file__).parent / 'fixtures/ScopeFixture.java'
            subprocess.run([config['javac'], '-cp', cp, '-d', str(root), str(helper)], check=True)
            java = [config['java'], '-cp', str(root) + os.pathsep + cp, 'ScopeFixture']
            unreserved_reference = root / 'unreserved-reference.swf'
            result = subprocess.run(java + [str(emitted), str(unreserved_reference)], stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout)
            prefix = 'QName(PackageNamespace("fixture"),"ScopeProbe")/instance/1/QName(PackageNamespace(""),'
            owners = {name: prefix + '"' + name + '")' for name in ('omit', 'defer')}
            plan = root / 'reservations.tsv'
            plan.write_bytes(source_patches.reservation_plan({'method_reservations': {
                owners['omit']: {'stack': 7, 'locals': 9}, owners['defer']: {'stack': 5, 'locals': 6},
            }}))

            def reserve(input_movie, name):
                output = root / (name + '.swf')
                run('reserve', input_movie, output, 'fixture.ScopeProbe', plan, root / (name + '-snapshot'), name=name)
                return output

            reference = reserve(unreserved_reference, 'reference')
            result = subprocess.run(java + ['check', str(unreserved_reference), str(reference)],
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout)
            raw_baseline = compile_from(reference, source, 'raw-baseline')
            baseline = reserve(raw_baseline, 'baseline')
            linked = root / 'linked.swf'
            run('link-layout', reference, baseline, linked, 'fixture.ScopeProbe', name='baseline-link')
            self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))

            # Minimum reservations never reduce a larger compiler requirement.
            minimum = root / 'minimum.tsv'
            minimum.write_text(owners['omit'] + '\t1\t1\n')
            unchanged = root / 'unchanged.swf'
            run('reserve', raw_baseline, unchanged, 'fixture.ScopeProbe', minimum, root / 'unchanged-snapshot', name='minimum')
            self.assertEqual(movie_source.uncompressed(raw_baseline), movie_source.uncompressed(unchanged))
            raw_hash = movie_source.digest(raw_baseline)
            for index, invalid in enumerate([
                '', owners['omit'] + '\t0\t0\n', owners['omit'] + '\t-1\t2\n',
                owners['omit'] + '\t65536\t2\n', owners['omit'] + '\t01\t2\n',
                (owners['omit'] + '\t3\t3\n') * 2, 'unknown\t3\t3\n',
            ]):
                bad = root / f'bad-{index}.tsv'
                bad.write_text(invalid)
                rejected = root / f'bad-{index}.swf'
                run('reserve', raw_baseline, rejected, 'fixture.ScopeProbe', bad, root / f'bad-{index}-snapshot',
                    name=f'bad-{index}', success=False)
                self.assertFalse(rejected.exists())
                self.assertEqual(movie_source.digest(raw_baseline), raw_hash)

            edited = root / 'edited.as'
            edited.write_text(source.read_text().replace('this.alpha=0.5', 'this.alpha=0.75'))
            raw_candidate = compile_from(reference, edited, 'raw-candidate')
            candidate = reserve(raw_candidate, 'candidate')
            rejected = root / 'rejected.swf'
            run('link-layout', reference, candidate, rejected, 'fixture.ScopeProbe', name='rejected', success=False)
            self.assertFalse(rejected.exists())
            feature = root / 'feature.swf'
            run('link-feature', reference, candidate, feature, 'fixture.ScopeProbe', name='feature')
            run('snapshot', feature, root / 'feature-records', name='feature-records')
            comparison = movie_source.compare(reference, feature, root / 'reference-snapshot', root / 'feature-records')
            self.assertEqual({r['owner'] for r in comparison['method_changes']}, {owners['defer']})
            assembly = '\n'.join(p.read_text() for p in (root / 'feature-records').glob('abc-*/method-*.asm'))
            self.assertIn('0.75', assembly)


if __name__ == '__main__':
    unittest.main()

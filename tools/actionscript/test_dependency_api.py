"""Explicit dependency declarations without changing dependency executable code."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

import movie_source
import source_patches


@unittest.skipUnless(os.environ.get('SRP_AS_TOOLS'), 'set SRP_AS_TOOLS to run real compiler integration')
class DependencyApiIntegration(unittest.TestCase):
    def test_hidden_static_declaration_preserves_dependencies_and_source_edits(self):
        with tempfile.TemporaryDirectory(prefix='srp-dependency-api-') as temp:
            root = Path(temp)
            config_path = Path(os.environ['SRP_AS_TOOLS']).resolve()
            config = json.loads(config_path.read_text())
            sdk = Path(config['flex'])
            tools = movie_source.Toolchain(config_path, root / 'bridge')
            dependency = root / 'Dependency.as'
            dependency.write_text('''package {
                public class Dependency {
                    public static var field:int=3;
                    public static function helper(value:int):int { return value+1; }
                    public static function other(value:int):int { return value-1; }
                }
            }''')
            source = root / 'ApiProbe.as'
            source.write_text('''package {
                import flash.display.Sprite;
                public class ApiProbe extends Sprite {
                    private var secret:int=7;
                    public function selected(value:int):int { return Dependency.helper(value); }
                    public function nativeValue(value:int):int { return Dependency.other(value)+secret; }
                    public function dynamicValue(value:Object):* { return value.x; }
                }
            }''')

            def compiler(jar, *args):
                result = subprocess.run([
                    config['java'], '-jar', str(sdk / 'lib' / jar), '+flexlib=' + str(sdk / 'frameworks'),
                    '-external-library-path=' + str(sdk / 'frameworks/libs/player/32.0/playerglobal.swc'),
                    '-library-path=', '-debug=false', '-optimize=false', *args,
                ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=90)
                self.assertEqual(result.returncode, 0, result.stdout)

            library = root / 'dependency.swc'
            compiler('compc.jar', '-source-path=' + str(root), '-include-classes=Dependency',
                     '-output=' + str(library))
            seed = root / 'seed.swf'
            compiler('mxmlc.jar', '-external-library-path+=' + str(library),
                     '-output=' + str(seed), str(source))
            with zipfile.ZipFile(library) as archive:
                library_movie = root / 'library.swf'
                library_movie.write_bytes(archive.read('library.swf'))
            header, tags = movie_source.split_movie(seed)
            dependency_tags = [tag for tag in movie_source.split_movie(library_movie)[1] if tag[0] == 82]
            self.assertTrue(dependency_tags)
            # The dependency is a separate embedded ABC, as in the real movie.
            seed.write_bytes(movie_source.join_movie(header, tags[:1] + dependency_tags + tags[1:]))
            owner = 'QName(PackageNamespace(""),"ApiProbe")/instance/1/QName(PackageNamespace(""),"selected")'
            entry = {'compiler': 'flex-mixed', 'composition': {
                'ffdec_compiler': 'ffdec-source', 'ffdec_methods': [owner], 'script_traits': 'ffdec',
                'ffdec_hidden_static': ['Dependency/helper']}}
            plan = root / 'composition.tsv'
            plan.write_bytes(source_patches.composition_plan(entry))

            def run(*args, name, success=True):
                log = root / (name + '.log')
                result = tools.run(*args, log=log)
                self.assertEqual(result.returncode == 0, success, log.read_text())
                return log.read_text()

            def compile_from(reference, text, name):
                path = root / (name + '.swf')
                run('compile', reference, path, 'ApiProbe', text, root / (name + '-snapshot'),
                    'flex-mixed', sdk, plan, name=name)
                retained = root / (name + '-snapshot-mixed')
                self.assertEqual((retained / 'composition.tsv').read_bytes(), plan.read_bytes())
                self.assertEqual((retained / 'source.as').read_bytes(), text.read_bytes())
                return path

            reference = compile_from(seed, source, 'reference')
            baseline = compile_from(reference, source, 'baseline')
            linked = root / 'linked.swf'
            run('link-layout', reference, baseline, linked, 'ApiProbe', name='baseline-link')
            self.assertEqual(movie_source.uncompressed(reference), movie_source.uncompressed(linked))
            assembly = '\n'.join(p.read_text() for p in (root / 'baseline-snapshot').glob('abc-*/method-*.asm'))
            self.assertIn('callproperty Multiname("helper"', assembly)
            self.assertIn('callproperty QName(PackageNamespace(""),"other")', assembly)
            # Composition itself verifies all non-target ABC bytes against seed.
            self.assertEqual(movie_source.split_movie(seed)[1][1:1 + len(dependency_tags)],
                             movie_source.split_movie(reference)[1][1:1 + len(dependency_tags)])

            edited = root / 'ApiProbe-edited.as'
            edited.write_text(source.read_text().replace('Dependency.helper(value)', 'Dependency.helper(value+2)'))
            candidate = compile_from(reference, edited, 'edited')
            rejected = root / 'rejected.swf'
            run('link-layout', reference, candidate, rejected, 'ApiProbe', name='edited-baseline', success=False)
            self.assertFalse(rejected.exists())
            permission = root / 'permission.tsv'
            permission.write_text(owner + '\t-\t-\n')
            feature = root / 'feature.swf'
            run('link-feature', reference, candidate, feature, 'ApiProbe', permission, name='feature-link')
            self.assertNotEqual(movie_source.uncompressed(reference), movie_source.uncompressed(feature))

            for number, member in enumerate(('Dependency/missing', 'Dependency/field', 'ApiProbe/selected')):
                invalid = root / f'invalid-{number}.tsv'
                invalid.write_text('ffdec_compiler=ffdec-source\nffdec_hide_static=' + member + '\n' + owner + '\n')
                output = root / f'invalid-{number}.swf'
                run('compile', seed, output, 'ApiProbe', source, root / f'invalid-{number}-snapshot',
                    'ffdec-source', sdk, invalid, name=f'invalid-{number}', success=False)
                self.assertFalse(output.exists())

            helper = root / 'DependencyFailure.java'
            helper.write_text('''import java.nio.file.*;
                import java.util.*;
                public class DependencyFailure {
                    public static void main(String[] args)throws Exception {
                        for(boolean badSource:List.of(true,false)) {
                            var movie=MovieSource.read(Path.of(args[0]));
                            var script=MovieSource.findScript(movie,"ApiProbe");
                            var dependency=MovieSource.findScript(movie,"Dependency").abc;
                            byte[] before=MovieSource.abcBytes(dependency);
                            var hidden=badSource?List.of("Dependency/helper"):
                                List.of("Dependency/helper","Dependency/missing");
                            boolean failed=false;
                            try {
                                new SourceCompiler(hidden).replaceScript(script,
                                    badSource?"package { public class ApiProbe { public function broken( }":
                                        Files.readString(Path.of(args[1])),
                                    List.of());
                            } catch(Exception expected) { failed=true; }
                            if(!failed || !Arrays.equals(before,MovieSource.abcBytes(dependency)))
                                throw new AssertionError("Compiler failure changed dependency declarations: badSource="
                                    +badSource+" failed="+failed+" before="+MovieSource.hash(before)
                                    +" after="+MovieSource.hash(MovieSource.abcBytes(dependency)));
                        }
                        System.exit(0);
                    }
                }''')
            cp = str(tools.work) + os.pathsep + config['ffdec']
            subprocess.run([config['javac'], '-cp', cp, '-d', str(root), str(helper)], check=True)
            result = subprocess.run([config['java'], '-cp', str(root) + os.pathsep + cp,
                                     'DependencyFailure', str(seed), str(source)],
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout)


if __name__ == '__main__':
    unittest.main()

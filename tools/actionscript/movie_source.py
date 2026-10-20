#!/usr/bin/env python3
"""Import private movie source and measure real compiler round trips.

No compiler output is approved using disassembly similarity. Exactness requires
equality of the complete uncompressed movie, including all ABC metadata/assets.
"""
import argparse
import csv
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import struct
import subprocess
import sys
import zlib

import source_patches

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def uncompressed(path):
    data = path.read_bytes()
    if data[:3] == b"CWS":
        stream = zlib.decompressobj()
        body = stream.decompress(data[8:]) + stream.flush()
        if not stream.eof or stream.unused_data:
            raise ValueError("Incomplete or trailing compressed SWF data")
        data = b"FWS" + data[3:8] + body
    elif data[:3] != b"FWS":
        raise ValueError("Expected FWS or CWS movie")
    if len(data) < 9 or len(data) != struct.unpack_from("<I", data, 4)[0]:
        raise ValueError("SWF declared length differs from actual length")
    return data


def split_movie(path):
    data = uncompressed(path)
    pos = 8 + (5 + 4 * (data[8] >> 3) + 7) // 8 + 4
    header = data[:pos]
    tags = []
    while pos < len(data):
        start = pos
        word, = struct.unpack_from("<H", data, pos)
        pos += 2
        kind, size = word >> 6, word & 63
        if size == 63:
            size, = struct.unpack_from("<I", data, pos)
            pos += 4
        pos += size
        if pos > len(data):
            raise ValueError("Truncated SWF tag")
        tags.append((kind, data[start:pos]))
        if kind == 0:
            if size or pos != len(data):
                raise ValueError("Bytes after SWF End")
            break
    if not tags or tags[-1][0] != 0:
        raise ValueError("Missing SWF End")
    return header, tags


def join_movie(header, tags):
    body = header[8:] + b"".join(raw for _, raw in tags)
    return b"FWS" + header[3:4] + struct.pack("<I", len(body) + 8) + body


def code_movie(original, output):
    header, tags = split_movie(original)
    output.write_bytes(join_movie(header, [t for t in tags if t[0] in (0, 69, 72, 82)]))


def transplant(original, compiled, output):
    header, tags = split_movie(original)
    _, changed = split_movie(compiled)
    old_abc = [kind for kind, _ in tags if kind in (72, 82)]
    new_abc = [t for t in changed if t[0] in (72, 82)]
    if old_abc != [kind for kind, _ in new_abc]:
        raise ValueError("Compiler changed the ABC block count or kinds")
    replacements = iter(new_abc)
    output.write_bytes(join_movie(header, [next(replacements) if kind in (72, 82) else (kind, raw) for kind, raw in tags]))


def methods(folder):
    with (folder / "methods.tsv").open() as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    result = {}
    for row in rows:
        key = (row["abc"], row["owner"])
        if key in result:
            raise ValueError(f"Ambiguous method identity {key}")
        result[key] = row
    return result


def deduplicate_snapshot(baseline, snapshot):
    """Share diagnostics only after comparing every file in an unchanged block."""
    for folder in snapshot.glob("abc-*"):
        reference = baseline / folder.name
        if not reference.is_dir():
            continue
        files = sorted(p.name for p in folder.iterdir())
        if files != sorted(p.name for p in reference.iterdir()):
            continue
        if not all((folder / name).read_bytes() == (reference / name).read_bytes() for name in files):
            continue
        shutil.rmtree(folder)
        folder.symlink_to(os.path.relpath(reference, folder.parent), target_is_directory=True)


def compare(reference, candidate, before, after):
    original, rebuilt = uncompressed(reference), uncompressed(candidate)
    a, b = methods(before), methods(after)
    changes = []
    for key in sorted(a.keys() | b.keys()):
        old, new = a.get(key), b.get(key)
        if old is None or new is None:
            changes.append({"abc": key[0], "owner": key[1], "status": "added" if old is None else "missing"})
            continue
        fields = [f for f in ("code_sha256", "record_sha256", "assembly_sha256") if old[f] != new[f]]
        if fields:
            changes.append({"abc": key[0], "owner": key[1], "status": "changed", "fields": fields,
                            "before_method": old["method"], "after_method": new["method"]})
    old_blocks = {p.parent.name: digest(p) for p in before.glob("abc-*/block.abc")}
    new_blocks = {p.parent.name: digest(p) for p in after.glob("abc-*/block.abc")}
    return {
        "file_exact": reference.read_bytes() == candidate.read_bytes(),
        "movie_exact": original == rebuilt,
        "reference_sha256": digest(reference), "candidate_sha256": digest(candidate),
        "reference_methods": len(a), "candidate_methods": len(b),
        "changed_abc_blocks": [k for k in sorted(old_blocks.keys() | new_blocks.keys()) if old_blocks.get(k) != new_blocks.get(k)],
        "method_changes": changes,
        "diagnostic_only": "Method hashes and resolved disassembly are not a proof of semantic equivalence. Unowned methods use numeric IDs; IDs may move.",
    }


class Toolchain:
    def __init__(self, config, work):
        self.config = json.loads(config.read_text())
        self.work = work
        self.work.mkdir(parents=True, exist_ok=True)
        self.env = dict(os.environ)
        self.env["PATH"] = str(Path(self.config["java"]).parent) + os.pathsep + self.env.get("PATH", "")
        self.cp = str(self.work) + os.pathsep + self.config["ffdec"]
        shutil.copyfile(HERE / "MovieSource.java", self.work / "MovieSource.java")
        shutil.copyfile(HERE / "MixedCompiler.java", self.work / "MixedCompiler.java")
        shutil.copyfile(HERE / "SourceCompiler.java", self.work / "SourceCompiler.java")
        shutil.copyfile(HERE / "CompilerReserves.java", self.work / "CompilerReserves.java")
        shutil.copyfile(HERE / "FeatureGate.java", self.work / "FeatureGate.java")
        shutil.copyfile(HERE / "movie_source.py", self.work / "movie_source.py")
        shutil.copyfile(HERE / "source_patches.py", self.work / "source_patches.py")
        subprocess.run([self.config["javac"], "-encoding", "UTF-8", "-cp", self.config["ffdec"],
                        "-d", str(self.work), str(self.work / "MovieSource.java"),
                        str(self.work / "MixedCompiler.java"), str(self.work / "SourceCompiler.java"),
                        str(self.work / "CompilerReserves.java"), str(self.work / "FeatureGate.java")],
                       check=True, env=self.env)

    def run(self, *args, log, timeout=600):
        with log.open("w") as stream:
            command = [self.config["java"], "-Xmx2g", "-Djava.awt.headless=true", "-cp", self.cp,
                       "MovieSource", *map(str, args)]
            with subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT, env=self.env,
                                  start_new_session=True) as process:
                try:
                    status = process.wait(timeout=timeout)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    raise
            return subprocess.CompletedProcess(command, status)

    def identity(self):
        jar = Path(self.config["ffdec"])
        inputs = {str(p.relative_to(jar.parent)): digest(p) for p in sorted(jar.parent.rglob("*.jar"))}
        inputs.update({str(p.relative_to(jar.parent)): digest(p) for p in sorted(jar.parent.rglob("*.swc"))})
        flex = Path(self.config.get("flex", "/nonexistent"))
        flex_inputs = {str(p.relative_to(flex)): digest(p) for p in sorted(flex.rglob("*"))
                       if p.is_file() and (p.suffix in (".jar", ".swc", ".xml", ".properties") or p.name == "mxmlc")}
        return {"ffdec_files": inputs, "flex_files": flex_inputs,
                "java_version": subprocess.check_output([self.config["java"], "-version"], stderr=subprocess.STDOUT, text=True),
                "python_version": sys.version, "zlib_version": zlib.ZLIB_RUNTIME_VERSION,
                "bridge_sha256": digest(self.work / "MovieSource.java"),
                "composition_bridge_sha256": digest(self.work / "MixedCompiler.java"),
                "source_compiler_sha256": digest(self.work / "SourceCompiler.java"),
                "compiler_reserves_sha256": digest(self.work / "CompilerReserves.java"),
                "feature_gate_sha256": digest(self.work / "FeatureGate.java"),
                "driver_sha256": digest(self.work / "movie_source.py"),
                "source_patches_sha256": digest(self.work / "source_patches.py")}


def init_project(movie, project, tools_config, release_name):
    """Pin a release and export all its ActionScript into `project` (a new directory)."""
    release_file = HERE / "releases" / (release_name + ".json")
    if release_file.parent != HERE / "releases" or not release_file.is_file():
        raise ValueError("Unknown release: add its verified movie and compiler hashes under tools/actionscript/releases first")
    release = json.loads(release_file.read_text())
    config = json.loads(tools_config.read_text())
    if digest(movie) != release["movie_sha256"]:
        raise ValueError("Movie does not match the pinned release")
    ffdec = Path(config["ffdec"])
    if digest(ffdec) != release["ffdec_sha256"] or digest(ffdec.parent / "lib/ffdec_lib.jar") != release["ffdec_lib_sha256"]:
        raise ValueError("Exporter does not match the release's pinned FFDec toolchain")
    project.mkdir(parents=True, exist_ok=False)
    tools = Toolchain(tools_config, project / "bridge")
    uncompressed(movie)
    shutil.copyfile(movie, project / "original.swf")
    code_movie(project / "original.swf", project / "code.swf")
    tools.run("export", project / "code.swf", project / "pristine", log=project / "export.log").check_returncode()
    tools.run("snapshot", project / "code.swf", project / "baseline", log=project / "snapshot.log").check_returncode()
    sources = {str(p.relative_to(project / "pristine")): digest(p) for p in sorted((project / "pristine").rglob("*.as"))}
    if not sources:
        raise ValueError("No ActionScript exported")
    write_json(project / "manifest.json", {"schema": 1, "release": release_name,
               "release_identity": release, "movie_sha256": digest(project / "original.swf"),
               "sources": sources, "toolchain": tools.identity()})
    print(f"Pinned {release_name}: {len(sources)} source files, {len(methods(project / 'baseline'))} method bodies", flush=True)


def verify_project(project):
    manifest = json.loads((project / "manifest.json").read_text())
    if digest(project / "original.swf") != manifest["movie_sha256"]:
        raise ValueError("Original movie differs from pinned release")
    actual = {str(p.relative_to(project / "pristine")): digest(p) for p in sorted((project / "pristine").rglob("*.as"))}
    if actual != manifest["sources"]:
        raise ValueError("Pristine sources differ from pinned export")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True, help="JSON with java, javac, ffdec and optional flex absolute paths")
    commands = parser.add_subparsers(dest="command", required=True)
    init = commands.add_parser("init", help="Pin a release and export all ActionScript into private storage")
    init.add_argument("movie", type=Path)
    init.add_argument("project", type=Path)
    init.add_argument("--release", required=True)
    measure = commands.add_parser("measure", help="Independently recompile each selected class from a fresh original")
    measure.add_argument("project", type=Path)
    measure.add_argument("output", type=Path)
    measure.add_argument("--compiler", choices=("ffdec", "ffdec-source", "flex", "flex-strict", "flex-library", "flex-mixed", "reconstruction"), required=True,
                         help="reconstruction uses pinned per-class compiler choices, defaulting to flex-strict")
    measure.add_argument("--classes", type=Path, help="One full script name per line; defaults to every exported script")
    measure.add_argument("--source-patches", type=Path, help="Pinned reconstruction manifest; pristine exports stay immutable")
    measure.add_argument("--jobs", type=int, default=1, help="Concurrent independent compiler processes (1-8)")
    measure.add_argument("--compact", action="store_true", help="Keep compiled code and comparison; reconstruct the full candidate with original.swf when needed")
    args = parser.parse_args()
    project = args.project.resolve()
    if args.command == "init":
        init_project(args.movie, project, args.tools, args.release)
        return 0
    manifest = verify_project(project)
    if args.compiler == "reconstruction" and not args.source_patches:
        raise ValueError("Reconstruction compilation requires a pinned source patch manifest")
    if not 1 <= args.jobs <= 8:
        raise ValueError("--jobs must be between 1 and 8")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    corrected, correction_evidence = {}, None
    if args.source_patches:
        corrected, correction_evidence = source_patches.load_sources(args.source_patches, project, manifest)
        correction_dir = output / "source-patches"
        correction_dir.mkdir()
        shutil.copyfile(args.source_patches, correction_dir / "manifest.json")
        for entry in correction_evidence["classes"]:
            destination = correction_dir / entry["patch"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(args.source_patches.parent / entry["patch"], destination)
        # Authenticate the saved copies too, before using any corrected input.
        saved, saved_evidence = source_patches.load_sources(correction_dir / "manifest.json", project, manifest)
        if saved != corrected or saved_evidence != correction_evidence:
            raise ValueError("Source patches changed while being recorded")
    tools = Toolchain(args.tools, output / "bridge")
    code_movie(project / "original.swf", output / "code.swf")
    # Recreate diagnostics from pinned bytes instead of trusting old reports.
    tools.run("snapshot", output / "code.swf", output / "baseline", log=output / "snapshot.log").check_returncode()
    tool_identity = tools.identity()
    # A changed compiler is a new experiment, recorded explicitly, never silently trusted.
    classes = args.classes.read_text().splitlines() if args.classes else [p[:-3].replace("/", ".") for p in manifest["sources"]]
    classes = [c.strip() for c in classes if c.strip() and not c.startswith("#")]
    if not classes or len(set(classes)) != len(classes):
        raise ValueError("Empty or duplicate class selection")
    for cls in classes:
        if cls.replace(".", "/") + ".as" not in manifest["sources"]:
            raise ValueError(f"Class not in pinned export: {cls}")
    report = {"schema": 1, "release": manifest["release"], "movie_sha256": manifest["movie_sha256"],
              "compiler": args.compiler, "toolchain": tool_identity, "classes": [], "complete": False,
              "selected_classes": classes, "jobs": args.jobs, "compact": args.compact,
              "source_patches": correction_evidence}
    write_json(output / "report.json", report)
    compiler_overrides = {entry["class"]: entry.get("compiler", "flex-strict")
                          for entry in (correction_evidence or {}).get("classes", [])}
    compiler_entries = {entry["class"]: entry
                        for entry in (correction_evidence or {}).get("classes", [])}

    def measure_class(number, cls):
        relative = cls.replace(".", "/") + ".as"
        folder = output / f"class-{number:04d}"
        folder.mkdir()
        compiler = compiler_overrides.get(cls, "flex-strict") if args.compiler == "reconstruction" else args.compiler
        row = {"class": cls, "source_sha256": manifest["sources"][relative], "directory": folder.name,
               "compiler": compiler}
        try:
            source = project / "pristine" / relative
            if digest(source) != manifest["sources"][relative]:
                raise ValueError("Source changed after project verification")
            if relative in corrected:
                source = folder / "source.as"
                source.write_bytes(corrected[relative])
                row["pristine_sha256"] = manifest["sources"][relative]
                row["source_sha256"] = digest(source)
            composition_args = []
            if compiler == "flex-mixed":
                plan = source_patches.composition_plan(compiler_entries.get(cls, {}))
                if plan is None:
                    raise ValueError("Mixed compilation requires an authenticated composition plan")
                plan_path = folder / "compiler-composition.tsv"
                plan_path.write_bytes(plan)
                composition_args = [plan_path]
                row["composition_sha256"] = digest(plan_path)
            reservation = source_patches.reservation_plan(compiler_entries.get(cls, {}))
            if reservation is not None:
                plan_path = folder / "method-reservations.tsv"
                plan_path.write_bytes(reservation)
                row["reservations_sha256"] = digest(plan_path)
            compiled_output = folder / ("unreserved-code.swf" if reservation is not None else "compiled-code.swf")
            compiled_snapshot = folder / ("unreserved-snapshot" if reservation is not None else "snapshot")
            result = tools.run("compile", output / "code.swf", compiled_output, cls,
                               source, compiled_snapshot, compiler,
                               tools.config.get("flex", ""), *composition_args, log=folder / "compile.log")
            if not result.returncode and reservation is not None:
                row["unreserved_code_sha256"] = digest(compiled_output)
                result = tools.run("reserve", compiled_output, folder / "compiled-code.swf", cls,
                                   plan_path, folder / "snapshot", log=folder / "reservation.log")
            if digest(source) != row["source_sha256"]:
                raise ValueError("Source changed during compilation")
            if result.returncode:
                row.update(status="compile_failed", exit_code=result.returncode)
            else:
                transplant(project / "original.swf", folder / "compiled-code.swf", folder / "candidate.swf")
                comparison = compare(project / "original.swf", folder / "candidate.swf", output / "baseline", folder / "snapshot")
                write_json(folder / "comparison.json", comparison)
                row.update(status="exact" if comparison["movie_exact"] else "different",
                           file_exact=comparison["file_exact"], changed_methods=len(comparison["method_changes"]),
                           changed_abc_blocks=comparison["changed_abc_blocks"])
                deduplicate_snapshot(output / "baseline", folder / "snapshot")
                if args.compact:
                    (folder / "candidate.swf").unlink()
        except subprocess.TimeoutExpired:
            row.update(status="timeout")
        except (ValueError, OSError) as error:
            row.update(status="tool_error", error=str(error))
        return row

    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        pending = [executor.submit(measure_class, number, cls) for number, cls in enumerate(classes)]
        for future in as_completed(pending):
            row = future.result()
            report["classes"].append(row)
            report["classes"].sort(key=lambda row: row["directory"])
            write_json(output / "report.json", report)
            print(f"{row['class']}: {row['status']} ({len(report['classes'])}/{len(classes)})", flush=True)
    report["complete"] = True
    report["all_exact"] = all(row["status"] == "exact" for row in report["classes"])
    write_json(output / "report.json", report)
    return 0 if report["all_exact"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))

#!/usr/bin/env python3
"""Assemble a complete baseline from every script's independently compiled records."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

import link_layout
import movie_source as movie
import source_patches


def indexed(rows, expected):
    result = {row["class"]: row for row in rows}
    if len(result) != len(rows) or set(result) != expected:
        raise ValueError("Report must cover every exported script exactly once")
    return result


def inputs(project, measurement, linked):
    pin = movie.verify_project(project)
    compile_bytes = (measurement / "report.json").read_bytes()
    compilation = json.loads(compile_bytes)
    linking = json.loads((linked / "report.json").read_text())
    if not (compilation["complete"] and linking["complete"]
            and linking["compilation_complete"] and linking["all_exact"]):
        raise ValueError("Complete, exact compilation/link reports are required")
    if (linked / "compilation-report.json").read_bytes() != compile_bytes or (
            linking["compilation_report_sha256"] != source_patches.sha(compile_bytes)):
        raise ValueError("Link report does not authenticate this compilation")
    if any(report["movie_sha256"] != pin["movie_sha256"] for report in (compilation, linking)):
        raise ValueError("Reports target a different release movie")
    if movie.digest(linked / "link_layout.py") != linking["link_driver_sha256"]:
        raise ValueError("Saved linking driver changed")
    expected = {path[:-3].replace("/", ".") for path in pin["sources"]}
    if len(expected) != len(pin["sources"]):
        raise ValueError("Ambiguous exported script paths")
    compiled = indexed(compilation["classes"], expected)
    linked_rows = indexed(linking["classes"], expected)
    if len({row["directory"] for row in compiled.values()}) != len(expected):
        raise ValueError("Scripts must have distinct compilation artifacts")
    if len(compilation["selected_classes"]) != len(expected) or set(compilation["selected_classes"]) != expected:
        raise ValueError("Compilation selection does not cover the whole tree")
    corrected, entries = {}, {}
    if compilation.get("source_patches") is not None:
        corrected, evidence = source_patches.load_sources(
            measurement / "source-patches/manifest.json", project, pin)
        if evidence != compilation["source_patches"]:
            raise ValueError("Reconstruction provenance changed")
        entries = {entry["class"]: entry for entry in evidence["classes"]}
    plan = []
    for name in sorted(expected):
        row, result = compiled[name], linked_rows[name]
        if row["status"] not in ("exact", "different") or result["status"] != "exact":
            raise ValueError("A script lacks successful compilation and exact linking")
        if not (result["file_exact"] and result["movie_exact"]
                and result["movie_sha256"] == pin["movie_sha256"]):
            raise ValueError("A script lacks complete baseline equality")
        relative = name.replace(".", "/") + ".as"
        source_hash = source_patches.sha(corrected[relative]) if relative in corrected else pin["sources"][relative]
        if row["source_sha256"] != source_hash or result["source_sha256"] != source_hash:
            raise ValueError("Compiled source identity changed")
        directory = row["directory"]
        if (directory in ("", ".", "..") or Path(directory).name != directory
                or result["directory"] != directory):
            raise ValueError("Invalid artifact directory")
        prior, artifact = measurement / directory, linked / directory / "linked-code.swf"
        if relative in corrected and (prior / "source.as").read_bytes() != corrected[relative]:
            raise ValueError("Compiled reconstruction source changed")
        link_layout.verify_compiler_plans(row, entries.get(name, {}), prior)
        if movie.digest(prior / "compiled-code.swf") != result["compiled_code_sha256"]:
            raise ValueError("Compiler artifact changed since linking")
        digest = movie.digest(artifact)
        if digest != result["linked_code_sha256"]:
            raise ValueError("Linked artifact changed")
        if any("\t" in value or "\n" in value for value in (name, str(artifact))):
            raise ValueError("Unsupported delimiter in assembly input")
        plan.append(f"{name}\t{artifact}\t{digest}\n")
    return pin, compilation, linking, "".join(plan)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("project", type=Path)
    parser.add_argument("measurement", type=Path)
    parser.add_argument("linked", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    project, measurement, linked, output = (
        path.resolve() for path in (args.project, args.measurement, args.linked, args.output))
    pin, compilation, linking, plan = inputs(project, measurement, linked)
    output.mkdir(parents=True, exist_ok=False)
    tools = movie.Toolchain(args.tools, output / "bridge")
    for filename in ("assemble_baseline.py", "AssembleBaseline.java"):
        shutil.copyfile(movie.HERE / filename, output / filename)
    subprocess.run([tools.config["javac"], "-encoding", "UTF-8", "-cp", tools.cp,
                    "-d", str(tools.work), str(output / "AssembleBaseline.java")],
                   check=True, env=tools.env)
    (output / "inputs.tsv").write_text(plan)
    movie.code_movie(project / "original.swf", output / "reference-code.swf")
    report = {
        "schema": 1, "complete": False, "release": pin["release"],
        "movie_sha256": pin["movie_sha256"], "scripts": len(pin["sources"]),
        "compilation_report_sha256": movie.digest(measurement / "report.json"),
        "link_report_sha256": movie.digest(linked / "report.json"),
        "assembly_driver_sha256": movie.digest(output / "assemble_baseline.py"),
        "assembly_bridge_sha256": movie.digest(output / "AssembleBaseline.java"),
        "inputs_sha256": movie.digest(output / "inputs.tsv"), "toolchain": tools.identity(),
    }
    movie.write_json(output / "report.json", report)
    with (output / "assembly.log").open("w") as log:
        subprocess.run([tools.config["java"], "-Xmx2g", "-Djava.awt.headless=true",
                        "-cp", tools.cp, "AssembleBaseline", str(output / "reference-code.swf"),
                        str(output / "inputs.tsv"), str(output / "assembled-code.swf"),
                        str(output / "coverage.tsv")],
                       stdout=log, stderr=subprocess.STDOUT, check=True, env=tools.env, timeout=1800)
    with (output / "coverage.tsv").open() as stream:
        coverage = list(csv.DictReader(stream, delimiter="\t"))
    indexed(coverage, {row["class"] for row in compilation["classes"]})
    movie.transplant(project / "original.swf", output / "assembled-code.swf", output / "rebuilt.swf")
    raw = (output / "rebuilt.swf").read_bytes()
    original = (project / "original.swf").read_bytes()
    profile = movie.serialization_profile(project / "original.swf")
    final = movie.serialize(raw, profile)
    if raw != movie.uncompressed(project / "original.swf") or final != original:
        raise ValueError("Assembled movie is not byte-identical to the complete release")
    (output / "rebuilt.swf").write_bytes(final)
    report.update(complete=True, file_exact=True, movie_exact=True,
                  serialization=profile,
                  rebuilt_sha256=hashlib.sha256(final).hexdigest(),
                  assembled_code_sha256=movie.digest(output / "assembled-code.swf"),
                  coverage_sha256=movie.digest(output / "coverage.tsv"),
                  abc_blocks=len({row["abc"] for row in coverage}),
                  method_signatures=sum(int(row["methods"]) for row in coverage),
                  method_bodies=sum(int(row["bodies"]) for row in coverage),
                  class_definitions=sum(int(row["classes"]) for row in coverage))
    movie.write_json(output / "report.json", report)
    print(f"Exact whole-tree rebuild: {report['scripts']} scripts, "
          f"{report['method_bodies']} compiled method bodies, SHA-256 {report['rebuilt_sha256']}")


if __name__ == "__main__":
    main()

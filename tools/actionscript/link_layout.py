#!/usr/bin/env python3
"""Try exact ABC layout linking on saved, independently compiled measurements.

The compiled input is authenticated against its recorded full-movie hash.
No source compilation is skipped or replaced with original method bodies.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import zlib

import movie_source as movie
import source_patches


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--compact", action="store_true", help="Keep linked code and hashes; omit full movie copies")
    parser.add_argument("project", type=Path)
    parser.add_argument("measurement", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        parser.error("--jobs must be between 1 and 8")
    project, measurement, output = (p.resolve() for p in (args.project, args.measurement, args.output))
    manifest = movie.verify_project(project)
    recorded = (measurement / "report.json").read_bytes()
    compiled = json.loads(recorded)
    if compiled["movie_sha256"] != manifest["movie_sha256"]:
        raise ValueError("Measurement targets a different movie")
    corrected = {}
    if compiled.get("source_patches") is not None:
        corrected, evidence = source_patches.load_sources(measurement / "source-patches" / "manifest.json", project, manifest)
        if evidence != compiled["source_patches"]:
            raise ValueError("Source patch provenance differs from measurement")
    output.mkdir(parents=True, exist_ok=False)
    (output / "compilation-report.json").write_bytes(recorded)
    shutil.copyfile(Path(__file__), output / "link_layout.py")
    tools = movie.Toolchain(args.tools, output / "bridge")
    movie.code_movie(project / "original.swf", output / "code.swf")
    original = (project / "original.swf").read_bytes()
    original_raw = movie.uncompressed(project / "original.swf")
    report = {"schema": 1, "movie_sha256": manifest["movie_sha256"],
              "link_driver_sha256": movie.digest(output / "link_layout.py"),
              "compilation_report_sha256": hashlib.sha256(recorded).hexdigest(),
              "compilation_complete": compiled["complete"], "linker": tools.identity(),
              "classes": [], "complete": False}
    movie.write_json(output / "report.json", report)

    def link(row):
        relative = row["class"].replace(".", "/") + ".as"
        expected_source = source_patches.sha(corrected[relative]) if relative in corrected else manifest["sources"].get(relative)
        if row["source_sha256"] != expected_source:
            raise ValueError("Measurement source differs from verified compile input")
        if relative in corrected:
            if row.get("pristine_sha256") != manifest["sources"][relative] or (measurement / row["directory"] / "source.as").read_bytes() != corrected[relative]:
                raise ValueError("Saved corrected source differs from pinned patch")
        folder = output / row["directory"]
        if folder.parent != output:
            raise ValueError("Invalid measurement directory")
        folder.mkdir()
        result = {"class": row["class"], "directory": folder.name, "source_sha256": row["source_sha256"]}
        if row["status"] not in ("different", "exact"):
            return dict(result, status="not_compiled", compilation_status=row["status"])
        prior = measurement / row["directory"]
        comparison = json.loads((prior / "comparison.json").read_text())
        # This reconstructs the measured FWS candidate, including all assets,
        # and verifies that the compiler artifact has not changed since then.
        movie.transplant(project / "original.swf", prior / "compiled-code.swf", folder / "measured.swf")
        if movie.digest(folder / "measured.swf") != comparison["candidate_sha256"]:
            raise ValueError("Compiled artifact differs from the measurement")
        (folder / "measured.swf").unlink()
        result["compiled_code_sha256"] = movie.digest(prior / "compiled-code.swf")
        try:
            process = tools.run("link-layout", output / "code.swf", prior / "compiled-code.swf",
                                folder / "linked-code.swf", row["class"], log=folder / "link.log")
            if process.returncode:
                return dict(result, status="different", exit_code=process.returncode)
            movie.transplant(project / "original.swf", folder / "linked-code.swf", folder / "linked.swf")
            raw = (folder / "linked.swf").read_bytes()
            # Deterministic serialization, not reuse of the original compressed
            # payload. The final file comparison independently proves equality.
            final = b"CWS" + raw[3:8] + zlib.compress(raw[8:], 9) if original[:3] == b"CWS" else raw
            (folder / "linked.swf").write_bytes(final)
            result.update(movie_exact=(raw == original_raw), file_exact=(final == original),
                          linked_code_sha256=movie.digest(folder / "linked-code.swf"),
                          movie_sha256=hashlib.sha256(final).hexdigest())
            result["status"] = "exact" if result["file_exact"] else "different"
            if args.compact:
                (folder / "linked.swf").unlink()
        except subprocess.TimeoutExpired:
            result["status"] = "timeout"
        return result

    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = [executor.submit(link, row) for row in compiled["classes"]]
        for future in as_completed(futures):
            row = future.result()
            report["classes"].append(row)
            report["classes"].sort(key=lambda item: item["directory"])
            movie.write_json(output / "report.json", report)
            print(f"{row['class']}: {row['status']} ({len(report['classes'])}/{len(futures)})", flush=True)
    report["complete"] = True
    report["all_exact"] = bool(report["classes"]) and compiled["complete"] and all(row["status"] == "exact" for row in report["classes"])
    movie.write_json(output / "report.json", report)
    return 0 if report["all_exact"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

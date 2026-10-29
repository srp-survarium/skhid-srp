Survarium v0.26g executable labels

Game and engine function names for inspecting and patching v0.26g build 2777.
Names come from comparison with the v0.21d EXE/PDB and from qualified names
in the v0.26g executable's own diagnostics. Vendor-library entries are included when needed to document a client patch.

Files (tab-separated, one header line; va is a preferred-image VA)
  functions.tsv    va, status, ref_021d, name   Reviewed names.
  inferred.tsv     va, status, ref_021d, name   Unreviewed names.
  abi.tsv          va, name, source, ret_cleanup, result
  abi_inputs.tsv   va, name, location, role, basis
  identity.json    Exact executable/PDB hashes, image bases and PE sections.
  lookup.py        Search by name or address; prints RVA, file offset and ABI.

status
  reviewed, verified  Checked by hand against the v0.21d body or matching vendor source.
  observed            A v0.26g diagnostic name, peer-reviewed against its body.
  inferred-high, inferred-medium
                      Translation-unit order and body likeness. Not reviewed
                      and without ABI rows; check the code before patching.
ref_021d is the matched v0.21d address; empty for observed names or vendor-source matches. An address
with several names, or a name at several addresses, has several rows.

ABI rows
  source       manual (hand analysis, per name) or auto (machine scan, name
               empty, applies to the whole body).
  ret_cleanup  Bytes popped by RET; several values if RETs differ; empty if
               not observed. A tail jump can move cleanup elsewhere.
  result       Return location and meaning, when observed.
  location     ECX, EAX, ...; "entry ESP+4" is the first stack slot above the
               return address.
  basis        observed (original data flow), inferred (from the matching
               v0.21d algorithm) or auto.
No row is a complete contract: clobbers, preservation and exceptions are not
established. LTO changes calling conventions, so use these locations, not a
source prototype. Body extents are not established either; a label is not a
safe patch length.

Usage (Python 3, from the repository root)
  python tools/re-kit/v026g/lookup.py stage_screen_image
  python tools/re-kit/v026g/lookup.py 'vostok::render::ui::renderer::draw_text' --exact
  python tools/re-kit/v026g/lookup.py 0x00379930 --runtime-base 0x400000
  python tools/re-kit/v026g/lookup.py draw_triangles --reviewed

RVA = va - 0x10000; runtime address = loaded module base + RVA. Supply
--runtime-base only when the loaded base is known. --reviewed leaves out
inferred.tsv.

Adding labels
New labels go in as rows. ../common/scripts/fold_inferred_labels.py adds a
function-map export (inferred.json, target-*-inferred.json and
target-observed.json) to these files. Evidence, reviews, low-confidence
candidates and audit history stay in private/function-map/. Reusable analysis
tools live in ../common/.

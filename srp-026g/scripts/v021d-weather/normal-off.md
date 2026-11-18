# Original normal-off compatibility

`v021d-vertex-blend-normal-off@3` supplies1218 native paths absent from stock and
the published patch stack.348 original/native contracts cover95 original pixel
programs, including the alpha-tested, detail and fuzziness combinations exposed
by the five-map load pilots. Every earlier482 path remains supported. The pack
is built by resource-porter `examples/vertex-blend-normal-off-021-pack.rs` from
`weather_fog_021/vertex_blend/normal-off-programs.json` and `normal_off.rs`.

The original pixel programs retain their normal, texture, alpha-test and finish
instructions.128 variable-alpha contracts receive a uniform branch at the
original second-texture interpolation coefficient. Only the existing explicit
negative masked-factor encoding selects the original masked equation. Original
linear materials retain every original instruction and interpolation result.
The branch uses the program's actual base texture, sampler, UV operand and
material-buffer slot; static liveness checks also cover parallax UVs. The native
falloff/encoded-factor binder lives at224, replacing the original unused blend
parameter descriptor at160. The existing500 native blend adapters still build
byte-identical after sharing this equation helper.

For the five constant-alpha vertex classes, original masked shaders compute
pow(saturate(factor+1),factor). All13 authored masked materials have factor in
[1,2], making the coefficient exactly1, equal to the original linear shader.
Ten pinned original programs and every authored factor verify this property.

The remaining ABI change is explicit:46 contracts read original scene_time at
396. Their shader operand, used reflection field and static-global buffer extent
are adapted to native offset400. Input semantic/register/component contracts,
output signatures, and every other used constant name/offset/size are checked
against pinned native counterparts. These are compatibility adaptations, not a
claim of byte-identical original shader execution.

`check_normal_off.py` reverses the bounded shader/reflection edits and requires
exact recovery of the original instructions. It verifies every composed alias,
base sampling, native isolation, constant-alpha proof and continued coverage of
all earlier paths. The1218 inverse checks,95 program disassemblies and affected
Clippy checks passed; native map loading and paired visual comparisons remain
required before acceptance.

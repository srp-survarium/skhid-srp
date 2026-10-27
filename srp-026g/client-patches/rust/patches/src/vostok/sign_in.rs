//! The sign-in (sign_in_on_connected 0x380800, client_patch.md section 6):
//! its 12-byte version field, "0.26g0", a zero, the patch revision, then
//! bytes 8..11, which the login server reads byte 8 of.
//!
//! At `sign_in.version_tail` (sites_026g.tsv) EDI is the field and EAX the
//! dword the replayed `mov [edi+8],eax` writes into bytes 8..11 (0 in the
//! original); 0x38088F loads EAX again before any use.

use super::context::{Context, Reg};

/// Byte 8 of the version field becomes `flags` (bytes 9..11 zero), through
/// the EAX the site's own write stores.
pub fn set_version_byte_8(ctx: &mut Context<'_>, flags: u8) {
    ctx.set_reg(Reg::Eax, u32::from(flags));
}

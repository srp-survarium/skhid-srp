//! Minimal PE32+ reader: headers, sections, static imports, delay imports, TLS directory.

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub va: u32,
    pub vsize: u32,
    pub raw_off: u32,
    pub raw_size: u32,
    pub characteristics: u32,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub dll: String,
    /// function name, or "#<ordinal>"
    pub name: String,
    /// RVA of the IAT slot
    pub slot_rva: u32,
    pub delay: bool,
}

#[derive(Debug, Clone)]
pub struct Tls {
    pub start: u64,
    pub end: u64,
    pub index_va: u64,
    pub callbacks_va: u64,
    pub zero_fill: u32,
}

#[derive(Debug, Clone)]
pub struct Pe {
    pub image_base: u64,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub entry_rva: u32,
    pub sections: Vec<Section>,
    pub imports: Vec<Import>,
    pub tls: Option<Tls>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

impl Pe {
    /// File offset of an RVA (None if it lies in no section's raw data).
    pub fn rva_to_off(&self, rva: u32) -> Option<usize> {
        if rva < self.size_of_headers {
            return Some(rva as usize);
        }
        for s in &self.sections {
            if rva >= s.va && rva < s.va + s.raw_size.max(s.vsize) {
                let d = rva - s.va;
                if d < s.raw_size {
                    return Some((s.raw_off + d) as usize);
                }
                return None;
            }
        }
        None
    }

    pub fn parse(b: &[u8]) -> Result<Pe, String> {
        if b.len() < 0x40 || &b[0..2] != b"MZ" {
            return Err("not MZ".into());
        }
        let nt = u32_at(b, 0x3c) as usize;
        if &b[nt..nt + 4] != b"PE\0\0" {
            return Err("no PE signature".into());
        }
        let fh = nt + 4;
        let nsec = u16_at(b, fh + 2) as usize;
        let opt_size = u16_at(b, fh + 16) as usize;
        let oh = fh + 20;
        if u16_at(b, oh) != 0x20b {
            return Err("not PE32+".into());
        }
        let entry_rva = u32_at(b, oh + 16);
        let image_base = u64_at(b, oh + 24);
        let size_of_image = u32_at(b, oh + 56);
        let size_of_headers = u32_at(b, oh + 60);
        let ndirs = u32_at(b, oh + 108) as usize;
        let dir = |i: usize| -> (u32, u32) {
            if i >= ndirs {
                return (0, 0);
            }
            let o = oh + 112 + i * 8;
            (u32_at(b, o), u32_at(b, o + 4))
        };
        let mut sections = Vec::new();
        let sh = oh + opt_size;
        for i in 0..nsec {
            let o = sh + i * 40;
            let raw_name = &b[o..o + 8];
            let end = raw_name.iter().position(|&c| c == 0).unwrap_or(8);
            sections.push(Section {
                name: String::from_utf8_lossy(&raw_name[..end]).into_owned(),
                vsize: u32_at(b, o + 8),
                va: u32_at(b, o + 12),
                raw_size: u32_at(b, o + 16),
                raw_off: u32_at(b, o + 20),
                characteristics: u32_at(b, o + 36),
            });
        }
        let mut pe = Pe {
            image_base,
            size_of_image,
            size_of_headers,
            entry_rva,
            sections,
            imports: Vec::new(),
            tls: None,
        };
        let cstr = |pe: &Pe, rva: u32| -> String {
            let o = pe.rva_to_off(rva).unwrap_or(0);
            let end = b[o..].iter().position(|&c| c == 0).unwrap_or(0);
            String::from_utf8_lossy(&b[o..o + end]).into_owned()
        };
        // static imports (directory 1)
        let (imp_rva, _) = dir(1);
        if imp_rva != 0 {
            let mut d = pe.rva_to_off(imp_rva).ok_or("import dir")?;
            loop {
                let oft = u32_at(b, d);
                let name_rva = u32_at(b, d + 12);
                let ft = u32_at(b, d + 16);
                if name_rva == 0 {
                    break;
                }
                let dll = cstr(&pe, name_rva);
                let lookup = if oft != 0 { oft } else { ft };
                let mut i = 0u32;
                loop {
                    let to = pe.rva_to_off(lookup + i * 8).ok_or("thunk")?;
                    let t = u64_at(b, to);
                    if t == 0 {
                        break;
                    }
                    let name = if t & (1 << 63) != 0 {
                        format!("#{}", t & 0xffff)
                    } else {
                        cstr(&pe, (t as u32) + 2)
                    };
                    pe.imports.push(Import { dll: dll.clone(), name, slot_rva: ft + i * 8, delay: false });
                    i += 1;
                }
                d += 20;
            }
        }
        // delay imports (directory 13), RVA-based descriptors (attributes bit 0)
        let (dl_rva, _) = dir(13);
        if dl_rva != 0 {
            let mut d = pe.rva_to_off(dl_rva).ok_or("delay dir")?;
            loop {
                let attrs = u32_at(b, d);
                let name_rva = u32_at(b, d + 4);
                let iat = u32_at(b, d + 12);
                let int = u32_at(b, d + 16);
                if name_rva == 0 {
                    break;
                }
                if attrs & 1 == 0 {
                    return Err("VA-based delay descriptor not supported".into());
                }
                let dll = cstr(&pe, name_rva);
                let mut i = 0u32;
                loop {
                    let to = pe.rva_to_off(int + i * 8).ok_or("delay INT")?;
                    let t = u64_at(b, to);
                    if t == 0 {
                        break;
                    }
                    let name = if t & (1 << 63) != 0 {
                        format!("#{}", t & 0xffff)
                    } else {
                        cstr(&pe, (t as u32) + 2)
                    };
                    pe.imports.push(Import { dll: dll.clone(), name, slot_rva: iat + i * 8, delay: true });
                    i += 1;
                }
                d += 32;
            }
        }
        // TLS (directory 9): VAs
        let (tls_rva, _) = dir(9);
        if tls_rva != 0 {
            let o = pe.rva_to_off(tls_rva).ok_or("tls dir")?;
            pe.tls = Some(Tls {
                start: u64_at(b, o),
                end: u64_at(b, o + 8),
                index_va: u64_at(b, o + 16),
                callbacks_va: u64_at(b, o + 24),
                zero_fill: u32_at(b, o + 32),
            });
        }
        Ok(pe)
    }
}

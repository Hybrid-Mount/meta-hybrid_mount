// SPDX-License-Identifier: GPL-3.0-only

//! In-memory ELF adaptation inspired by maxsteeel/lkmloader (GPL-3.0).
//! The packaged .ko is never rewritten. See THIRD_PARTY.md for the upstream revision.

use std::collections::BTreeMap;
use std::io::BufRead;

use object::{Object, ObjectSection, ObjectSymbol};

type Result<T> = std::result::Result<T, String>;

pub struct ModuleImage {
    pub bytes: Vec<u8>,
    pub name: String,
    pub vermagic: String,
    modinfo_header: usize,
    modinfo_alignment: usize,
    modinfo: Vec<u8>,
    undefined: Vec<(String, usize)>,
}

impl ModuleImage {
    pub fn parse(bytes: Vec<u8>) -> Result<Self> {
        let file = object::File::parse(bytes.as_slice()).map_err(|err| err.to_string())?;
        if file.format() != object::BinaryFormat::Elf
            || !file.is_64()
            || file.endianness() != object::Endianness::Little
            || file.architecture() != object::Architecture::Aarch64
            || file.kind() != object::ObjectKind::Relocatable
        {
            return Err(
                "LKM loader requires a little-endian AArch64 ELF64 relocatable module".into(),
            );
        }
        let section = file.section_by_name(".modinfo").ok_or("missing .modinfo")?;
        let modinfo_alignment = usize::try_from(section.align().max(1))
            .map_err(|_| "module metadata alignment is too large")?;
        if !modinfo_alignment.is_power_of_two() || modinfo_alignment > 64 * 1024 * 1024 {
            return Err("invalid module metadata alignment".into());
        }
        let modinfo = section.data().map_err(|err| err.to_string())?.to_vec();
        let field = |prefix: &[u8]| -> Result<String> {
            let value = modinfo
                .split(|byte| *byte == 0)
                .find_map(|entry| entry.strip_prefix(prefix))
                .ok_or("missing module metadata")?;
            String::from_utf8(value.to_vec()).map_err(|err| err.to_string())
        };
        let name = field(b"name=")?;
        let vermagic = field(b"vermagic=")?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
        {
            return Err("invalid module name".into());
        }
        let shoff = usize::try_from(u64::from_le_bytes(
            bytes
                .get(40..48)
                .ok_or("truncated ELF header")?
                .try_into()
                .map_err(|_| "invalid ELF header")?,
        ))
        .map_err(|_| "section offset is too large")?;
        if bytes.get(58..60) != Some(&64_u16.to_le_bytes()) {
            return Err("unsupported ELF section header size".into());
        }
        let modinfo_header = section
            .index()
            .0
            .checked_mul(64)
            .and_then(|n| shoff.checked_add(n))
            .ok_or("section header overflow")?;
        checked_range(&bytes, modinfo_header, 64)?;
        let symtab = file.section_by_name(".symtab").ok_or("missing .symtab")?;
        let (symoff, symsize) = symtab.file_range().ok_or("missing symbol data")?;
        let symoff = usize::try_from(symoff).map_err(|_| "symbol offset is too large")?;
        let symsize = usize::try_from(symsize).map_err(|_| "symbol table is too large")?;
        checked_range(&bytes, symoff, symsize)?;
        let mut undefined = Vec::new();
        for symbol in file.symbols().filter(|symbol| symbol.is_undefined()) {
            let name = symbol.name().map_err(|err| err.to_string())?;
            if name.is_empty() {
                continue;
            }
            let offset = symbol
                .index()
                .0
                .checked_mul(24)
                .ok_or("symbol offset overflow")?;
            if offset.checked_add(24).is_none_or(|end| end > symsize) {
                return Err("symbol outside .symtab".into());
            }
            undefined.push((name.to_owned(), symoff + offset));
        }
        Ok(Self {
            bytes,
            name,
            vermagic,
            modinfo_header,
            modinfo_alignment,
            modinfo,
            undefined,
        })
    }

    /// Only nonzero core-kernel addresses are usable: another LKM could be unloaded.
    /// Exact names win over compiler suffixes, and ambiguous names stay unresolved.
    pub fn resolve_symbols(&mut self, input: impl BufRead) -> Result<usize> {
        let mut addresses: BTreeMap<&str, Option<(u64, bool)>> = self
            .undefined
            .iter()
            .map(|(name, _)| (name.as_str(), None))
            .collect();
        for line in input.lines() {
            let line = line.map_err(|err| format!("read kallsyms: {err}"))?;
            let mut fields = line.split_whitespace();
            let (Some(address), Some(_kind), Some(name)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            if fields.next().is_some() {
                continue;
            }
            let Ok(address) = u64::from_str_radix(address, 16) else {
                continue;
            };
            if address == 0 {
                continue;
            }
            let canonical = name
                .split_once(".llvm.")
                .map(|(base, _)| base)
                .or_else(|| name.split_once('$').map(|(base, _)| base))
                .unwrap_or(name);
            let key = if addresses.contains_key(name) {
                name
            } else {
                canonical
            };
            let Some(slot) = addresses.get_mut(key) else {
                continue;
            };
            let exact = key == name;
            match *slot {
                None => *slot = Some((address, exact)),
                Some((_, false)) if exact => *slot = Some((address, true)),
                Some((previous, was_exact)) if was_exact == exact && previous != address => {
                    *slot = Some((0, exact));
                }
                _ => {}
            }
        }
        let mut resolved = 0;
        for (name, offset) in &self.undefined {
            if let Some((address, _)) = addresses.get(name.as_str()).copied().flatten() {
                if address == 0 {
                    continue;
                }
                self.bytes[*offset + 6..*offset + 8].copy_from_slice(&0xfff1_u16.to_le_bytes()); // SHN_ABS
                self.bytes[*offset + 8..*offset + 16].copy_from_slice(&address.to_le_bytes());
                resolved += 1;
            }
        }
        Ok(resolved)
    }

    pub fn patch_vermagic(&mut self, required: &str) -> Result<()> {
        if required.is_empty()
            || required.len() > 512
            || required.bytes().any(|ch| ch == 0 || ch == b'\n')
        {
            return Err("invalid requested vermagic".into());
        }
        let mut metadata = Vec::new();
        for entry in self
            .modinfo
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
        {
            if entry.starts_with(b"vermagic=") {
                continue;
            }
            metadata.extend_from_slice(entry);
            metadata.push(0);
        }
        metadata.extend_from_slice(b"vermagic=");
        metadata.extend_from_slice(required.as_bytes());
        metadata.push(0);
        let start = self
            .bytes
            .len()
            .checked_add(self.modinfo_alignment - 1)
            .ok_or("module size overflow")?
            & !(self.modinfo_alignment - 1);
        self.bytes.resize(start, 0);
        self.bytes.extend_from_slice(&metadata);
        self.bytes[self.modinfo_header + 24..self.modinfo_header + 32]
            .copy_from_slice(&(start as u64).to_le_bytes());
        self.bytes[self.modinfo_header + 32..self.modinfo_header + 40]
            .copy_from_slice(&(metadata.len() as u64).to_le_bytes());
        self.vermagic = required.to_owned();
        Ok(())
    }

    /// Bind the kernel message to both this module and this exact attempted vermagic.
    pub fn requested_vermagic(&self, record: &str) -> Option<String> {
        let prefix = format!(
            "{}: version magic '{}' should be '",
            self.name, self.vermagic
        );
        let message = record
            .split_once(';')
            .map_or(record, |(_, message)| message);
        let required = message
            .trim_start()
            .strip_prefix(&prefix)?
            .split_once('\'')?
            .0;
        (required != self.vermagic && !required.is_empty()).then(|| required.to_owned())
    }
}

fn checked_range(bytes: &[u8], offset: usize, size: usize) -> Result<&[u8]> {
    let end = offset.checked_add(size).ok_or("ELF range overflow")?;
    bytes
        .get(offset..end)
        .ok_or_else(|| "ELF section outside the module image".into())
}

/// A successful load is never repeated. Only an explicit vermagic rejection permits a retry.
pub fn load_with_vermagic_retry(
    image: &mut ModuleImage,
    mut insert: impl FnMut(&[u8]) -> std::io::Result<()>,
    mut requested: impl FnMut(&ModuleImage) -> Option<String>,
) -> Result<()> {
    match insert(&image.bytes) {
        Ok(()) => Ok(()),
        Err(err) if err.raw_os_error() == Some(8) => {
            // Linux ENOEXEC
            let required = requested(image).ok_or_else(|| {
                format!("module rejected without a matching vermagic diagnostic: {err}")
            })?;
            image.patch_vermagic(&required)?;
            log::info!(
                "retrying {} with kernel-requested vermagic {required}",
                image.name
            );
            insert(&image.bytes)
                .map_err(|err| format!("module still rejected after vermagic adaptation: {err}"))
        }
        Err(err) => Err(format!("insert module: {err}")),
    }
}

#[cfg(test)]
#[path = "lkm_image_tests.rs"]
mod tests;

//! Owned ELF hash table sections for symbol tables: `.hash`, and `.gnu.hash`
use crate::string_table::StringTable;
use crate::symbol::{Symbol, SymbolTable};
use crate::{make_option_type, make_result_type};
use anyhow::{anyhow, Result};
use elf::endian::EndianParse;
use elf::file::Class;
use elf::hash::GnuHashHeader;
use elf::hash::SysVHashHeader;
use elf::parse::ParseAt;
use std::mem::size_of;

/// The SysV hash function.
pub fn sysv_hash(name: &[u8]) -> u32 {
    elf::hash::sysv_hash(name)
}

/// The GNU hash function.
pub fn gnu_hash(name: &[u8]) -> u32 {
    elf::hash::gnu_hash(name)
}

/// An owned ELF SysV hash table (`.hash`), analogous to
/// `elf::hash::SysVHashTable` but holding owned data (`Vec<u32>`) rather than
/// slices for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SysVHashTable {
    buckets: Vec<u32>,
    chains: Vec<u32>,
}

impl SysVHashTable {
    /// Creates a new `SysVHashTable` by parsing section data.
    pub fn new<E: EndianParse>(endian: E, class: Class, data: &[u8]) -> Result<Self> {
        let mut offset = 0;
        let hdr = SysVHashHeader::parse_at(endian, class, &mut offset, data)?;

        let mut buckets = Vec::with_capacity(hdr.nbucket as usize);
        for _ in 0..hdr.nbucket {
            buckets.push(endian.parse_u32_at(&mut offset, data)?);
        }

        let mut chains = Vec::with_capacity(hdr.nchain as usize);
        for _ in 0..hdr.nchain {
            chains.push(endian.parse_u32_at(&mut offset, data)?);
        }

        Ok(SysVHashTable { buckets, chains })
    }

    /// Finds a symbol in the given symbol table and string table by name using
    /// the SysV hash table. Returns the symbol index and symbol if found.
    pub fn find(
        &self,
        name: &[u8],
        symtab: &SymbolTable,
        strtab: &StringTable,
    ) -> Result<Option<(usize, Symbol)>> {
        if self.buckets.is_empty() {
            return Ok(None);
        }

        let hash = sysv_hash(name);
        let bkt_idx = hash as usize % self.buckets.len();
        let mut index = *self.buckets.get(bkt_idx).ok_or_else(|| {
            anyhow!("Bucket index {} out of bounds in SysVHashTable::find", bkt_idx)
        })? as usize;

        // Bound the number of chain lookups by the chain size so we don't loop forever
        let mut i = 0;
        while index != 0 && i < self.chains.len() {
            let symbol = symtab
                .entries
                .get(index)
                .ok_or_else(|| {
                    anyhow!("Symbol index {} out of bounds in SysVHashTable::find", index)
                })?
                .clone();
            if strtab.get_slice(symbol.st_name as usize)? == name {
                return Ok(Some((index, symbol)));
            }
            index = *self.chains.get(index).ok_or_else(|| {
                anyhow!("Chain index {} out of bounds in SysVHashTable::find", index)
            })? as usize;
            i += 1;
        }

        Ok(None)
    }
}

make_option_type!(SysVHashTable);
make_result_type!(SysVHashTable);
make_result_type!(OptionSysVHashTable);

/// An owned ELF GNU hash table (`.gnu.hash`), analogous to
/// `elf::hash::GnuHashTable` but holding owned data (`Vec<u32>`, `Vec<u64>`)
/// rather than slices for easier FFI.
///
/// The upstream `elf::hash::GnuHashTable<'data, E>` is designed for
/// zero-copy parsing and holds references (`&'data [u8]`) to the
/// underlying ELF data, tying it to the `'data` lifetime. An owned, self-contained (`'static`)
/// version of the hash table is easier to manage than the upstream type across the FFI
/// boundary.
///
/// This struct fulfills that need by eagerly parsing the section data
/// and copying it into owned `Vec` collections, removing the lifetime
/// dependency. As such, its implementation of `new` and `find` closely
/// mirrors the logic in `elf::hash` but is adapted to work with owned
/// collections rather than lazily-parsed slices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnuHashTable {
    hdr: GnuHashHeader,
    bloom: Vec<u64>,
    buckets: Vec<u32>,
    chains: Vec<u32>,
    bloom_width: u32,
}

impl Default for GnuHashTable {
    fn default() -> Self {
        Self {
            hdr: GnuHashHeader { nbucket: 0, table_start_idx: 0, nbloom: 0, nshift: 0 },
            bloom: Vec::new(),
            buckets: Vec::new(),
            chains: Vec::new(),
            bloom_width: 0,
        }
    }
}

impl GnuHashTable {
    /// Creates a new `GnuHashTable` by parsing section data.
    pub fn new<E: EndianParse>(endian: E, class: Class, data: &[u8]) -> Result<Self> {
        let mut offset = 0;
        let hdr = GnuHashHeader::parse_at(endian, class, &mut offset, data)?;

        let nbloom = hdr.nbloom as usize;
        let mut bloom = Vec::with_capacity(nbloom);
        let bloom_width;
        match class {
            Class::ELF32 => {
                bloom_width = 8 * size_of::<u32>() as u32;
                for _ in 0..nbloom {
                    bloom.push(endian.parse_u32_at(&mut offset, data)? as u64);
                }
            }
            Class::ELF64 => {
                bloom_width = 8 * size_of::<u64>() as u32;
                for _ in 0..nbloom {
                    bloom.push(endian.parse_u64_at(&mut offset, data)?);
                }
            }
        }

        let mut buckets = Vec::with_capacity(hdr.nbucket as usize);
        for _ in 0..hdr.nbucket {
            buckets.push(endian.parse_u32_at(&mut offset, data)?);
        }

        let mut chains = Vec::new();
        while offset < data.len() {
            chains.push(endian.parse_u32_at(&mut offset, data)?);
        }

        Ok(GnuHashTable { hdr, bloom, buckets, chains, bloom_width })
    }

    /// Finds a symbol in the given symbol table and string table by name using
    /// the GNU hash table. Returns the symbol index and symbol if found.
    pub fn find(
        &self,
        name: &[u8],
        symtab: &SymbolTable,
        strtab: &StringTable,
    ) -> Result<Option<(usize, Symbol)>> {
        if self.buckets.is_empty() || self.hdr.nbloom == 0 {
            return Ok(None);
        }

        let hash = gnu_hash(name);

        // Test against bloom filter.
        let bloom_idx = (hash / self.bloom_width) % self.hdr.nbloom;
        let filter = *self.bloom.get(bloom_idx as usize).ok_or_else(|| {
            anyhow!("Bloom index {} out of bounds in GnuHashTable::find", bloom_idx)
        })?;

        if filter & (1 << (hash % self.bloom_width)) == 0 {
            return Ok(None);
        }
        let hash2 = hash.checked_shr(self.hdr.nshift).ok_or_else(|| anyhow!("Bad shift"))?;
        if filter & (1 << (hash2 % self.bloom_width)) == 0 {
            return Ok(None);
        }

        let table_start_idx = self.hdr.table_start_idx as usize;
        let bucket_idx = (hash as usize) % self.buckets.len();
        let chain_start_idx = *self.buckets.get(bucket_idx).ok_or_else(|| {
            anyhow!("Bucket index {} out of bounds in GnuHashTable::find", bucket_idx)
        })? as usize;
        if chain_start_idx < table_start_idx {
            return Ok(None);
        }

        for chain_idx in (chain_start_idx - table_start_idx)..self.chains.len() {
            let chain_hash = *self.chains.get(chain_idx).ok_or_else(|| {
                anyhow!("Chain index {} out of bounds in GnuHashTable::find", chain_idx)
            })?;

            if hash | 1 == chain_hash | 1 {
                let sym_idx = chain_idx.checked_add(table_start_idx).ok_or_else(|| {
                    anyhow!("Integer overflow calculating symbol index in GnuHashTable::find")
                })?;
                let symbol = symtab
                    .entries
                    .get(sym_idx)
                    .ok_or_else(|| {
                        anyhow!("Symbol index {} out of bounds in GnuHashTable::find", sym_idx)
                    })?
                    .clone();
                if strtab.get_slice(symbol.st_name as usize)? == name {
                    return Ok(Some((sym_idx, symbol)));
                }
            }

            if chain_hash & 1 != 0 {
                break;
            }
        }

        Ok(None)
    }
}

make_option_type!(GnuHashTable);
make_result_type!(GnuHashTable);
make_result_type!(OptionGnuHashTable);

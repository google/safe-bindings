//! Owned wrapper for ELF string table sections.
use crate::{make_option_type, make_result_type};
use anyhow::{anyhow, Result};

/// An owned ELF string table, analogous to `elf::string_table::StringTable`
/// but holding owned data (`Vec<u8>`) rather than a slice for easier FFI.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct StringTable {
    data: Vec<u8>,
}

impl StringTable {
    /// Creates a new StringTable by cloning the provided data slice.
    ///
    /// # Arguments
    ///
    /// * `data` - The raw byte slice containing the string table data.
    pub fn new(data: &[u8]) -> Self {
        StringTable { data: data.to_vec() }
    }

    /// Internal helper to get a slice from the buffer corresponding to the
    /// string at `offset`.
    ///
    /// The returned slice does *not* include the NUL terminator.
    ///
    /// # Arguments
    ///
    /// * `offset` - The byte offset within the string table where the string begins.
    ///
    /// # Returns
    ///
    /// A `Result` containing a byte slice (`&[u8]`) if successful, or an `anyhow::Error`
    /// if the offset is out of bounds, the table is empty, or no NUL terminator is found.
    pub(crate) fn get_slice(&self, offset: usize) -> Result<&[u8]> {
        if self.data.is_empty() {
            return Err(anyhow!("StringTable: Bad offset {offset} in empty table"));
        }
        let start =
            self.data.get(offset..).ok_or_else(|| anyhow!("StringTable: Bad offset {offset}"))?;
        let end = start
            .iter()
            .position(|&b| b == 0u8)
            .ok_or_else(|| anyhow!("StringTable: Missing NUL terminator for offset {offset}"))?;
        Ok(&start[..end])
    }

    /// Returns an owned copy of the string bytes at `offset`.
    pub fn get(&self, offset: usize) -> Result<Vec<u8>> {
        self.get_slice(offset).map(|s| s.to_vec())
    }
}

// Makes `Option<StringTable>` available as `OptionStringTable` for FFI.
make_option_type!(StringTable);
// Makes `Result<StringTable>` available as `ResultStringTable` for FFI.
make_result_type!(StringTable);
// Makes `Result<Option<StringTable>>` available as `ResultOptionStringTable` for FFI.
make_result_type!(OptionStringTable);

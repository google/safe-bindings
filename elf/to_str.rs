use crate::constants::{
    DynamicTag, ElfMachine, ProgramHeaderType, SectionType, SymbolBind, SymbolType,
    SymbolVisibility,
};
pub fn d_tag_to_str(tag: DynamicTag) -> &'static str {
    elf::to_str::d_tag_to_str(tag.0).unwrap_or("Unknown")
}

pub fn sh_type_to_str(sh_type: SectionType) -> &'static str {
    elf::to_str::sh_type_to_str(sh_type.0).unwrap_or("Unknown")
}

pub fn e_machine_to_str(e_machine: ElfMachine) -> &'static str {
    elf::to_str::e_machine_to_str(e_machine.0 as u16).unwrap_or("Unknown")
}

pub fn e_machine_to_human_str(e_machine: ElfMachine) -> &'static str {
    elf::to_str::e_machine_to_human_str(e_machine.0 as u16).unwrap_or("Unknown")
}

pub fn p_type_to_str(p_type: ProgramHeaderType) -> &'static str {
    elf::to_str::p_type_to_str(p_type.0).unwrap_or("Unknown")
}

pub fn st_symtype_to_str(st_symtype: SymbolType) -> &'static str {
    elf::to_str::st_symtype_to_str(st_symtype.0 as u8).unwrap_or("Unknown")
}

pub fn st_bind_to_str(st_bind: SymbolBind) -> &'static str {
    elf::to_str::st_bind_to_str(st_bind.0 as u8).unwrap_or("Unknown")
}

pub fn st_vis_to_str(st_vis: SymbolVisibility) -> &'static str {
    elf::to_str::st_vis_to_str(st_vis.0 as u8).unwrap_or("Unknown")
}

pub fn e_osabi_to_string(osabi: u8) -> String {
    elf::to_str::e_osabi_to_string(osabi)
}

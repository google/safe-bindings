use crate::make_result_type;

use anyhow::{anyhow, Result};
use crubit_annotate::cpp_enum;
use elf::abi;
use std::fmt;

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct ElfClass(i32);

#[allow(non_upper_case_globals)]
impl ElfClass {
    pub const kElf32: ElfClass = ElfClass(0);
    pub const kElf64: ElfClass = ElfClass(1);
}
impl From<elf::file::Class> for ElfClass {
    fn from(v: elf::file::Class) -> Self {
        match v {
            elf::file::Class::ELF32 => ElfClass::kElf32,
            elf::file::Class::ELF64 => ElfClass::kElf64,
        }
    }
}

make_result_type!(ElfClass);

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct AnyEndian(i32);

#[allow(non_upper_case_globals)]
impl AnyEndian {
    pub const kLittle: AnyEndian = AnyEndian(0);
    pub const kBig: AnyEndian = AnyEndian(1);
}
impl From<elf::endian::AnyEndian> for AnyEndian {
    fn from(v: elf::endian::AnyEndian) -> Self {
        match v {
            elf::endian::AnyEndian::Little => AnyEndian::kLittle,
            elf::endian::AnyEndian::Big => AnyEndian::kBig,
        }
    }
}

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct ElfMachine(pub i32);

#[allow(non_upper_case_globals)]
impl ElfMachine {
    pub const kEmNone: ElfMachine = ElfMachine(abi::EM_NONE as i32);
    pub const kEmM32: ElfMachine = ElfMachine(abi::EM_M32 as i32);
    pub const kEmSparc: ElfMachine = ElfMachine(abi::EM_SPARC as i32);
    pub const kEm386: ElfMachine = ElfMachine(abi::EM_386 as i32);
    pub const kEm68k: ElfMachine = ElfMachine(abi::EM_68K as i32);
    pub const kEm88k: ElfMachine = ElfMachine(abi::EM_88K as i32);
    pub const kEmIamcu: ElfMachine = ElfMachine(abi::EM_IAMCU as i32);
    pub const kEm860: ElfMachine = ElfMachine(abi::EM_860 as i32);
    pub const kEmMips: ElfMachine = ElfMachine(abi::EM_MIPS as i32);
    pub const kEmS370: ElfMachine = ElfMachine(abi::EM_S370 as i32);
    pub const kEmMipsRs3Le: ElfMachine = ElfMachine(abi::EM_MIPS_RS3_LE as i32);
    pub const kEmParisc: ElfMachine = ElfMachine(abi::EM_PARISC as i32);
    pub const kEmVpp500: ElfMachine = ElfMachine(abi::EM_VPP500 as i32);
    pub const kEmSparc32plus: ElfMachine = ElfMachine(abi::EM_SPARC32PLUS as i32);
    pub const kEm960: ElfMachine = ElfMachine(abi::EM_960 as i32);
    pub const kEmPpc: ElfMachine = ElfMachine(abi::EM_PPC as i32);
    pub const kEmPpc64: ElfMachine = ElfMachine(abi::EM_PPC64 as i32);
    pub const kEmS390: ElfMachine = ElfMachine(abi::EM_S390 as i32);
    pub const kEmSpu: ElfMachine = ElfMachine(abi::EM_SPU as i32);
    pub const kEmV800: ElfMachine = ElfMachine(abi::EM_V800 as i32);
    pub const kEmFr20: ElfMachine = ElfMachine(abi::EM_FR20 as i32);
    pub const kEmRh32: ElfMachine = ElfMachine(abi::EM_RH32 as i32);
    pub const kEmRce: ElfMachine = ElfMachine(abi::EM_RCE as i32);
    pub const kEmArm: ElfMachine = ElfMachine(abi::EM_ARM as i32);
    pub const kEmAlpha: ElfMachine = ElfMachine(abi::EM_ALPHA as i32);
    pub const kEmSh: ElfMachine = ElfMachine(abi::EM_SH as i32);
    pub const kEmSparcv9: ElfMachine = ElfMachine(abi::EM_SPARCV9 as i32);
    pub const kEmTricore: ElfMachine = ElfMachine(abi::EM_TRICORE as i32);
    pub const kEmArc: ElfMachine = ElfMachine(abi::EM_ARC as i32);
    pub const kEmH8300: ElfMachine = ElfMachine(abi::EM_H8_300 as i32);
    pub const kEmH8300h: ElfMachine = ElfMachine(abi::EM_H8_300H as i32);
    pub const kEmH8s: ElfMachine = ElfMachine(abi::EM_H8S as i32);
    pub const kEmH8500: ElfMachine = ElfMachine(abi::EM_H8_500 as i32);
    pub const kEmIa64: ElfMachine = ElfMachine(abi::EM_IA_64 as i32);
    pub const kEmMipsX: ElfMachine = ElfMachine(abi::EM_MIPS_X as i32);
    pub const kEmColdfire: ElfMachine = ElfMachine(abi::EM_COLDFIRE as i32);
    pub const kEm68hc12: ElfMachine = ElfMachine(abi::EM_68HC12 as i32);
    pub const kEmMma: ElfMachine = ElfMachine(abi::EM_MMA as i32);
    pub const kEmPcp: ElfMachine = ElfMachine(abi::EM_PCP as i32);
    pub const kEmNcpu: ElfMachine = ElfMachine(abi::EM_NCPU as i32);
    pub const kEmNdr1: ElfMachine = ElfMachine(abi::EM_NDR1 as i32);
    pub const kEmStarcore: ElfMachine = ElfMachine(abi::EM_STARCORE as i32);
    pub const kEmMe16: ElfMachine = ElfMachine(abi::EM_ME16 as i32);
    pub const kEmSt100: ElfMachine = ElfMachine(abi::EM_ST100 as i32);
    pub const kEmTinyj: ElfMachine = ElfMachine(abi::EM_TINYJ as i32);
    pub const kEmX8664: ElfMachine = ElfMachine(abi::EM_X86_64 as i32);
    pub const kEmPdsp: ElfMachine = ElfMachine(abi::EM_PDSP as i32);
    pub const kEmPdp10: ElfMachine = ElfMachine(abi::EM_PDP10 as i32);
    pub const kEmPdp11: ElfMachine = ElfMachine(abi::EM_PDP11 as i32);
    pub const kEmFx66: ElfMachine = ElfMachine(abi::EM_FX66 as i32);
    pub const kEmSt9plus: ElfMachine = ElfMachine(abi::EM_ST9PLUS as i32);
    pub const kEmSt7: ElfMachine = ElfMachine(abi::EM_ST7 as i32);
    pub const kEm68hc16: ElfMachine = ElfMachine(abi::EM_68HC16 as i32);
    pub const kEm68hc11: ElfMachine = ElfMachine(abi::EM_68HC11 as i32);
    pub const kEm68hc08: ElfMachine = ElfMachine(abi::EM_68HC08 as i32);
    pub const kEm68hc05: ElfMachine = ElfMachine(abi::EM_68HC05 as i32);
    pub const kEmSvx: ElfMachine = ElfMachine(abi::EM_SVX as i32);
    pub const kEmSt19: ElfMachine = ElfMachine(abi::EM_ST19 as i32);
    pub const kEmVax: ElfMachine = ElfMachine(abi::EM_VAX as i32);
    pub const kEmCris: ElfMachine = ElfMachine(abi::EM_CRIS as i32);
    pub const kEmJavelin: ElfMachine = ElfMachine(abi::EM_JAVELIN as i32);
    pub const kEmFirepath: ElfMachine = ElfMachine(abi::EM_FIREPATH as i32);
    pub const kEmZsp: ElfMachine = ElfMachine(abi::EM_ZSP as i32);
    pub const kEmMmix: ElfMachine = ElfMachine(abi::EM_MMIX as i32);
    pub const kEmHuany: ElfMachine = ElfMachine(abi::EM_HUANY as i32);
    pub const kEmPrism: ElfMachine = ElfMachine(abi::EM_PRISM as i32);
    pub const kEmAvr: ElfMachine = ElfMachine(abi::EM_AVR as i32);
    pub const kEmFr30: ElfMachine = ElfMachine(abi::EM_FR30 as i32);
    pub const kEmD10v: ElfMachine = ElfMachine(abi::EM_D10V as i32);
    pub const kEmD30v: ElfMachine = ElfMachine(abi::EM_D30V as i32);
    pub const kEmV850: ElfMachine = ElfMachine(abi::EM_V850 as i32);
    pub const kEmM32r: ElfMachine = ElfMachine(abi::EM_M32R as i32);
    pub const kEmMn10300: ElfMachine = ElfMachine(abi::EM_MN10300 as i32);
    pub const kEmMn10200: ElfMachine = ElfMachine(abi::EM_MN10200 as i32);
    pub const kEmPj: ElfMachine = ElfMachine(abi::EM_PJ as i32);
    pub const kEmOpenrisc: ElfMachine = ElfMachine(abi::EM_OPENRISC as i32);
    pub const kEmArcCompact: ElfMachine = ElfMachine(abi::EM_ARC_COMPACT as i32);
    pub const kEmXtensa: ElfMachine = ElfMachine(abi::EM_XTENSA as i32);
    pub const kEmVideocore: ElfMachine = ElfMachine(abi::EM_VIDEOCORE as i32);
    pub const kEmTmmGpp: ElfMachine = ElfMachine(abi::EM_TMM_GPP as i32);
    pub const kEmNs32k: ElfMachine = ElfMachine(abi::EM_NS32K as i32);
    pub const kEmTpc: ElfMachine = ElfMachine(abi::EM_TPC as i32);
    pub const kEmSnp1k: ElfMachine = ElfMachine(abi::EM_SNP1K as i32);
    pub const kEmSt200: ElfMachine = ElfMachine(abi::EM_ST200 as i32);
    pub const kEmIp2k: ElfMachine = ElfMachine(abi::EM_IP2K as i32);
    pub const kEmMax: ElfMachine = ElfMachine(abi::EM_MAX as i32);
    pub const kEmCr: ElfMachine = ElfMachine(abi::EM_CR as i32);
    pub const kEmF2mc16: ElfMachine = ElfMachine(abi::EM_F2MC16 as i32);
    pub const kEmMsp430: ElfMachine = ElfMachine(abi::EM_MSP430 as i32);
    pub const kEmBlackfin: ElfMachine = ElfMachine(abi::EM_BLACKFIN as i32);
    pub const kEmSeC33: ElfMachine = ElfMachine(abi::EM_SE_C33 as i32);
    pub const kEmSep: ElfMachine = ElfMachine(abi::EM_SEP as i32);
    pub const kEmArca: ElfMachine = ElfMachine(abi::EM_ARCA as i32);
    pub const kEmUnicore: ElfMachine = ElfMachine(abi::EM_UNICORE as i32);
    pub const kEmExcess: ElfMachine = ElfMachine(abi::EM_EXCESS as i32);
    pub const kEmDxp: ElfMachine = ElfMachine(abi::EM_DXP as i32);
    pub const kEmAlteraNios2: ElfMachine = ElfMachine(abi::EM_ALTERA_NIOS2 as i32);
    pub const kEmCrx: ElfMachine = ElfMachine(abi::EM_CRX as i32);
    pub const kEmXgate: ElfMachine = ElfMachine(abi::EM_XGATE as i32);
    pub const kEmC166: ElfMachine = ElfMachine(abi::EM_C166 as i32);
    pub const kEmM16c: ElfMachine = ElfMachine(abi::EM_M16C as i32);
    pub const kEmDspic30f: ElfMachine = ElfMachine(abi::EM_DSPIC30F as i32);
    pub const kEmCe: ElfMachine = ElfMachine(abi::EM_CE as i32);
    pub const kEmM32c: ElfMachine = ElfMachine(abi::EM_M32C as i32);
    pub const kEmTsk3000: ElfMachine = ElfMachine(abi::EM_TSK3000 as i32);
    pub const kEmRs08: ElfMachine = ElfMachine(abi::EM_RS08 as i32);
    pub const kEmSharc: ElfMachine = ElfMachine(abi::EM_SHARC as i32);
    pub const kEmEcog2: ElfMachine = ElfMachine(abi::EM_ECOG2 as i32);
    pub const kEmScore7: ElfMachine = ElfMachine(abi::EM_SCORE7 as i32);
    pub const kEmDsp24: ElfMachine = ElfMachine(abi::EM_DSP24 as i32);
    pub const kEmVideocore3: ElfMachine = ElfMachine(abi::EM_VIDEOCORE3 as i32);
    pub const kEmLatticemico32: ElfMachine = ElfMachine(abi::EM_LATTICEMICO32 as i32);
    pub const kEmSeC17: ElfMachine = ElfMachine(abi::EM_SE_C17 as i32);
    pub const kEmTiC6000: ElfMachine = ElfMachine(abi::EM_TI_C6000 as i32);
    pub const kEmTiC2000: ElfMachine = ElfMachine(abi::EM_TI_C2000 as i32);
    pub const kEmTiC5500: ElfMachine = ElfMachine(abi::EM_TI_C5500 as i32);
    pub const kEmTiArp32: ElfMachine = ElfMachine(abi::EM_TI_ARP32 as i32);
    pub const kEmTiPru: ElfMachine = ElfMachine(abi::EM_TI_PRU as i32);
    pub const kEmMmdspPlus: ElfMachine = ElfMachine(abi::EM_MMDSP_PLUS as i32);
    pub const kEmCypressM8c: ElfMachine = ElfMachine(abi::EM_CYPRESS_M8C as i32);
    pub const kEmR32c: ElfMachine = ElfMachine(abi::EM_R32C as i32);
    pub const kEmTrimedia: ElfMachine = ElfMachine(abi::EM_TRIMEDIA as i32);
    pub const kEmQdsp6: ElfMachine = ElfMachine(abi::EM_QDSP6 as i32);
    pub const kEm8051: ElfMachine = ElfMachine(abi::EM_8051 as i32);
    pub const kEmStxp7x: ElfMachine = ElfMachine(abi::EM_STXP7X as i32);
    pub const kEmNds32: ElfMachine = ElfMachine(abi::EM_NDS32 as i32);
    pub const kEmEcog1x: ElfMachine = ElfMachine(abi::EM_ECOG1X as i32);
    pub const kEmMaxq30: ElfMachine = ElfMachine(abi::EM_MAXQ30 as i32);
    pub const kEmXimo16: ElfMachine = ElfMachine(abi::EM_XIMO16 as i32);
    pub const kEmManik: ElfMachine = ElfMachine(abi::EM_MANIK as i32);
    pub const kEmCraynv2: ElfMachine = ElfMachine(abi::EM_CRAYNV2 as i32);
    pub const kEmRx: ElfMachine = ElfMachine(abi::EM_RX as i32);
    pub const kEmMetag: ElfMachine = ElfMachine(abi::EM_METAG as i32);
    pub const kEmMcstElbrus: ElfMachine = ElfMachine(abi::EM_MCST_ELBRUS as i32);
    pub const kEmEcog16: ElfMachine = ElfMachine(abi::EM_ECOG16 as i32);
    pub const kEmCr16: ElfMachine = ElfMachine(abi::EM_CR16 as i32);
    pub const kEmEtpu: ElfMachine = ElfMachine(abi::EM_ETPU as i32);
    pub const kEmSle9x: ElfMachine = ElfMachine(abi::EM_SLE9X as i32);
    pub const kEmL10m: ElfMachine = ElfMachine(abi::EM_L10M as i32);
    pub const kEmK10m: ElfMachine = ElfMachine(abi::EM_K10M as i32);
    pub const kEmAarch64: ElfMachine = ElfMachine(abi::EM_AARCH64 as i32);
    pub const kEmAvr32: ElfMachine = ElfMachine(abi::EM_AVR32 as i32);
    pub const kEmStm8: ElfMachine = ElfMachine(abi::EM_STM8 as i32);
    pub const kEmTile64: ElfMachine = ElfMachine(abi::EM_TILE64 as i32);
    pub const kEmTilepro: ElfMachine = ElfMachine(abi::EM_TILEPRO as i32);
    pub const kEmMicroblaze: ElfMachine = ElfMachine(abi::EM_MICROBLAZE as i32);
    pub const kEmCuda: ElfMachine = ElfMachine(abi::EM_CUDA as i32);
    pub const kEmTilegx: ElfMachine = ElfMachine(abi::EM_TILEGX as i32);
    pub const kEmCloudshield: ElfMachine = ElfMachine(abi::EM_CLOUDSHIELD as i32);
    pub const kEmCorea1st: ElfMachine = ElfMachine(abi::EM_COREA_1ST as i32);
    pub const kEmCorea2nd: ElfMachine = ElfMachine(abi::EM_COREA_2ND as i32);
    pub const kEmArcCompact2: ElfMachine = ElfMachine(abi::EM_ARC_COMPACT2 as i32);
    pub const kEmOpen8: ElfMachine = ElfMachine(abi::EM_OPEN8 as i32);
    pub const kEmRl78: ElfMachine = ElfMachine(abi::EM_RL78 as i32);
    pub const kEmVideocore5: ElfMachine = ElfMachine(abi::EM_VIDEOCORE5 as i32);
    pub const kEm78kor: ElfMachine = ElfMachine(abi::EM_78KOR as i32);
    pub const kEm56800ex: ElfMachine = ElfMachine(abi::EM_56800EX as i32);
    pub const kEmBa1: ElfMachine = ElfMachine(abi::EM_BA1 as i32);
    pub const kEmBa2: ElfMachine = ElfMachine(abi::EM_BA2 as i32);
    pub const kEmXcore: ElfMachine = ElfMachine(abi::EM_XCORE as i32);
    pub const kEmMchpPic: ElfMachine = ElfMachine(abi::EM_MCHP_PIC as i32);
    pub const kEmIntel205: ElfMachine = ElfMachine(abi::EM_INTEL205 as i32);
    pub const kEmIntel206: ElfMachine = ElfMachine(abi::EM_INTEL206 as i32);
    pub const kEmIntel207: ElfMachine = ElfMachine(abi::EM_INTEL207 as i32);
    pub const kEmIntel208: ElfMachine = ElfMachine(abi::EM_INTEL208 as i32);
    pub const kEmIntel209: ElfMachine = ElfMachine(abi::EM_INTEL209 as i32);
    pub const kEmKm32: ElfMachine = ElfMachine(abi::EM_KM32 as i32);
    pub const kEmKmx32: ElfMachine = ElfMachine(abi::EM_KMX32 as i32);
    pub const kEmKmx16: ElfMachine = ElfMachine(abi::EM_KMX16 as i32);
    pub const kEmKmx8: ElfMachine = ElfMachine(abi::EM_KMX8 as i32);
    pub const kEmKvarc: ElfMachine = ElfMachine(abi::EM_KVARC as i32);
    pub const kEmCdp: ElfMachine = ElfMachine(abi::EM_CDP as i32);
    pub const kEmCoge: ElfMachine = ElfMachine(abi::EM_COGE as i32);
    pub const kEmCool: ElfMachine = ElfMachine(abi::EM_COOL as i32);
    pub const kEmNorc: ElfMachine = ElfMachine(abi::EM_NORC as i32);
    pub const kEmCsrKalimba: ElfMachine = ElfMachine(abi::EM_CSR_KALIMBA as i32);
    pub const kEmZ80: ElfMachine = ElfMachine(abi::EM_Z80 as i32);
    pub const kEmVisium: ElfMachine = ElfMachine(abi::EM_VISIUM as i32);
    pub const kEmFt32: ElfMachine = ElfMachine(abi::EM_FT32 as i32);
    pub const kEmMoxie: ElfMachine = ElfMachine(abi::EM_MOXIE as i32);
    pub const kEmAmdgpu: ElfMachine = ElfMachine(abi::EM_AMDGPU as i32);
    pub const kEmRiscv: ElfMachine = ElfMachine(abi::EM_RISCV as i32);
    pub const kEmBpf: ElfMachine = ElfMachine(abi::EM_BPF as i32);
    pub const kEmNum: ElfMachine = ElfMachine(248_i32);
    pub const kUnknown: ElfMachine = ElfMachine(-1);
}

impl TryFrom<u16> for ElfMachine {
    type Error = anyhow::Error;

    fn try_from(v: u16) -> Result<Self> {
        Ok(match v {
            abi::EM_NONE => ElfMachine::kEmNone,
            abi::EM_M32 => ElfMachine::kEmM32,
            abi::EM_SPARC => ElfMachine::kEmSparc,
            abi::EM_386 => ElfMachine::kEm386,
            abi::EM_68K => ElfMachine::kEm68k,
            abi::EM_88K => ElfMachine::kEm88k,
            abi::EM_IAMCU => ElfMachine::kEmIamcu,
            abi::EM_860 => ElfMachine::kEm860,
            abi::EM_MIPS => ElfMachine::kEmMips,
            abi::EM_S370 => ElfMachine::kEmS370,
            abi::EM_MIPS_RS3_LE => ElfMachine::kEmMipsRs3Le,
            abi::EM_PARISC => ElfMachine::kEmParisc,
            abi::EM_VPP500 => ElfMachine::kEmVpp500,
            abi::EM_SPARC32PLUS => ElfMachine::kEmSparc32plus,
            abi::EM_960 => ElfMachine::kEm960,
            abi::EM_PPC => ElfMachine::kEmPpc,
            abi::EM_PPC64 => ElfMachine::kEmPpc64,
            abi::EM_S390 => ElfMachine::kEmS390,
            abi::EM_SPU => ElfMachine::kEmSpu,
            abi::EM_V800 => ElfMachine::kEmV800,
            abi::EM_FR20 => ElfMachine::kEmFr20,
            abi::EM_RH32 => ElfMachine::kEmRh32,
            abi::EM_RCE => ElfMachine::kEmRce,
            abi::EM_ARM => ElfMachine::kEmArm,
            abi::EM_ALPHA => ElfMachine::kEmAlpha,
            abi::EM_SH => ElfMachine::kEmSh,
            abi::EM_SPARCV9 => ElfMachine::kEmSparcv9,
            abi::EM_TRICORE => ElfMachine::kEmTricore,
            abi::EM_ARC => ElfMachine::kEmArc,
            abi::EM_H8_300 => ElfMachine::kEmH8300,
            abi::EM_H8_300H => ElfMachine::kEmH8300h,
            abi::EM_H8S => ElfMachine::kEmH8s,
            abi::EM_H8_500 => ElfMachine::kEmH8500,
            abi::EM_IA_64 => ElfMachine::kEmIa64,
            abi::EM_MIPS_X => ElfMachine::kEmMipsX,
            abi::EM_COLDFIRE => ElfMachine::kEmColdfire,
            abi::EM_68HC12 => ElfMachine::kEm68hc12,
            abi::EM_MMA => ElfMachine::kEmMma,
            abi::EM_PCP => ElfMachine::kEmPcp,
            abi::EM_NCPU => ElfMachine::kEmNcpu,
            abi::EM_NDR1 => ElfMachine::kEmNdr1,
            abi::EM_STARCORE => ElfMachine::kEmStarcore,
            abi::EM_ME16 => ElfMachine::kEmMe16,
            abi::EM_ST100 => ElfMachine::kEmSt100,
            abi::EM_TINYJ => ElfMachine::kEmTinyj,
            abi::EM_X86_64 => ElfMachine::kEmX8664,
            abi::EM_PDSP => ElfMachine::kEmPdsp,
            abi::EM_PDP10 => ElfMachine::kEmPdp10,
            abi::EM_PDP11 => ElfMachine::kEmPdp11,
            abi::EM_FX66 => ElfMachine::kEmFx66,
            abi::EM_ST9PLUS => ElfMachine::kEmSt9plus,
            abi::EM_ST7 => ElfMachine::kEmSt7,
            abi::EM_68HC16 => ElfMachine::kEm68hc16,
            abi::EM_68HC11 => ElfMachine::kEm68hc11,
            abi::EM_68HC08 => ElfMachine::kEm68hc08,
            abi::EM_68HC05 => ElfMachine::kEm68hc05,
            abi::EM_SVX => ElfMachine::kEmSvx,
            abi::EM_ST19 => ElfMachine::kEmSt19,
            abi::EM_VAX => ElfMachine::kEmVax,
            abi::EM_CRIS => ElfMachine::kEmCris,
            abi::EM_JAVELIN => ElfMachine::kEmJavelin,
            abi::EM_FIREPATH => ElfMachine::kEmFirepath,
            abi::EM_ZSP => ElfMachine::kEmZsp,
            abi::EM_MMIX => ElfMachine::kEmMmix,
            abi::EM_HUANY => ElfMachine::kEmHuany,
            abi::EM_PRISM => ElfMachine::kEmPrism,
            abi::EM_AVR => ElfMachine::kEmAvr,
            abi::EM_FR30 => ElfMachine::kEmFr30,
            abi::EM_D10V => ElfMachine::kEmD10v,
            abi::EM_D30V => ElfMachine::kEmD30v,
            abi::EM_V850 => ElfMachine::kEmV850,
            abi::EM_M32R => ElfMachine::kEmM32r,
            abi::EM_MN10300 => ElfMachine::kEmMn10300,
            abi::EM_MN10200 => ElfMachine::kEmMn10200,
            abi::EM_PJ => ElfMachine::kEmPj,
            abi::EM_OPENRISC => ElfMachine::kEmOpenrisc,
            abi::EM_ARC_COMPACT => ElfMachine::kEmArcCompact,
            abi::EM_XTENSA => ElfMachine::kEmXtensa,
            abi::EM_VIDEOCORE => ElfMachine::kEmVideocore,
            abi::EM_TMM_GPP => ElfMachine::kEmTmmGpp,
            abi::EM_NS32K => ElfMachine::kEmNs32k,
            abi::EM_TPC => ElfMachine::kEmTpc,
            abi::EM_SNP1K => ElfMachine::kEmSnp1k,
            abi::EM_ST200 => ElfMachine::kEmSt200,
            abi::EM_IP2K => ElfMachine::kEmIp2k,
            abi::EM_MAX => ElfMachine::kEmMax,
            abi::EM_CR => ElfMachine::kEmCr,
            abi::EM_F2MC16 => ElfMachine::kEmF2mc16,
            abi::EM_MSP430 => ElfMachine::kEmMsp430,
            abi::EM_BLACKFIN => ElfMachine::kEmBlackfin,
            abi::EM_SE_C33 => ElfMachine::kEmSeC33,
            abi::EM_SEP => ElfMachine::kEmSep,
            abi::EM_ARCA => ElfMachine::kEmArca,
            abi::EM_UNICORE => ElfMachine::kEmUnicore,
            abi::EM_EXCESS => ElfMachine::kEmExcess,
            abi::EM_DXP => ElfMachine::kEmDxp,
            abi::EM_ALTERA_NIOS2 => ElfMachine::kEmAlteraNios2,
            abi::EM_CRX => ElfMachine::kEmCrx,
            abi::EM_XGATE => ElfMachine::kEmXgate,
            abi::EM_C166 => ElfMachine::kEmC166,
            abi::EM_M16C => ElfMachine::kEmM16c,
            abi::EM_DSPIC30F => ElfMachine::kEmDspic30f,
            abi::EM_CE => ElfMachine::kEmCe,
            abi::EM_M32C => ElfMachine::kEmM32c,
            abi::EM_TSK3000 => ElfMachine::kEmTsk3000,
            abi::EM_RS08 => ElfMachine::kEmRs08,
            abi::EM_SHARC => ElfMachine::kEmSharc,
            abi::EM_ECOG2 => ElfMachine::kEmEcog2,
            abi::EM_SCORE7 => ElfMachine::kEmScore7,
            abi::EM_DSP24 => ElfMachine::kEmDsp24,
            abi::EM_VIDEOCORE3 => ElfMachine::kEmVideocore3,
            abi::EM_LATTICEMICO32 => ElfMachine::kEmLatticemico32,
            abi::EM_SE_C17 => ElfMachine::kEmSeC17,
            abi::EM_TI_C6000 => ElfMachine::kEmTiC6000,
            abi::EM_TI_C2000 => ElfMachine::kEmTiC2000,
            abi::EM_TI_C5500 => ElfMachine::kEmTiC5500,
            abi::EM_TI_ARP32 => ElfMachine::kEmTiArp32,
            abi::EM_TI_PRU => ElfMachine::kEmTiPru,
            abi::EM_MMDSP_PLUS => ElfMachine::kEmMmdspPlus,
            abi::EM_CYPRESS_M8C => ElfMachine::kEmCypressM8c,
            abi::EM_R32C => ElfMachine::kEmR32c,
            abi::EM_TRIMEDIA => ElfMachine::kEmTrimedia,
            abi::EM_QDSP6 => ElfMachine::kEmQdsp6,
            abi::EM_8051 => ElfMachine::kEm8051,
            abi::EM_STXP7X => ElfMachine::kEmStxp7x,
            abi::EM_NDS32 => ElfMachine::kEmNds32,
            abi::EM_ECOG1X => ElfMachine::kEmEcog1x,
            abi::EM_MAXQ30 => ElfMachine::kEmMaxq30,
            abi::EM_XIMO16 => ElfMachine::kEmXimo16,
            abi::EM_MANIK => ElfMachine::kEmManik,
            abi::EM_CRAYNV2 => ElfMachine::kEmCraynv2,
            abi::EM_RX => ElfMachine::kEmRx,
            abi::EM_METAG => ElfMachine::kEmMetag,
            abi::EM_MCST_ELBRUS => ElfMachine::kEmMcstElbrus,
            abi::EM_ECOG16 => ElfMachine::kEmEcog16,
            abi::EM_CR16 => ElfMachine::kEmCr16,
            abi::EM_ETPU => ElfMachine::kEmEtpu,
            abi::EM_SLE9X => ElfMachine::kEmSle9x,
            abi::EM_L10M => ElfMachine::kEmL10m,
            abi::EM_K10M => ElfMachine::kEmK10m,
            abi::EM_AARCH64 => ElfMachine::kEmAarch64,
            abi::EM_AVR32 => ElfMachine::kEmAvr32,
            abi::EM_STM8 => ElfMachine::kEmStm8,
            abi::EM_TILE64 => ElfMachine::kEmTile64,
            abi::EM_TILEPRO => ElfMachine::kEmTilepro,
            abi::EM_MICROBLAZE => ElfMachine::kEmMicroblaze,
            abi::EM_CUDA => ElfMachine::kEmCuda,
            abi::EM_TILEGX => ElfMachine::kEmTilegx,
            abi::EM_CLOUDSHIELD => ElfMachine::kEmCloudshield,
            abi::EM_COREA_1ST => ElfMachine::kEmCorea1st,
            abi::EM_COREA_2ND => ElfMachine::kEmCorea2nd,
            abi::EM_ARC_COMPACT2 => ElfMachine::kEmArcCompact2,
            abi::EM_OPEN8 => ElfMachine::kEmOpen8,
            abi::EM_RL78 => ElfMachine::kEmRl78,
            abi::EM_VIDEOCORE5 => ElfMachine::kEmVideocore5,
            abi::EM_78KOR => ElfMachine::kEm78kor,
            abi::EM_56800EX => ElfMachine::kEm56800ex,
            abi::EM_BA1 => ElfMachine::kEmBa1,
            abi::EM_BA2 => ElfMachine::kEmBa2,
            abi::EM_XCORE => ElfMachine::kEmXcore,
            abi::EM_MCHP_PIC => ElfMachine::kEmMchpPic,
            abi::EM_INTEL205 => ElfMachine::kEmIntel205,
            abi::EM_INTEL206 => ElfMachine::kEmIntel206,
            abi::EM_INTEL207 => ElfMachine::kEmIntel207,
            abi::EM_INTEL208 => ElfMachine::kEmIntel208,
            abi::EM_INTEL209 => ElfMachine::kEmIntel209,
            abi::EM_KM32 => ElfMachine::kEmKm32,
            abi::EM_KMX32 => ElfMachine::kEmKmx32,
            abi::EM_KMX16 => ElfMachine::kEmKmx16,
            abi::EM_KMX8 => ElfMachine::kEmKmx8,
            abi::EM_KVARC => ElfMachine::kEmKvarc,
            abi::EM_CDP => ElfMachine::kEmCdp,
            abi::EM_COGE => ElfMachine::kEmCoge,
            abi::EM_COOL => ElfMachine::kEmCool,
            abi::EM_NORC => ElfMachine::kEmNorc,
            abi::EM_CSR_KALIMBA => ElfMachine::kEmCsrKalimba,
            abi::EM_Z80 => ElfMachine::kEmZ80,
            abi::EM_VISIUM => ElfMachine::kEmVisium,
            abi::EM_FT32 => ElfMachine::kEmFt32,
            abi::EM_MOXIE => ElfMachine::kEmMoxie,
            abi::EM_AMDGPU => ElfMachine::kEmAmdgpu,
            abi::EM_RISCV => ElfMachine::kEmRiscv,
            abi::EM_BPF => ElfMachine::kEmBpf,
            _ => return Err(anyhow!("Invalid ELF machine type")),
        })
    }
}

make_result_type!(ElfMachine);

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct SectionType(pub u32);

#[allow(non_upper_case_globals)]
impl SectionType {
    pub const kNull: SectionType = SectionType(abi::SHT_NULL);
    pub const kProgBits: SectionType = SectionType(abi::SHT_PROGBITS);
    pub const kSymTab: SectionType = SectionType(abi::SHT_SYMTAB);
    pub const kStrTab: SectionType = SectionType(abi::SHT_STRTAB);
    pub const kRela: SectionType = SectionType(abi::SHT_RELA);
    pub const kHash: SectionType = SectionType(abi::SHT_HASH);
    pub const kDynamic: SectionType = SectionType(abi::SHT_DYNAMIC);
    pub const kNote: SectionType = SectionType(abi::SHT_NOTE);
    pub const kNoBits: SectionType = SectionType(abi::SHT_NOBITS);
    pub const kRel: SectionType = SectionType(abi::SHT_REL);
    pub const kShLib: SectionType = SectionType(abi::SHT_SHLIB);
    pub const kDynSym: SectionType = SectionType(abi::SHT_DYNSYM);
    pub const kInitArray: SectionType = SectionType(abi::SHT_INIT_ARRAY);
    pub const kFiniArray: SectionType = SectionType(abi::SHT_FINI_ARRAY);
    pub const kPreInitArray: SectionType = SectionType(abi::SHT_PREINIT_ARRAY);
    pub const kGroup: SectionType = SectionType(abi::SHT_GROUP);
    pub const kSymTabShndx: SectionType = SectionType(abi::SHT_SYMTAB_SHNDX);
    pub const kGnuAttributes: SectionType = SectionType(abi::SHT_GNU_ATTRIBUTES);
    pub const kGnuHash: SectionType = SectionType(abi::SHT_GNU_HASH);
    pub const kGnuLibList: SectionType = SectionType(abi::SHT_GNU_LIBLIST);
    pub const kGnuVerDef: SectionType = SectionType(abi::SHT_GNU_VERDEF);
    pub const kGnuVerNeed: SectionType = SectionType(abi::SHT_GNU_VERNEED);
    pub const kGnuVerSym: SectionType = SectionType(abi::SHT_GNU_VERSYM);
}

impl PartialEq<u32> for SectionType {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for SectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<u32> for SectionType {
    type Error = anyhow::Error;

    fn try_from(v: u32) -> Result<Self> {
        Ok(match v {
            abi::SHT_NULL => SectionType::kNull,
            abi::SHT_PROGBITS => SectionType::kProgBits,
            abi::SHT_SYMTAB => SectionType::kSymTab,
            abi::SHT_STRTAB => SectionType::kStrTab,
            abi::SHT_RELA => SectionType::kRela,
            abi::SHT_HASH => SectionType::kHash,
            abi::SHT_DYNAMIC => SectionType::kDynamic,
            abi::SHT_NOTE => SectionType::kNote,
            abi::SHT_NOBITS => SectionType::kNoBits,
            abi::SHT_REL => SectionType::kRel,
            abi::SHT_SHLIB => SectionType::kShLib,
            abi::SHT_DYNSYM => SectionType::kDynSym,
            abi::SHT_INIT_ARRAY => SectionType::kInitArray,
            abi::SHT_FINI_ARRAY => SectionType::kFiniArray,
            abi::SHT_PREINIT_ARRAY => SectionType::kPreInitArray,
            abi::SHT_GROUP => SectionType::kGroup,
            abi::SHT_SYMTAB_SHNDX => SectionType::kSymTabShndx,
            abi::SHT_GNU_ATTRIBUTES => SectionType::kGnuAttributes,
            abi::SHT_GNU_HASH => SectionType::kGnuHash,
            abi::SHT_GNU_LIBLIST => SectionType::kGnuLibList,
            abi::SHT_GNU_VERDEF => SectionType::kGnuVerDef,
            abi::SHT_GNU_VERNEED => SectionType::kGnuVerNeed,
            abi::SHT_GNU_VERSYM => SectionType::kGnuVerSym,
            _ => return Err(anyhow!("Invalid section type")),
        })
    }
}
make_result_type!(SectionType);

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct ProgramHeaderType(pub u32);

#[allow(non_upper_case_globals)]
impl ProgramHeaderType {
    pub const kNull: ProgramHeaderType = ProgramHeaderType(abi::PT_NULL);
    pub const kLoad: ProgramHeaderType = ProgramHeaderType(abi::PT_LOAD);
    pub const kDynamic: ProgramHeaderType = ProgramHeaderType(abi::PT_DYNAMIC);
    pub const kInterp: ProgramHeaderType = ProgramHeaderType(abi::PT_INTERP);
    pub const kNote: ProgramHeaderType = ProgramHeaderType(abi::PT_NOTE);
    pub const kShLib: ProgramHeaderType = ProgramHeaderType(abi::PT_SHLIB);
    pub const kPhdr: ProgramHeaderType = ProgramHeaderType(abi::PT_PHDR);
    pub const kTls: ProgramHeaderType = ProgramHeaderType(abi::PT_TLS);
    pub const kGnuEhFrame: ProgramHeaderType = ProgramHeaderType(abi::PT_GNU_EH_FRAME);
    pub const kGnuStack: ProgramHeaderType = ProgramHeaderType(abi::PT_GNU_STACK);
    pub const kGnuRelro: ProgramHeaderType = ProgramHeaderType(abi::PT_GNU_RELRO);
    pub const kGnuProperty: ProgramHeaderType = ProgramHeaderType(abi::PT_GNU_PROPERTY);
}

impl PartialEq<u32> for ProgramHeaderType {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for ProgramHeaderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl TryFrom<u32> for ProgramHeaderType {
    type Error = anyhow::Error;

    fn try_from(v: u32) -> Result<Self> {
        Ok(match v {
            abi::PT_NULL => ProgramHeaderType::kNull,
            abi::PT_LOAD => ProgramHeaderType::kLoad,
            abi::PT_DYNAMIC => ProgramHeaderType::kDynamic,
            abi::PT_INTERP => ProgramHeaderType::kInterp,
            abi::PT_NOTE => ProgramHeaderType::kNote,
            abi::PT_SHLIB => ProgramHeaderType::kShLib,
            abi::PT_PHDR => ProgramHeaderType::kPhdr,
            abi::PT_TLS => ProgramHeaderType::kTls,
            abi::PT_GNU_EH_FRAME => ProgramHeaderType::kGnuEhFrame,
            abi::PT_GNU_STACK => ProgramHeaderType::kGnuStack,
            abi::PT_GNU_RELRO => ProgramHeaderType::kGnuRelro,
            abi::PT_GNU_PROPERTY => ProgramHeaderType::kGnuProperty,
            _ => return Err(anyhow!("Invalid program header type")),
        })
    }
}
make_result_type!(ProgramHeaderType);

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct SymbolType(pub i32);

#[allow(non_upper_case_globals)]
impl SymbolType {
    pub const kNoType: SymbolType = SymbolType(abi::STT_NOTYPE as i32);
    pub const kObject: SymbolType = SymbolType(abi::STT_OBJECT as i32);
    pub const kFunc: SymbolType = SymbolType(abi::STT_FUNC as i32);
    pub const kSection: SymbolType = SymbolType(abi::STT_SECTION as i32);
    pub const kFile: SymbolType = SymbolType(abi::STT_FILE as i32);
    pub const kCommon: SymbolType = SymbolType(abi::STT_COMMON as i32);
    pub const kTls: SymbolType = SymbolType(abi::STT_TLS as i32);
    pub const kGnuIFunc: SymbolType = SymbolType(abi::STT_GNU_IFUNC as i32);
}
impl TryFrom<u8> for SymbolType {
    type Error = anyhow::Error;

    fn try_from(v: u8) -> Result<Self> {
        Ok(match v {
            abi::STT_NOTYPE => SymbolType::kNoType,
            abi::STT_OBJECT => SymbolType::kObject,
            abi::STT_FUNC => SymbolType::kFunc,
            abi::STT_SECTION => SymbolType::kSection,
            abi::STT_FILE => SymbolType::kFile,
            abi::STT_COMMON => SymbolType::kCommon,
            abi::STT_TLS => SymbolType::kTls,
            abi::STT_GNU_IFUNC => SymbolType::kGnuIFunc,
            _ => return Err(anyhow!("Invalid symbol type")),
        })
    }
}
make_result_type!(SymbolType);

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct SymbolBind(pub i32);

#[allow(non_upper_case_globals)]
impl SymbolBind {
    pub const kLocal: SymbolBind = SymbolBind(abi::STB_LOCAL as i32);
    pub const kGlobal: SymbolBind = SymbolBind(abi::STB_GLOBAL as i32);
    pub const kWeak: SymbolBind = SymbolBind(abi::STB_WEAK as i32);
    pub const kGnuUnique: SymbolBind = SymbolBind(abi::STB_GNU_UNIQUE as i32);
}
impl TryFrom<u8> for SymbolBind {
    type Error = anyhow::Error;

    fn try_from(v: u8) -> Result<Self> {
        Ok(match v {
            abi::STB_LOCAL => SymbolBind::kLocal,
            abi::STB_GLOBAL => SymbolBind::kGlobal,
            abi::STB_WEAK => SymbolBind::kWeak,
            abi::STB_GNU_UNIQUE => SymbolBind::kGnuUnique,
            _ => return Err(anyhow!("Invalid symbol binding")),
        })
    }
}
make_result_type!(SymbolBind);

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct SymbolVisibility(pub i32);

#[allow(non_upper_case_globals)]
impl SymbolVisibility {
    pub const kDefault: SymbolVisibility = SymbolVisibility(abi::STV_DEFAULT as i32);
    pub const kInternal: SymbolVisibility = SymbolVisibility(abi::STV_INTERNAL as i32);
    pub const kHidden: SymbolVisibility = SymbolVisibility(abi::STV_HIDDEN as i32);
    pub const kProtected: SymbolVisibility = SymbolVisibility(abi::STV_PROTECTED as i32);
}
impl TryFrom<u8> for SymbolVisibility {
    type Error = anyhow::Error;

    fn try_from(v: u8) -> Result<Self> {
        Ok(match v {
            abi::STV_DEFAULT => SymbolVisibility::kDefault,
            abi::STV_INTERNAL => SymbolVisibility::kInternal,
            abi::STV_HIDDEN => SymbolVisibility::kHidden,
            abi::STV_PROTECTED => SymbolVisibility::kProtected,
            _ => return Err(anyhow!("Invalid symbol visibility")),
        })
    }
}
make_result_type!(SymbolVisibility);

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct DynamicTag(pub i64);

#[allow(non_upper_case_globals)]
impl DynamicTag {
    pub const kNull: DynamicTag = DynamicTag(abi::DT_NULL);
    pub const kNeeded: DynamicTag = DynamicTag(abi::DT_NEEDED);
    pub const kPltRelSz: DynamicTag = DynamicTag(abi::DT_PLTRELSZ);
    pub const kPltGot: DynamicTag = DynamicTag(abi::DT_PLTGOT);
    pub const kHash: DynamicTag = DynamicTag(abi::DT_HASH);
    pub const kStrTab: DynamicTag = DynamicTag(abi::DT_STRTAB);
    pub const kSymTab: DynamicTag = DynamicTag(abi::DT_SYMTAB);
    pub const kRela: DynamicTag = DynamicTag(abi::DT_RELA);
    pub const kRelaSz: DynamicTag = DynamicTag(abi::DT_RELASZ);
    pub const kRelaEnt: DynamicTag = DynamicTag(abi::DT_RELAENT);
    pub const kStrSz: DynamicTag = DynamicTag(abi::DT_STRSZ);
    pub const kSymEnt: DynamicTag = DynamicTag(abi::DT_SYMENT);
    pub const kInit: DynamicTag = DynamicTag(abi::DT_INIT);
    pub const kFini: DynamicTag = DynamicTag(abi::DT_FINI);
    pub const kSoname: DynamicTag = DynamicTag(abi::DT_SONAME);
    pub const kRpath: DynamicTag = DynamicTag(abi::DT_RPATH);
    pub const kSymbolic: DynamicTag = DynamicTag(abi::DT_SYMBOLIC);
    pub const kRel: DynamicTag = DynamicTag(abi::DT_REL);
    pub const kRelSz: DynamicTag = DynamicTag(abi::DT_RELSZ);
    pub const kRelEnt: DynamicTag = DynamicTag(abi::DT_RELENT);
    pub const kPltRel: DynamicTag = DynamicTag(abi::DT_PLTREL);
    pub const kDebug: DynamicTag = DynamicTag(abi::DT_DEBUG);
    pub const kTextRel: DynamicTag = DynamicTag(abi::DT_TEXTREL);
    pub const kJmpRel: DynamicTag = DynamicTag(abi::DT_JMPREL);
    pub const kBindNow: DynamicTag = DynamicTag(abi::DT_BIND_NOW);
    pub const kInitArray: DynamicTag = DynamicTag(abi::DT_INIT_ARRAY);
    pub const kFiniArray: DynamicTag = DynamicTag(abi::DT_FINI_ARRAY);
    pub const kInitArraySz: DynamicTag = DynamicTag(abi::DT_INIT_ARRAYSZ);
    pub const kFiniArraySz: DynamicTag = DynamicTag(abi::DT_FINI_ARRAYSZ);
    pub const kRunpath: DynamicTag = DynamicTag(abi::DT_RUNPATH);
    pub const kFlags: DynamicTag = DynamicTag(abi::DT_FLAGS);
    pub const kPreInitArray: DynamicTag = DynamicTag(abi::DT_PREINIT_ARRAY);
    pub const kPreInitArraySz: DynamicTag = DynamicTag(abi::DT_PREINIT_ARRAYSZ);
    pub const kSymTabShndx: DynamicTag = DynamicTag(abi::DT_SYMTAB_SHNDX);
    pub const kGuileGcRoot: DynamicTag = DynamicTag(abi::DT_GUILE_GC_ROOT);
    pub const kGuileGcRootSz: DynamicTag = DynamicTag(abi::DT_GUILE_GC_ROOT_SZ);
    pub const kGuileEntry: DynamicTag = DynamicTag(abi::DT_GUILE_ENTRY);
    pub const kGuileVmVersion: DynamicTag = DynamicTag(abi::DT_GUILE_VM_VERSION);
    pub const kGuileFrameMaps: DynamicTag = DynamicTag(abi::DT_GUILE_FRAME_MAPS);
    pub const kLoos: DynamicTag = DynamicTag(abi::DT_LOOS);
    pub const kGnuPrelinked: DynamicTag = DynamicTag(abi::DT_GNU_PRELINKED);
    pub const kGnuConflictSz: DynamicTag = DynamicTag(abi::DT_GNU_CONFLICTSZ);
    pub const kGnuLibListSz: DynamicTag = DynamicTag(abi::DT_GNU_LIBLISTSZ);
    pub const kChecksum: DynamicTag = DynamicTag(abi::DT_CHECKSUM);
    pub const kPltPadSz: DynamicTag = DynamicTag(abi::DT_PLTPADSZ);
    pub const kMoveEnt: DynamicTag = DynamicTag(abi::DT_MOVEENT);
    pub const kMoveSz: DynamicTag = DynamicTag(abi::DT_MOVESZ);
    pub const kFeature1: DynamicTag = DynamicTag(abi::DT_FEATURE_1);
    pub const kPosflag1: DynamicTag = DynamicTag(abi::DT_POSFLAG_1);
    pub const kSymInSz: DynamicTag = DynamicTag(abi::DT_SYMINSZ);
    pub const kSymInEnt: DynamicTag = DynamicTag(abi::DT_SYMINENT);
    pub const kGnuHash: DynamicTag = DynamicTag(abi::DT_GNU_HASH);
    pub const kTlsDescPlt: DynamicTag = DynamicTag(abi::DT_TLSDESC_PLT);
    pub const kTlsDescGot: DynamicTag = DynamicTag(abi::DT_TLSDESC_GOT);
    pub const kGnuConflict: DynamicTag = DynamicTag(abi::DT_GNU_CONFLICT);
    pub const kGnuLiblist: DynamicTag = DynamicTag(abi::DT_GNU_LIBLIST);
    pub const kConfig: DynamicTag = DynamicTag(abi::DT_CONFIG);
    pub const kDepaudit: DynamicTag = DynamicTag(abi::DT_DEPAUDIT);
    pub const kAudit: DynamicTag = DynamicTag(abi::DT_AUDIT);
    pub const kPltPad: DynamicTag = DynamicTag(abi::DT_PLTPAD);
    pub const kMoveTab: DynamicTag = DynamicTag(abi::DT_MOVETAB);
    pub const kSymInfo: DynamicTag = DynamicTag(abi::DT_SYMINFO);
    pub const kVersym: DynamicTag = DynamicTag(abi::DT_VERSYM);
    pub const kRelaCount: DynamicTag = DynamicTag(abi::DT_RELACOUNT);
    pub const kRelCount: DynamicTag = DynamicTag(abi::DT_RELCOUNT);
    pub const kFlags1: DynamicTag = DynamicTag(abi::DT_FLAGS_1);
    pub const kVerDef: DynamicTag = DynamicTag(abi::DT_VERDEF);
    pub const kVerDefNum: DynamicTag = DynamicTag(abi::DT_VERDEFNUM);
    pub const kVerNeed: DynamicTag = DynamicTag(abi::DT_VERNEED);
    pub const kVerNeedNum: DynamicTag = DynamicTag(abi::DT_VERNEEDNUM);
    pub const kHios: DynamicTag = DynamicTag(abi::DT_HIOS);
    pub const kLoproc: DynamicTag = DynamicTag(abi::DT_LOPROC);
    pub const kHiproc: DynamicTag = DynamicTag(abi::DT_HIPROC);
    pub const kArmSymTabSz: DynamicTag = DynamicTag(abi::DT_ARM_SYMTABSZ);
    pub const kArmPreemptMap: DynamicTag = DynamicTag(abi::DT_ARM_PREEMPTMAP);
    pub const kAarch64BtiPlt: DynamicTag = DynamicTag(abi::DT_AARCH64_BTI_PLT);
    pub const kAarch64PacPlt: DynamicTag = DynamicTag(abi::DT_AARCH64_PAC_PLT);
    pub const kAarch64VariantPcs: DynamicTag = DynamicTag(abi::DT_AARCH64_VARIANT_PCS);
    pub const kPpcGot: DynamicTag = DynamicTag(abi::DT_PPC_GOT);
    pub const kPpcOpt: DynamicTag = DynamicTag(abi::DT_PPC_OPT);
    pub const kPpc64Glink: DynamicTag = DynamicTag(abi::DT_PPC64_GLINK);
    pub const kPpc64Opd: DynamicTag = DynamicTag(abi::DT_PPC64_OPD);
    pub const kPpc64OpdSz: DynamicTag = DynamicTag(abi::DT_PPC64_OPDSZ);
    pub const kPpc64Opt: DynamicTag = DynamicTag(abi::DT_PPC64_OPT);
    pub const kRiscvVariantCc: DynamicTag = DynamicTag(abi::DT_RISCV_VARIANT_CC);
}

impl PartialEq<i64> for DynamicTag {
    fn eq(&self, other: &i64) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for DynamicTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<i64> for DynamicTag {
    fn from(v: i64) -> Self {
        DynamicTag(v)
    }
}

use std::collections::BTreeSet;

use walrus::{
    ir::{self, dfs_in_order, Value, Visitor},
    Module, ValType,
};

use crate::{
    abi::oasis::{Info, OasisV1},
    Config, Error, Parameters,
};

const EXPORT_SUB_VERSION_PREFIX: &str = "__oasis_sv_";

fn check_valtype_acceptable(ty: ValType) -> Result<(), Error> {
    match ty {
        ValType::F32 | ValType::F64 => Err(Error::ModuleUsesFloatingPoint),
        _ => Ok(()),
    }
}

struct FloatScanner(bool);

impl<'instr> Visitor<'instr> for FloatScanner {
    fn visit_const(&mut self, instr: &ir::Const) {
        match instr.value {
            Value::F32(_) | Value::F64(_) => self.0 = true,
            _ => {}
        }
    }

    fn visit_binop(&mut self, instr: &ir::Binop) {
        use ir::BinaryOp::*;
        match instr.op {
            F32Eq
            | F32Ne
            | F32Lt
            | F32Gt
            | F32Le
            | F32Ge
            | F64Eq
            | F64Ne
            | F64Lt
            | F64Gt
            | F64Le
            | F64Ge
            | F32Add
            | F32Sub
            | F32Mul
            | F32Div
            | F32Min
            | F32Max
            | F32Copysign
            | F64Add
            | F64Sub
            | F64Mul
            | F64Div
            | F64Min
            | F64Max
            | F64Copysign
            | F32x4ReplaceLane { .. }
            | F64x2ReplaceLane { .. }
            | F32x4Eq
            | F32x4Ne
            | F32x4Lt
            | F32x4Gt
            | F32x4Le
            | F32x4Ge
            | F64x2Eq
            | F64x2Ne
            | F64x2Lt
            | F64x2Gt
            | F64x2Le
            | F64x2Ge
            | F32x4Add
            | F32x4Sub
            | F32x4Mul
            | F32x4Div
            | F32x4Min
            | F32x4Max
            | F32x4PMin
            | F32x4PMax
            | F64x2Add
            | F64x2Sub
            | F64x2Mul
            | F64x2Div
            | F64x2Min
            | F64x2Max
            | F64x2PMin
            | F64x2PMax => self.0 = true,
            I32Eq
            | I32Ne
            | I32LtS
            | I32LtU
            | I32GtS
            | I32GtU
            | I32LeS
            | I32LeU
            | I32GeS
            | I32GeU
            | I64Eq
            | I64Ne
            | I64LtS
            | I64LtU
            | I64GtS
            | I64GtU
            | I64LeS
            | I64LeU
            | I64GeS
            | I64GeU
            | I32Add
            | I32Sub
            | I32Mul
            | I32DivS
            | I32DivU
            | I32RemS
            | I32RemU
            | I32And
            | I32Or
            | I32Xor
            | I32Shl
            | I32ShrS
            | I32ShrU
            | I32Rotl
            | I32Rotr
            | I64Add
            | I64Sub
            | I64Mul
            | I64DivS
            | I64DivU
            | I64RemS
            | I64RemU
            | I64And
            | I64Or
            | I64Xor
            | I64Shl
            | I64ShrS
            | I64ShrU
            | I64Rotl
            | I64Rotr
            | I8x16ReplaceLane { .. }
            | I16x8ReplaceLane { .. }
            | I32x4ReplaceLane { .. }
            | I64x2ReplaceLane { .. }
            | I8x16Eq
            | I8x16Ne
            | I8x16LtS
            | I8x16LtU
            | I8x16GtS
            | I8x16GtU
            | I8x16LeS
            | I8x16LeU
            | I8x16GeS
            | I8x16GeU
            | I16x8Eq
            | I16x8Ne
            | I16x8LtS
            | I16x8LtU
            | I16x8GtS
            | I16x8GtU
            | I16x8LeS
            | I16x8LeU
            | I16x8GeS
            | I16x8GeU
            | I32x4Eq
            | I32x4Ne
            | I32x4LtS
            | I32x4LtU
            | I32x4GtS
            | I32x4GtU
            | I32x4LeS
            | I32x4LeU
            | I32x4GeS
            | I32x4GeU
            | I64x2Eq
            | I64x2Ne
            | I64x2LtS
            | I64x2GtS
            | I64x2LeS
            | I64x2GeS
            | V128And
            | V128Or
            | V128Xor
            | V128AndNot
            | I8x16Shl
            | I8x16ShrS
            | I8x16ShrU
            | I8x16Add
            | I8x16AddSatS
            | I8x16AddSatU
            | I8x16Sub
            | I8x16SubSatS
            | I8x16SubSatU
            | I16x8Shl
            | I16x8ShrS
            | I16x8ShrU
            | I16x8Add
            | I16x8AddSatS
            | I16x8AddSatU
            | I16x8Sub
            | I16x8SubSatS
            | I16x8SubSatU
            | I16x8Mul
            | I32x4Shl
            | I32x4ShrS
            | I32x4ShrU
            | I32x4Add
            | I32x4Sub
            | I32x4Mul
            | I64x2Shl
            | I64x2ShrS
            | I64x2ShrU
            | I64x2Add
            | I64x2Sub
            | I64x2Mul
            | I8x16NarrowI16x8S
            | I8x16NarrowI16x8U
            | I16x8NarrowI32x4S
            | I16x8NarrowI32x4U
            | I8x16RoundingAverageU
            | I16x8RoundingAverageU
            | I8x16MinS
            | I8x16MinU
            | I8x16MaxS
            | I8x16MaxU
            | I16x8MinS
            | I16x8MinU
            | I16x8MaxS
            | I16x8MaxU
            | I32x4MinS
            | I32x4MinU
            | I32x4MaxS
            | I32x4MaxU
            | I32x4DotI16x8S
            | I16x8Q15MulrSatS
            | I16x8ExtMulLowI8x16S
            | I16x8ExtMulHighI8x16S
            | I16x8ExtMulLowI8x16U
            | I16x8ExtMulHighI8x16U
            | I32x4ExtMulLowI16x8S
            | I32x4ExtMulHighI16x8S
            | I32x4ExtMulLowI16x8U
            | I32x4ExtMulHighI16x8U
            | I64x2ExtMulLowI32x4S
            | I64x2ExtMulHighI32x4S
            | I64x2ExtMulLowI32x4U
            | I64x2ExtMulHighI32x4U => {
                // Ignore these. They're all listed so we don't need to use
                // the catch-all arm and miss any future additions.
            }
        }
    }

    fn visit_unop(&mut self, instr: &ir::Unop) {
        use ir::UnaryOp::*;
        match instr.op {
            F32Abs
            | F32Neg
            | F32Ceil
            | F32Floor
            | F32Trunc
            | F32Nearest
            | F32Sqrt
            | F64Abs
            | F64Neg
            | F64Ceil
            | F64Floor
            | F64Trunc
            | F64Nearest
            | F64Sqrt
            | I32TruncSF32
            | I32TruncUF32
            | I32TruncSF64
            | I32TruncUF64
            | I64TruncSF32
            | I64TruncUF32
            | I64TruncSF64
            | I64TruncUF64
            | F32ConvertSI32
            | F32ConvertUI32
            | F32ConvertSI64
            | F32ConvertUI64
            | F32DemoteF64
            | F64ConvertSI32
            | F64ConvertUI32
            | F64ConvertSI64
            | F64ConvertUI64
            | F64PromoteF32
            | I32ReinterpretF32
            | I64ReinterpretF64
            | F32ReinterpretI32
            | F64ReinterpretI64
            | F32x4Splat
            | F32x4ExtractLane { .. }
            | F64x2Splat
            | F64x2ExtractLane { .. }
            | F32x4Abs
            | F32x4Neg
            | F32x4Sqrt
            | F32x4Ceil
            | F32x4Floor
            | F32x4Trunc
            | F32x4Nearest
            | F64x2Abs
            | F64x2Neg
            | F64x2Sqrt
            | F64x2Ceil
            | F64x2Floor
            | F64x2Trunc
            | F64x2Nearest
            | I32x4TruncSatF64x2SZero
            | I32x4TruncSatF64x2UZero
            | F64x2ConvertLowI32x4S
            | F64x2ConvertLowI32x4U
            | F32x4DemoteF64x2Zero
            | F64x2PromoteLowF32x4
            | I32x4TruncSatF32x4S
            | I32x4TruncSatF32x4U
            | F32x4ConvertI32x4S
            | F32x4ConvertI32x4U
            | I32TruncSSatF32
            | I32TruncUSatF32
            | I32TruncSSatF64
            | I32TruncUSatF64
            | I64TruncSSatF32
            | I64TruncUSatF32
            | I64TruncSSatF64
            | I64TruncUSatF64 => self.0 = true,
            I32Eqz
            | I32Clz
            | I32Ctz
            | I32Popcnt
            | I64Eqz
            | I64Clz
            | I64Ctz
            | I64Popcnt
            | I32WrapI64
            | I64ExtendSI32
            | I64ExtendUI32
            | I32Extend8S
            | I32Extend16S
            | I64Extend8S
            | I64Extend16S
            | I64Extend32S
            | I8x16Splat
            | I8x16ExtractLaneS { .. }
            | I8x16ExtractLaneU { .. }
            | I16x8Splat
            | I16x8ExtractLaneS { .. }
            | I16x8ExtractLaneU { .. }
            | I32x4Splat
            | I32x4ExtractLane { .. }
            | I64x2Splat
            | I64x2ExtractLane { .. }
            | V128Not
            | V128AnyTrue
            | I8x16Abs
            | I8x16Popcnt
            | I8x16Neg
            | I8x16AllTrue
            | I8x16Bitmask
            | I16x8Abs
            | I16x8Neg
            | I16x8AllTrue
            | I16x8Bitmask
            | I32x4Abs
            | I32x4Neg
            | I32x4AllTrue
            | I32x4Bitmask
            | I64x2Abs
            | I64x2Neg
            | I64x2AllTrue
            | I64x2Bitmask
            | I16x8ExtAddPairwiseI8x16S
            | I16x8ExtAddPairwiseI8x16U
            | I32x4ExtAddPairwiseI16x8S
            | I32x4ExtAddPairwiseI16x8U
            | I64x2ExtendLowI32x4S
            | I64x2ExtendHighI32x4S
            | I64x2ExtendLowI32x4U
            | I64x2ExtendHighI32x4U
            | I16x8WidenLowI8x16S
            | I16x8WidenLowI8x16U
            | I16x8WidenHighI8x16S
            | I16x8WidenHighI8x16U
            | I32x4WidenLowI16x8S
            | I32x4WidenLowI16x8U
            | I32x4WidenHighI16x8S
            | I32x4WidenHighI16x8U => {
                // Ignore these. They're all listed so we don't need to use
                // the catch-all arm and miss any future additions.
            }
        }
    }

    fn visit_load(&mut self, instr: &ir::Load) {
        match instr.kind {
            ir::LoadKind::F32 | ir::LoadKind::F64 => self.0 = true,
            _ => {}
        }
    }

    fn visit_store(&mut self, instr: &ir::Store) {
        match instr.kind {
            ir::StoreKind::F32 | ir::StoreKind::F64 => self.0 = true,
            _ => {}
        }
    }
}

impl<Cfg: Config> OasisV1<Cfg> {
    pub(super) fn validate_module(
        &self,
        module: &mut Module,
        params: &Parameters,
    ) -> Result<Info, Error> {
        // Verify that all required exports are there.
        let exports: BTreeSet<&str> = module
            .exports
            .iter()
            .map(|export| export.name.as_str())
            .collect();
        for required in Self::REQUIRED_EXPORTS {
            if !exports.contains(required) {
                return Err(Error::CodeMissingRequiredExport(required.to_string()));
            }
        }

        for reserved in Self::RESERVED_EXPORTS {
            if exports.contains(reserved) {
                return Err(Error::CodeDeclaresReservedExport(reserved.to_string()));
            }
        }

        // Determine supported ABI sub-version.
        let sv_exports: Vec<_> = exports
            .iter()
            .filter(|export| export.starts_with(EXPORT_SUB_VERSION_PREFIX))
            .collect();
        let abi_sv = match sv_exports[..] {
            [] => {
                // No versions, this is v0.
                0
            }
            [sv] => {
                // A single version, parse which one.
                sv.strip_prefix(EXPORT_SUB_VERSION_PREFIX)
                    .ok_or(Error::CodeMalformed)?
                    .parse::<u32>()
                    .map_err(|_| Error::CodeMalformed)?
            }
            _ => {
                // Multiple versions.
                return Err(Error::CodeDeclaresMultipleSubVersions);
            }
        };

        // Verify that there is no start function defined.
        if module.start.is_some() {
            return Err(Error::CodeDeclaresStartFunction);
        }

        // Verify that there is at most one memory defined.
        if module.memories.iter().count() > 1 {
            return Err(Error::CodeDeclaresTooManyMemories);
        }

        // Verify that the code doesn't use any floating point instructions.
        let mut function_count = 0u32;
        for func in module.functions() {
            let func_type = module.types.get(func.ty());
            for val_type in func_type.params().iter().chain(func_type.results().iter()) {
                check_valtype_acceptable(*val_type)?;
            }
            if let walrus::FunctionKind::Local(local) = &func.kind {
                function_count += 1;
                if function_count > params.max_wasm_functions {
                    return Err(Error::CodeDeclaresTooManyFunctions);
                }
                let mut scanner = FloatScanner(false);
                dfs_in_order(&mut scanner, local, local.entry_block());
                if scanner.0 {
                    return Err(Error::ModuleUsesFloatingPoint);
                }
            }
        }

        // ... or tables with floating point elements.
        for table in module.tables.iter() {
            check_valtype_acceptable(table.element_ty)?;
        }

        // ... or floating point elements.
        for element in module.elements.iter() {
            check_valtype_acceptable(element.ty)?;
        }

        // ... or floating point globals.
        for global in module.globals.iter() {
            check_valtype_acceptable(global.ty)?;
        }

        // ... just don't think about floats in any way.
        let mut local_count = 0u32;
        for local in module.locals.iter() {
            local_count += 1;
            if local_count > params.max_wasm_locals {
                return Err(Error::CodeDeclaresTooManyLocals);
            }
            check_valtype_acceptable(local.ty())?;
        }

        Ok(Info { abi_sv })
    }
}

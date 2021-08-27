//! Gas metering instrumentation.
use std::collections::BTreeMap;

use walrus::{ir::*, FunctionBuilder, GlobalId, LocalFunction, Module};

use crate::Error;

/// Name of the exported global that holds the gas limit.
pub const EXPORT_GAS_LIMIT: &str = "gas_limit";
/// Name of the exported global that holds the gas limit exhausted flag.
pub const EXPORT_GAS_LIMIT_EXHAUSTED: &str = "gas_limit_exhausted";

/// Configures the gas limit on the given instance.
pub fn set_gas_limit<C>(
    instance: &wasm3::Instance<'_, '_, C>,
    gas_limit: u64,
) -> Result<(), Error> {
    instance
        .set_global(EXPORT_GAS_LIMIT, gas_limit)
        .map_err(|err| Error::ExecutionFailed(err.into()))
}

/// Returns the remaining gas.
pub fn get_remaining_gas<C>(instance: &wasm3::Instance<'_, '_, C>) -> Result<u64, Error> {
    instance
        .get_global(EXPORT_GAS_LIMIT)
        .map_err(|err| Error::ExecutionFailed(err.into()))
}

/// Checks whether gas limit has been exhausted while the given instance was executing.
pub fn is_gas_limit_exhausted<C>(instance: &wasm3::Instance<'_, '_, C>) -> bool {
    let value: u32 = instance
        .get_global(EXPORT_GAS_LIMIT_EXHAUSTED)
        .unwrap_or_default();
    value != 0
}

/// Attempts to use the given amount of gas.
pub fn use_gas<C>(instance: &wasm3::Instance<'_, '_, C>, amount: u64) -> Result<(), wasm3::Trap> {
    let gas_limit: u64 = instance
        .get_global(EXPORT_GAS_LIMIT)
        .map_err(|_| wasm3::Trap::Abort)?;
    if gas_limit < amount {
        let _ = instance.set_global(EXPORT_GAS_LIMIT_EXHAUSTED, 1u32);
        return Err(wasm3::Trap::Abort);
    }
    instance
        .set_global(EXPORT_GAS_LIMIT, gas_limit - amount)
        .map_err(|_| wasm3::Trap::Abort)?;
    Ok(())
}

/// Inject gas metering instrumentation into the module.
pub fn transform(module: &mut Module) {
    let gas_limit_global = module.globals.add_local(
        walrus::ValType::I64,
        true,
        walrus::InitExpr::Value(Value::I64(0)),
    );
    let gas_limit_exhausted_global = module.globals.add_local(
        walrus::ValType::I32,
        true,
        walrus::InitExpr::Value(Value::I32(0)),
    );
    module.exports.add(EXPORT_GAS_LIMIT, gas_limit_global);
    module
        .exports
        .add(EXPORT_GAS_LIMIT_EXHAUSTED, gas_limit_exhausted_global);

    for (_, func) in module.funcs.iter_local_mut() {
        transform_function(func, gas_limit_global, gas_limit_exhausted_global);
    }
}

/// Instruction cost function.
fn instruction_cost(_instr: &Instr) -> u64 {
    // Currently default to 1 for all instructions.
    1
}

/// A block of instructions which is metered.
#[derive(Debug)]
struct MeteredBlock {
    /// Instruction sequence where metering code should be injected.
    seq_id: InstrSeqId,
    /// Start index of instruction within the instruction sequence before which the metering code
    /// should be injected.
    start_index: usize,
    /// Instruction cost.
    cost: u64,
    /// Indication of whether the metered block can be merged in case instruction sequence and start
    /// index match. In case the block cannot be merged this contains the index
    merge_index: Option<usize>,
}

impl MeteredBlock {
    fn new(seq_id: InstrSeqId, start_index: usize) -> Self {
        Self {
            seq_id,
            start_index,
            cost: 0,
            merge_index: None,
        }
    }

    /// Create a mergable version of this metered block with the given start index.
    fn mergable(&self, start_index: usize) -> Self {
        Self {
            seq_id: self.seq_id,
            start_index,
            cost: 0,
            merge_index: Some(self.start_index),
        }
    }
}

/// A map of finalized metered blocks.
#[derive(Default)]
struct MeteredBlocks {
    blocks: BTreeMap<InstrSeqId, Vec<MeteredBlock>>,
}

impl MeteredBlocks {
    /// Finalize the given metered block. This means that the cost associated with the block cannot
    /// change anymore.
    fn finalize(&mut self, block: MeteredBlock) {
        if block.cost > 0 {
            self.blocks.entry(block.seq_id).or_default().push(block);
        }
    }
}

fn determine_metered_blocks(func: &mut LocalFunction) -> BTreeMap<InstrSeqId, Vec<MeteredBlock>> {
    // NOTE: This is based on walrus::ir::dfs_in_order but we need more information.

    let mut blocks = MeteredBlocks::default();
    let mut stack: Vec<(InstrSeqId, usize, MeteredBlock)> = vec![(
        func.entry_block(),                       // Initial instruction sequence to visit.
        0,                                        // Instruction offset within the sequence.
        MeteredBlock::new(func.entry_block(), 0), // Initial metered block.
    )];

    'traversing_blocks: while let Some((seq_id, index, mut metered_block)) = stack.pop() {
        let seq = func.block(seq_id);

        'traversing_instrs: for (index, (instr, _)) in seq.instrs.iter().enumerate().skip(index) {
            // NOTE: Current instruction is always included in the current metered block.
            metered_block.cost += instruction_cost(instr);

            // Determine whether we need to end/start a metered block.
            match instr {
                Instr::Block(Block { seq }) => {
                    // Do not start a new metered block as blocks are unconditional and metered
                    // blocks can encompass many of them to avoid injecting unnecessary
                    // instructions.
                    stack.push((seq_id, index + 1, metered_block.mergable(index + 1)));
                    stack.push((*seq, 0, metered_block));
                    continue 'traversing_blocks;
                }

                Instr::Loop(Loop { seq }) => {
                    // Finalize current metered block.
                    blocks.finalize(metered_block);
                    // Start a new metered block for remainder of block.
                    stack.push((seq_id, index + 1, MeteredBlock::new(seq_id, index + 1)));
                    // Start a new metered block for loop body.
                    stack.push((*seq, 0, MeteredBlock::new(*seq, 0)));
                    continue 'traversing_blocks;
                }

                Instr::IfElse(IfElse {
                    consequent,
                    alternative,
                }) => {
                    // Finalize current metered block.
                    blocks.finalize(metered_block);

                    // Start a new metered block for remainder of block.
                    stack.push((seq_id, index + 1, MeteredBlock::new(seq_id, index + 1)));
                    // Start new metered blocks for alternative and consequent blocks.
                    stack.push((*alternative, 0, MeteredBlock::new(*alternative, 0)));
                    stack.push((*consequent, 0, MeteredBlock::new(*consequent, 0)));
                    continue 'traversing_blocks;
                }

                Instr::Call(_)
                | Instr::CallIndirect(_)
                | Instr::Br(_)
                | Instr::BrIf(_)
                | Instr::BrTable(_)
                | Instr::Return(_) => {
                    // Finalize current metered block and start a new one for the remainder.
                    blocks.finalize(std::mem::replace(
                        &mut metered_block,
                        MeteredBlock::new(seq_id, index + 1),
                    ));
                    continue 'traversing_instrs;
                }

                _ => continue 'traversing_instrs,
            }
        }

        // Check if we can merge the blocks.
        if let Some((_, _, upper)) = stack.last_mut() {
            match upper.merge_index {
                Some(index)
                    if upper.seq_id == metered_block.seq_id
                        && index == metered_block.start_index =>
                {
                    // Blocks can be merged, so overwrite upper.
                    *upper = metered_block;
                    continue 'traversing_blocks;
                }
                _ => {
                    // Blocks cannot be merged so treat as new block.
                }
            }
        }

        blocks.finalize(metered_block);
    }

    blocks.blocks
}

fn transform_function(
    func: &mut LocalFunction,
    gas_limit_global: GlobalId,
    gas_limit_exhausted_global: GlobalId,
) {
    // First pass: determine where metering instructions should be injected.
    let blocks = determine_metered_blocks(func);

    // Second pass: actually emit metering instructions in correct positions.
    let builder = func.builder_mut();
    for (seq_id, blocks) in blocks {
        let mut seq = builder.instr_seq(seq_id);
        let instrs = seq.instrs_mut();

        let original_instrs = std::mem::take(instrs);
        let new_instrs_len = instrs.len() + METERING_INSTRUCTION_COUNT * blocks.len();
        let mut new_instrs = Vec::with_capacity(new_instrs_len);

        let mut block_iter = blocks.into_iter().peekable();
        for (index, (instr, loc)) in original_instrs.into_iter().enumerate() {
            match block_iter.peek() {
                Some(block) if block.start_index == index => {
                    inject_metering(
                        builder,
                        &mut new_instrs,
                        block_iter.next().unwrap(),
                        gas_limit_global,
                        gas_limit_exhausted_global,
                    );
                }
                _ => {}
            }

            // Push original instruction.
            new_instrs.push((instr, loc));
        }

        let mut seq = builder.instr_seq(seq_id);
        let instrs = seq.instrs_mut();
        *instrs = new_instrs;
    }
}

/// Number of injected metering instructions (needed to calculate final instruction size).
const METERING_INSTRUCTION_COUNT: usize = 8;

fn inject_metering(
    builder: &mut FunctionBuilder,
    instrs: &mut Vec<(Instr, InstrLocId)>,
    block: MeteredBlock,
    gas_limit_global: GlobalId,
    gas_limit_exhausted_global: GlobalId,
) {
    let mut builder = builder.dangling_instr_seq(None);
    let seq = builder
        // if unsigned(globals[gas_limit]) < unsigned(block.cost) { throw(); }
        .global_get(gas_limit_global)
        .i64_const(block.cost as i64)
        .binop(BinaryOp::I64LtU)
        .if_else(
            None,
            |then| {
                then.i32_const(1)
                    .global_set(gas_limit_exhausted_global)
                    .unreachable();
            },
            |_else| {},
        )
        // globals[gas_limit] -= block.cost;
        .global_get(gas_limit_global)
        .i64_const(block.cost as i64)
        .binop(BinaryOp::I64Sub)
        .global_set(gas_limit_global);

    instrs.append(seq.instrs_mut());
}

#[cfg(test)]
mod test {
    macro_rules! test_transform {
        (name = $name:ident, source = $src:expr, expected = $expected:expr) => {
            #[test]
            fn $name() {
                let src = wat::parse_str($src).unwrap();
                let expected = wat::parse_str($expected).unwrap();

                let mut result_module = walrus::ModuleConfig::new()
                    .generate_producers_section(false)
                    .parse(&src)
                    .unwrap();

                super::transform(&mut result_module);

                let mut expected_module = walrus::ModuleConfig::new()
                    .generate_producers_section(false)
                    .parse(&expected)
                    .unwrap();

                assert_eq!(result_module.emit_wasm(), expected_module.emit_wasm());
            }
        };
    }

    test_transform! {
        name = simple,
        source = r#"
        (module
            (func (result i32)
                (i32.const 1)))
        "#,
        expected = r#"
        (module
            (func (result i32)
                (if
                    (i64.lt_u
                        (global.get 0)
                        (i64.const 1))
                    (then
                        (global.set 1
                            (i32.const 1))
                        (unreachable)))
                (global.set 0
                    (i64.sub
                        (global.get 0)
                        (i64.const 1)))
                (i32.const 1))
            (global (;0;) (mut i64) (i64.const 0))
            (global (;1;) (mut i32) (i32.const 0))
            (export "gas_limit" (global 0))
            (export "gas_limit_exhausted" (global 1)))
        "#
    }

    test_transform! {
        name = nested_blocks,
        source = r#"
        (module
            (func (result i32)
                (block
                    (block
                        (block
                            (i32.const 1)
                            (drop))))
                (i32.const 1)))
        "#,
        expected = r#"
        (module
            (func (result i32)
                (if
                    (i64.lt_u
                        (global.get 0)
                        (i64.const 6))
                    (then
                        (global.set 1
                            (i32.const 1))
                        (unreachable)))
                (global.set 0
                    (i64.sub
                        (global.get 0)
                        (i64.const 6)))
                (block
                    (block
                        (block
                            (i32.const 1)
                            (drop))))
                (i32.const 1))
            (global (;0;) (mut i64) (i64.const 0))
            (global (;1;) (mut i32) (i32.const 0))
            (export "gas_limit" (global 0))
            (export "gas_limit_exhausted" (global 1)))
        "#
    }

    test_transform! {
        name = nested_blocks_with_loop,
        source = r#"
        (module
            (func (result i32)
                (block
                    (block
                        (block
                            (i32.const 1)
                            (drop))
                        (loop
                            (i32.const 1)
                            (drop)
                            (i32.const 1)
                            (drop)
                            (br 0))))
                (i32.const 1)))
        "#,
        expected = r#"
        (module
            (func (result i32)
                (if
                    (i64.lt_u
                        (global.get 0)
                        (i64.const 6))
                    (then
                        (global.set 1
                            (i32.const 1))
                        (unreachable)))
                (global.set 0
                    (i64.sub
                        (global.get 0)
                        (i64.const 6)))
                (block
                    (block
                        (block
                            (i32.const 1)
                            (drop))
                        (loop
                            (if
                                (i64.lt_u
                                    (global.get 0)
                                    (i64.const 5))
                                (then
                                    (global.set 1
                                        (i32.const 1))
                                    (unreachable)))
                            (global.set 0
                                (i64.sub
                                    (global.get 0)
                                    (i64.const 5)))
                            (i32.const 1)
                            (drop)
                            (i32.const 1)
                            (drop)
                            (br 0))))
                (if
                    (i64.lt_u
                        (global.get 0)
                        (i64.const 1))
                    (then
                        (global.set 1
                            (i32.const 1))
                        (unreachable)))
                (global.set 0
                    (i64.sub
                        (global.get 0)
                        (i64.const 1)))
                (i32.const 1))
            (global (;0;) (mut i64) (i64.const 0))
            (global (;1;) (mut i32) (i32.const 0))
            (export "gas_limit" (global 0))
            (export "gas_limit_exhausted" (global 1)))
        "#
    }

    test_transform! {
        name = if_else,
        source = r#"
        (module
            (func (result i32)
                (i32.const 1)
                (if
                    (then
                        (i32.const 1)
                        (drop)
                        (i32.const 1)
                        (drop))
                    (else
                        (i32.const 1)
                        (drop)))
                (i32.const 1)))
        "#,
        expected = r#"
        (module
            (func (result i32)
                (if
                    (i64.lt_u
                        (global.get 0)
                        (i64.const 2))
                    (then
                        (global.set 1
                            (i32.const 1))
                        (unreachable)))
                (global.set 0
                    (i64.sub
                        (global.get 0)
                        (i64.const 2)))
                (i32.const 1)
                (if
                    (then
                        (if
                            (i64.lt_u
                                (global.get 0)
                                (i64.const 4))
                            (then
                                (global.set 1
                                    (i32.const 1))
                                (unreachable)))
                        (global.set 0
                            (i64.sub
                                (global.get 0)
                                (i64.const 4)))
                        (i32.const 1)
                        (drop)
                        (i32.const 1)
                        (drop)
                    )
                    (else
                        (if
                            (i64.lt_u
                                (global.get 0)
                                (i64.const 2))
                            (then
                                (global.set 1
                                    (i32.const 1))
                                (unreachable)))
                        (global.set 0
                            (i64.sub
                                (global.get 0)
                                (i64.const 2)))
                        (i32.const 1)
                        (drop)
                    )
                )
                (if
                    (i64.lt_u
                        (global.get 0)
                        (i64.const 1))
                    (then
                        (global.set 1
                            (i32.const 1))
                        (unreachable)))
                (global.set 0
                    (i64.sub
                        (global.get 0)
                        (i64.const 1)))
                (i32.const 1))
            (global (;0;) (mut i64) (i64.const 0))
            (global (;1;) (mut i32) (i32.const 0))
            (export "gas_limit" (global 0))
            (export "gas_limit_exhausted" (global 1)))
        "#
    }
}

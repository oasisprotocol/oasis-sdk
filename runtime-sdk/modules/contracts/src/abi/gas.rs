//! Gas metering instrumentation.
use walrus::{ir::*, FunctionBuilder, GlobalId, LocalFunction, Module};

/// Name of the exported global that holds the gas limit.
pub const EXPORT_GAS_LIMIT: &str = "gas_limit";
/// Name of the exported global that holds the gas limit exhausted flag.
pub const EXPORT_GAS_LIMIT_EXHAUSTED: &str = "gas_limit_exhausted";

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

    for (id, func) in module.funcs.iter_local_mut() {
        transform_function(func, gas_limit_global, gas_limit_exhausted_global);
    }
}

#[derive(Default)]
struct MeteredBlock {
    start_index: usize,
    cost: u64,
}

fn transform_function(
    func: &mut LocalFunction,
    gas_limit_global: GlobalId,
    gas_limit_exhausted_global: GlobalId,
) {
    let mut stack = vec![func.entry_block()];
    let builder = func.builder_mut();

    while let Some(seq_id) = stack.pop() {
        // Start processing a new block of instructions.
        let mut seq = builder.instr_seq(seq_id);
        let instrs = seq.instrs_mut();
        let mut blocks = vec![];
        let mut current_block = MeteredBlock::default();

        // First pass: determine where metering instructions should be injected and put any block
        // references on the stack so we can visit them.
        for (index, (instr, _)) in instrs.iter().enumerate() {
            // NOTE: Current instruction is always included in the current metered block.
            current_block.cost += 1; // TODO: Cost function based on instruction type.

            // Determine whether we need to end/start a metered block.
            match &instr {
                Instr::Block(_)
                | Instr::Loop(_)
                | Instr::Call(_)
                | Instr::CallIndirect(_)
                | Instr::Br(_)
                | Instr::BrIf(_)
                | Instr::BrTable(_)
                | Instr::Return(_)
                | Instr::IfElse(_) => {
                    // End current metered block and start a new one.
                    blocks.push(std::mem::replace(
                        &mut current_block,
                        MeteredBlock {
                            start_index: index + 1,
                            cost: 0,
                        },
                    ));
                }

                _ => {}
            }

            // Queue processing of child blocks.
            match &instr {
                Instr::Block(Block { seq }) | Instr::Loop(Loop { seq }) => {
                    stack.push(*seq);
                }

                Instr::IfElse(IfElse {
                    consequent,
                    alternative,
                }) => {
                    stack.push(*alternative);
                    stack.push(*consequent);
                }

                _ => {}
            }
        }

        // Push last metered block.
        blocks.push(current_block);

        // Second pass: actually emit metering instructions in correct positions.
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

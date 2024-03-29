use acir::{circuit::Circuit, native_types::WitnessMap};
use acvm::{
    pwg::{ACVMStatus, ErrorLocation, OpcodeResolutionError, ACVM},
    BlackBoxFunctionSolver,
};

use crate::errors::{ExecutionError, NargoError};

pub fn execute_circuit<B: BlackBoxFunctionSolver>(
    circuit: &Circuit,
    initial_witness: WitnessMap,
    blackbox_solver: &B,
) -> Result<WitnessMap, NargoError> {
    let mut acvm = ACVM::new(blackbox_solver, &circuit.opcodes, initial_witness);

    // This message should be resolved by a nargo foreign call only when we have an unsatisfied assertion.
    let assert_message: Option<String> = None;
    loop {
        let solver_status = acvm.solve();

        match solver_status {
            ACVMStatus::Solved => break,
            ACVMStatus::InProgress => {
                unreachable!("Execution should not stop while in `InProgress` state.")
            }
            ACVMStatus::Failure(error) => {
                let call_stack = match &error {
                    OpcodeResolutionError::UnsatisfiedConstrain {
                        opcode_location: ErrorLocation::Resolved(opcode_location),
                    } => Some(vec![*opcode_location]),
                    OpcodeResolutionError::BrilligFunctionFailed { call_stack, .. } => {
                        Some(call_stack.clone())
                    }
                    _ => None,
                };

                return Err(NargoError::ExecutionError(match call_stack {
                    Some(call_stack) => {
                        // First check whether we have a runtime assertion message that should be resolved on an ACVM failure
                        // If we do not have a runtime assertion message, we should check whether the circuit has any hardcoded
                        // messages associated with a specific `OpcodeLocation`.
                        // Otherwise return the provided opcode resolution error.
                        if let Some(assert_message) = assert_message {
                            ExecutionError::AssertionFailed(assert_message.to_owned(), call_stack)
                        } else if let Some(assert_message) = circuit.get_assert_message(
                            *call_stack.last().expect("Call stacks should not be empty"),
                        ) {
                            ExecutionError::AssertionFailed(assert_message.to_owned(), call_stack)
                        } else {
                            ExecutionError::SolvingError(error)
                        }
                    }
                    None => ExecutionError::SolvingError(error),
                }));
            }
            ACVMStatus::RequiresForeignCall(_foreign_call) => {}
        }
    }

    Ok(acvm.finalize())
}

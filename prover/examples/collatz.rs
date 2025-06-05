use anyhow::Result;
use clap::{value_parser, Parser};
use petravm_asm::isa::GenericISA;
use petravm_prover::{
    prover::{verify_proof, Prover},
    test_utils::{collatz, generate_collatz_trace},
};

#[derive(Debug, Parser)]
struct Args {
    /// The starting number of the Collatz sequence.
    #[arg(short, long, default_value_t = 100, value_parser = value_parser!(u32).range(1..))]
    n: u32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let _guard = petravm_asm::init_logger();

    let num_steps = collatz(args.n);

    println!(
        "Generating trace for collatz({}) in {} steps...",
        args.n, num_steps
    );

    let trace = generate_collatz_trace(args.n)?;
    trace.validate()?;

    let prover = Prover::new(Box::new(GenericISA));
    let (proof, statement, compiled_cs) = prover.prove(&trace)?;

    verify_proof(&statement, &compiled_cs, proof)?;

    Ok(())
}

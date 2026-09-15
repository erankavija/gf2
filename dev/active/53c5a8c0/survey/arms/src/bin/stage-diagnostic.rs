//! Stage diagnostic for the GF(2^m) dot product (jit:53c5a8c0).
//!
//! Emits one JSON record per stage per invocation on stdout. The launcher runs
//! this binary in repeated fresh processes, so the summary can resample across
//! processes rather than across repetitions inside one process. Each record
//! carries the stage identifier, the vector length, the repetition count the
//! probe amortised over and the total nanoseconds it observed; the summary
//! derives every rate from those fields.
//!
//! Before the first timing the binary checks that the reconstructed stages
//! compose to the value the consumer returns, and refuses to emit a record
//! otherwise: a decomposition that does not reproduce the consumer's result is
//! not a decomposition of the consumer.
//!
//! Usage: stage-diagnostic --elements <n> --seed <u64> --process <index>

use clmul_crossover_arms::stages::{Stage, StageFixture};
use clmul_crossover_arms::{banks, Case};

fn argument(name: &str) -> String {
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == name {
            return args.next().unwrap_or_else(|| {
                panic!("{name} needs a value");
            });
        }
    }
    panic!("missing required argument {name}");
}

fn main() {
    let elements: usize = argument("--elements").parse().expect("--elements is a count");
    let seed: u64 = argument("--seed").parse().expect("--seed is a u64");
    let process: u64 = argument("--process").parse().expect("--process is an index");

    let case = Case::FieldDot {
        count: elements,
        inner: 1,
        seed,
    };
    let bank = banks(&case, 0, 1).pop().expect("one bank");
    let mut fixture = StageFixture::new(elements, &bank);
    assert_eq!(
        fixture.reconstructed_value(),
        fixture.consumer_value(),
        "the reconstructed stages do not compose to the consumer's dot product"
    );
    assert_ne!(
        fixture.accumulator() >> clmul_crossover_arms::FIELD_DEGREE,
        0,
        "the reduction stage received an already reduced accumulator"
    );

    for stage in Stage::ALL {
        let sample = fixture.measure(stage);
        let record = serde_json::json!({
            "schema": "clmul-crossover-stage-sample-v1",
            "process": process,
            "seed": seed,
            "sample": sample,
        });
        println!("{record}");
    }
}

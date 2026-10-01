//! Runs the BCH walkthrough examples, so their assertions stay part of the
//! fast tier rather than only compiling under `--all-targets`.

#[path = "../examples/bch_binary_quickstart.rs"]
mod bch_binary_quickstart;
#[path = "../examples/bch_nonbinary_explicit.rs"]
mod bch_nonbinary_explicit;

#[test]
fn binary_quickstart_runs() {
    bch_binary_quickstart::main().expect("the binary quickstart example succeeds");
}

#[test]
fn nonbinary_explicit_runs() {
    bch_nonbinary_explicit::main().expect("the nonbinary explicit example succeeds");
}

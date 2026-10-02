//! Runs the field linear-algebra example at small dimensions, so its identity
//! checks stay part of the fast tier.

#[allow(dead_code)] // `main` reads the process arguments; the test drives `run`.
#[path = "../examples/field_linear_algebra.rs"]
mod field_linear_algebra;

#[test]
fn field_linear_algebra_example_runs() {
    let args = ["--n", "65", "--charpoly-n", "33", "--seed", "7"].map(String::from);
    let params = field_linear_algebra::parse_args(args).expect("the arguments parse");
    field_linear_algebra::run(&params).expect("every identity holds");
}

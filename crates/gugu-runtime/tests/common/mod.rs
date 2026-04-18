// Each tests/*.rs is its own crate, so unused helpers warn per-file.
#![allow(dead_code)]

use gugu_parser::parse;
use gugu_runtime::{LowerError, exec, lower, read_outputs};

pub fn run(src: &str) -> Vec<String> {
    let program = parse(src).expect("parse failed");
    let res = exec(&program).expect("exec failed");
    read_outputs(&res)
}

pub fn run_one(src: &str) -> String {
    run(src).into_iter().next().unwrap_or_default()
}

pub fn lower_err(src: &str) -> LowerError {
    let program = parse(src).expect("parse failed");
    match lower(&program) {
        Err(e) => e,
        Ok(_) => panic!("expected lowering error"),
    }
}

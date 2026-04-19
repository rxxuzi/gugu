use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("usage: gugu <command> [args]");
        eprintln!();
        eprintln!("commands:");
        eprintln!("  run [--trace] <file.gu>   Run a .gu program");
        eprintln!("  test <file.gu>            Run every `test \"...\"` block");
        eprintln!("  check <file.gu>           Parse and type-check (no execution)");
        eprintln!("  lex <file.gu>             Dump tokens");
        process::exit(1);
    }

    let cmd = &args[1];
    match cmd.as_str() {
        "run" => cmd_run(&args[2..]),
        "test" => cmd_test(&args[2..]),
        "check" => cmd_check(&args[2..]),
        "lex" => cmd_lex(&args[2..]),
        other => {
            eprintln!("unknown command: {other}");
            process::exit(1);
        }
    }
}

fn read_file(args: &[String]) -> String {
    if args.is_empty() {
        eprintln!("error: no input file");
        process::exit(1);
    }
    let path = &args[0];
    match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            process::exit(1);
        }
    }
}

fn cmd_run(args: &[String]) {
    let (trace, rest) = match args.split_first() {
        Some((f, rest)) if f == "--trace" => (true, rest),
        _ => (false, args),
    };
    let src = read_file(rest);

    let program = match gugu_parser::parse(&src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse error: {e}");
            process::exit(1);
        }
    };

    // --trace prints every bloom; without it, only blooms touching a `?`
    // inspected atom surface. Both route through exec_traced.
    let result = gugu_runtime::exec_traced(&program, |t| {
        if trace {
            eprintln!("[bloom {:>4}] @{} >< @{}", t.count, t.lhs, t.rhs);
        } else if t.inspected {
            eprintln!("[inspect {:>4}] @{} >< @{}", t.count, t.lhs, t.rhs);
        }
    });

    match result {
        Ok(result) => {
            for line in gugu_runtime::read_outputs(&result) {
                println!("{line}");
            }
            eprintln!("# {} bloom(s)", result.blooms);
        }
        Err(e) => {
            eprintln!("runtime error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_test(args: &[String]) {
    let src = read_file(args);
    let program = match gugu_parser::parse(&src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse error: {e}");
            process::exit(1);
        }
    };

    let outcomes = gugu_runtime::run_tests(&program);
    if outcomes.is_empty() {
        eprintln!("no `test` blocks in this file");
        return;
    }

    let (mut pass, mut fail) = (0u32, 0u32);
    for o in &outcomes {
        if o.pass {
            pass += 1;
            println!("pass  {:?}", o.label);
        } else {
            fail += 1;
            println!("FAIL  {:?}", o.label);
            println!("    lhs = {}", o.lhs);
            println!("    rhs = {}", o.rhs);
        }
    }
    println!("# {pass} passed, {fail} failed");
    if fail > 0 {
        process::exit(1);
    }
}

fn cmd_check(args: &[String]) {
    let src = read_file(args);

    match gugu_parser::parse(&src) {
        Ok(program) => {
            let agents = program
                .items
                .iter()
                .filter(|i| matches!(i, gugu_parser::ast::TopLevel::Agent(_)))
                .count();
            let rules = program
                .items
                .iter()
                .filter(|i| matches!(i, gugu_parser::ast::TopLevel::Rule(_)))
                .count();
            let gens = program.gens.len();
            println!("ok: {agents} agent(s), {rules} rule(s), {gens} @GEN block(s)");
        }
        Err(e) => {
            eprintln!("parse error: {e}");
            process::exit(1);
        }
    }
}

fn cmd_lex(args: &[String]) {
    let src = read_file(args);

    match gugu_lexer::lex(&src) {
        Ok(tokens) => {
            for tok in &tokens {
                println!("{:>4}..{:<4} {:?}", tok.span.start, tok.span.end, tok.kind);
            }
            println!("# {} token(s)", tokens.len());
        }
        Err(e) => {
            eprintln!("lex error: {e}");
            process::exit(1);
        }
    }
}

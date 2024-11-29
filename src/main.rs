#![cfg(not(tarpaulin_include))]
use cargo_tarpaulin::args::CargoTarpaulinCli;
use cargo_tarpaulin::cargo::{rust_flags, rustdoc_flags};
use cargo_tarpaulin::config::{Color, Config, ConfigWrapper};
use cargo_tarpaulin::{run, setup_logging};
use std::collections::HashMap;
use tracing::{info, trace};

fn print_env(seen_rustflags: HashMap<String, Vec<String>>, prefix: &str, default_val: &str) {
    info!("Printing `{}`", prefix);
    if seen_rustflags.is_empty() {
        info!("No configs provided printing default RUSTFLAGS");
        println!("{prefix}={default_val}");
    } else if seen_rustflags.len() == 1 {
        let flags = seen_rustflags.keys().next().unwrap();
        println!(r#"{prefix}="{flags}""#);
    } else {
        for (k, v) in &seen_rustflags {
            info!("RUSTFLAGS for configs {:?}", v);
            println!(r#"{prefix}="{k}""#);
        }
    }
}

fn main() -> Result<(), String> {
    let args = CargoTarpaulinCli::from_args();

    let logging_args = args.config.logging;
    setup_logging(
        logging_args.color.unwrap_or(Color::Auto),
        logging_args.debug,
        logging_args.verbose,
        logging_args.stderr,
    );

    let config = ConfigWrapper::from(args.config);

    trace!("Config vector: {:#?}", config);

    let print_flags_args = args.print_flags;
    if print_flags_args.print_rust_flags {
        print_flags(&config, rust_flags, "RUSTFLAGS");
        return Ok(());
    }

    if print_flags_args.print_rustdoc_flags {
        print_flags(&config, rustdoc_flags, "RUSTDOCFLAGS");
        return Ok(());
    }

    trace!("Debug mode activated");

    // Since this is the last function we run and don't do any error mitigations (other than
    // printing the error to the user it's fine to unwrap here
    run(&config.0).map_err(|e| e.to_string())
}

fn print_flags<F>(config: &ConfigWrapper, flags_fn: F, prefix: &str)
where
    F: Fn(&Config) -> String,
{
    let mut seen_flags = HashMap::new();
    for config in &config.0 {
        if config.name == "report" {
            continue;
        }

        let flags = flags_fn(config);
        seen_flags
            .entry(flags)
            .or_insert_with(Vec::new)
            .push(config.name.clone());
    }

    let default = Config::default();
    print_env(seen_flags, prefix, &flags_fn(&default));
}

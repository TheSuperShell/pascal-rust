use clap::{arg, command};
use pascal_rust::compile_into_file;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init_logging(stack: bool, scope: bool) {
    let directives = format!(
        "pascal=warn,pascal::semantic={},pascal::interp={}",
        match scope {
            true => "debug",
            false => "warn",
        },
        match stack {
            true => "debug",
            false => "warn",
        }
    );
    let filter = EnvFilter::new(directives);
    fmt().with_env_filter(filter).compact().init();
}

fn main() {
    let matches = command!()
        .arg(arg!(<path> "Path of the scrip"))
        .arg(arg!(<target> "Compilation target"))
        .arg(arg!(--scope "Turn on scope logging"))
        .arg(arg!(--stack "Turn on stack logging"))
        .get_matches();
    let path = matches.get_one::<String>("path").unwrap();
    let target = matches.get_one::<String>("target").unwrap();
    init_logging(matches.get_flag("stack"), matches.get_flag("scope"));
    match compile_into_file(path, target) {
        Err(e) => {
            error!(target: "pascal", "{e}");
            std::process::exit(1);
        }
        Ok(_) => {}
    };
}

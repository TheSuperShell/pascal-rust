use clap::{Command, arg, command};
use pascal_rust::{compile_into_file, interprete};
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init_logging(stack: bool, scope: bool) {
    let directives = format!(
        "pascal=warn,pascal::semantic={},pascal::interp={},pascal::compiler={}",
        match scope {
            true => "debug",
            false => "warn",
        },
        match stack {
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
    let cmd = command!()
        .subcommand(
            Command::new("compile")
                .about("Compile a pascal file")
                .arg(arg!(<path> "Path of the scrip"))
                .arg(arg!(<target> "Compilation target"))
                .arg(arg!(--scope "Turn on scope logging"))
                .arg(arg!(--stack "Turn on stack logging")),
        )
        .subcommand(
            Command::new("interp")
                .about("Interperet a pascal file")
                .arg(arg!(<path> "Path of the script"))
                .arg(arg!(--scope "Turn on scope logging"))
                .arg(arg!(--stack "Turn on stack logging")),
        );
    let help_message = cmd.get_about().cloned();
    let matches = cmd.get_matches();
    match matches.subcommand() {
        Some(("compile", sub_m)) => {
            init_logging(sub_m.get_flag("stack"), sub_m.get_flag("scope"));
            let path = sub_m.get_one::<String>("path").unwrap();
            let target = sub_m.get_one::<String>("target").unwrap();
            match compile_into_file(path, target) {
                Err(e) => {
                    error!(target: "pascal", "{e}");
                    std::process::exit(1);
                }
                Ok(_) => {}
            };
        }
        Some(("interp", sub_m)) => {
            init_logging(sub_m.get_flag("stack"), sub_m.get_flag("scope"));
            let path = sub_m.get_one::<String>("path").unwrap();
            match interprete(path) {
                Err(e) => {
                    error!(target: "pascal", "{e}");
                    std::process::exit(1)
                }
                Ok(_) => {}
            }
        }
        _ => println!("{}", help_message.unwrap_or_default()),
    }
}

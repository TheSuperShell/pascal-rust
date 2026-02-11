use dotenv::dotenv;
use pascal_rust::interprete;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init_logging() {
    dotenv().ok();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("pascal::semantic=info,pascal::interp=warn"));
    fmt().with_env_filter(filter).compact().init();
}

fn main() {
    init_logging();
    match interprete("examples/factorial.pas") {
        Err(e) => error!("{e}"),
        _ => (),
    }
}

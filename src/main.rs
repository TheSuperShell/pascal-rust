use pascal_rust::interprete;

fn main() {
    match interprete("examples/factorial.pas") {
        Err(e) => println!("{e}"),
        _ => (),
    }
}

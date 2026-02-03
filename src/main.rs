use pascal_rust::interprete;

fn main() {
    match interprete() {
        Err(e) => println!("{e}"),
        _ => (),
    }
}

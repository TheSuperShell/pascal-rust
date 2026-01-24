mod error;
mod lexer;
mod parser;
mod tokens;

use lexer::Lexer;

use crate::parser::Parser;

fn main() {
    let source_code = std::fs::read_to_string("examples/factorial.pas").expect("file should exist");
    let lexer = Lexer::new(&source_code);
    let mut parser = Parser::new(lexer).unwrap();
    let tree = parser.parse().unwrap();
    println!(
        "program={:?}\nexpressions={:?}\nstatements={:?}\ntypes={:?}",
        tree.program, tree.expr_pool, tree.stmt_pool, tree.type_pool
    );
}

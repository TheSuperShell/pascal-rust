mod error;
mod lexer;
mod parser;
mod semantic_analyzer;
mod symbols;
mod tokens;
mod utils;

use lexer::Lexer;

use crate::{parser::Parser, semantic_analyzer::SemanticAnalyzer};

fn main() {
    let source_code = std::fs::read_to_string("examples/factorial.pas").expect("file should exist");
    let lexer = Lexer::new(&source_code);
    let mut parser = Parser::new(lexer).unwrap();
    let tree = parser.parse().unwrap();
    println!("{tree}");
    let semantic_analyzer = SemanticAnalyzer::new();
    let semantic_metadata = semantic_analyzer.analyze(&tree).unwrap();
    println!("{:?}", semantic_metadata);
}

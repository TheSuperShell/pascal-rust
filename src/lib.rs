mod builtins;
mod error;
mod interpreter;
mod lexer;
mod parser;
mod semantic_analyzer;
mod symbols;
mod tokens;
mod utils;

use std::path::Path;

use lexer::Lexer;

use crate::{
    error::Error, interpreter::Interpreter, parser::Parser, semantic_analyzer::SemanticAnalyzer,
};

pub fn interprete<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    let source_code = std::fs::read_to_string(path).expect("file should exist");
    let lexer = Lexer::new(&source_code);
    let parser = Parser::new(lexer)?;
    let tree = parser.parse()?;
    // println!("{tree}");
    let semantic_analyzer = SemanticAnalyzer::new();
    let semantic_metadata = semantic_analyzer.analyze(&tree)?;
    Interpreter::new().interperet(&tree, &semantic_metadata)?;
    Ok(())
}

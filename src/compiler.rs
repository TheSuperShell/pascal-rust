use crate::{
    error::Error,
    parser::{Expr, ExprRef, Stmt, StmtRef, Tree},
    tokens::TokenType,
};
use std::fmt::Write;

pub struct Compiler {
    bytes: String,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            bytes: String::new(),
        }
    }

    pub fn compile(mut self, tree: &Tree) -> Result<String, Error> {
        self.visit_stmt(&tree.program, tree)?;
        Ok(self.bytes)
    }

    fn visit_stmt(&mut self, stmt: &StmtRef, tree: &Tree) -> Result<(), Error> {
        match tree.stmt_pool.get(*stmt) {
            Stmt::Program { name: _, block } => {
                writeln!(self.bytes, "section .data")?;
                writeln!(self.bytes, "fmt db \"%d\", 10, 0")?;
                writeln!(self.bytes)?;
                writeln!(self.bytes, "section .text")?;
                writeln!(self.bytes, "global main")?;
                writeln!(self.bytes, "extern printf")?;
                writeln!(self.bytes)?;

                writeln!(self.bytes, "main:")?;
                writeln!(self.bytes, "sub rsp, 40")?;
                writeln!(self.bytes)?;
                self.visit_stmt(block, tree)?;
                writeln!(self.bytes, "add rsp, 40")?;
                writeln!(self.bytes, "xor eax, eax")?;
                writeln!(self.bytes, "ret")?;
                Ok(())
            }
            Stmt::Block {
                declarations: _,
                statements,
            } => self.visit_stmt(statements, tree),
            Stmt::Compound(stmts) => stmts.iter().try_for_each(|v| {
                self.visit_stmt(v, tree)?;
                writeln!(self.bytes)?;
                Ok(())
            }),
            Stmt::NoOp => Ok(()),
            Stmt::Call { call } => self.visit_call(call, tree),
            _ => todo!(),
        }
    }

    fn visit_call(&mut self, call: &ExprRef, tree: &Tree) -> Result<(), Error> {
        match tree.expr_pool.get(*call) {
            Expr::Call { name, args } => {
                if name.lexem(tree.source_code).to_lowercase() != "writeln" {
                    unimplemented!("Only writeln is supported for now")
                }
                writeln!(self.bytes, "; call writeln")?;
                for arg in args {
                    self.visit_expr(arg, tree)?;
                    writeln!(self.bytes, "pop rax")?;
                    writeln!(self.bytes, "mov rcx, fmt")?;
                    writeln!(self.bytes, "mov rdx, rax")?;
                    writeln!(self.bytes, "call printf")?;
                }
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn visit_expr(&mut self, expr: &ExprRef, tree: &Tree) -> Result<(), Error> {
        match tree.expr_pool.get(*expr) {
            Expr::LiteralInteger(i) => {
                writeln!(self.bytes, "push {i}")?;
                Ok(())
            }
            Expr::UnaryOp { op, expr } => {
                self.visit_expr(expr, tree)?;
                match op {
                    TokenType::Plus => {}
                    TokenType::Minus => {
                        writeln!(self.bytes, "pop rax")?;
                        writeln!(self.bytes, "neg rax")?;
                        writeln!(self.bytes, "push rax")?;
                    }
                    _ => todo!(),
                }
                Ok(())
            }
            Expr::BinOp { op, left, right } => {
                self.visit_expr(left, tree)?;
                self.visit_expr(right, tree)?;
                writeln!(self.bytes, "pop rbx")?;
                writeln!(self.bytes, "pop rax")?;
                match op {
                    TokenType::Plus => {
                        writeln!(self.bytes, "add rax, rbx")?;
                    }
                    TokenType::Minus => {
                        writeln!(self.bytes, "sub rax, rbx")?;
                    }
                    TokenType::Mul => {
                        writeln!(self.bytes, "imul rax, rbx")?;
                    }
                    TokenType::IntegerDiv => {
                        writeln!(self.bytes, "xor rdx, rdx")?;
                        writeln!(self.bytes, "div rbx")?;
                    }
                    _ => todo!(),
                }
                writeln!(self.bytes, "push rax")?;
                Ok(())
            }
            _ => todo!(),
        }
    }
}

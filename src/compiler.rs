use crate::{
    error::Error,
    parser::{Expr, ExprRef, Stmt, StmtRef, Tree},
    tokens::TokenType,
};
use std::fmt::Display;
use std::io::Write;

#[derive(Debug, Clone)]
pub enum Value<'a> {
    Integer(i32),
    Rax,
    Rbx,
    Rcx,
    Rdx,

    Rsp,

    Eax,

    Variable(&'a str),
}

impl<'a> Display for Value<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(i) => write!(f, "{i}"),
            Value::Rax => write!(f, "rax"),
            Value::Rbx => write!(f, "rbx"),
            Value::Rcx => write!(f, "rcx"),
            Value::Rdx => write!(f, "rdx"),
            Value::Rsp => write!(f, "rsp"),
            Value::Eax => write!(f, "eax"),
            Value::Variable(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Command<'a> {
    Push(Value<'a>),
    Pop(Value<'a>),
    Mov { dst: Value<'a>, src: Value<'a> },
    Add { dst: Value<'a>, src: Value<'a> },
    Sub { dst: Value<'a>, src: Value<'a> },
    Imul { dst: Value<'a>, src: Value<'a> },
    Div(Value<'a>),
    Neg { dst: Value<'a> },
    Xor { dst: Value<'a>, src: Value<'a> },
    Call { name: &'a str },
    Ret,
}

#[derive(Debug, Clone)]
pub struct Assambler<'a, W: Write> {
    output: W,
    commands: Vec<Command<'a>>,
}

impl<'a, W: Write> Assambler<'a, W> {
    pub fn new(out: W) -> Self {
        Self {
            output: out,
            commands: Vec::new(),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::mem::replace(&mut self.commands, Vec::new())
            .into_iter()
            .try_for_each(|cmd| match cmd {
                Command::Push(v) => {
                    writeln!(self.output, "push {}", v)
                }
                Command::Mov { dst, src } => {
                    writeln!(self.output, "mov {}, {}", dst, src)
                }
                Command::Add { dst, src } => {
                    writeln!(self.output, "add {}, {}", dst, src)
                }
                Command::Sub { dst, src } => {
                    writeln!(self.output, "sub {}, {}", dst, src)
                }
                Command::Imul { dst, src } => {
                    writeln!(self.output, "imul {}, {}", dst, src)
                }
                Command::Div(v) => {
                    writeln!(self.output, "div {}", v)
                }
                Command::Neg { dst } => {
                    writeln!(self.output, "neg {}", dst)
                }
                Command::Ret => {
                    writeln!(self.output, "ret")
                }
                Command::Call { name } => {
                    writeln!(self.output, "call {}", name)
                }
                Command::Xor { dst, src } => {
                    writeln!(self.output, "xor {}, {}", dst, src)
                }
                Command::Pop(v) => {
                    writeln!(self.output, "pop {}", v)
                }
            })
    }

    pub fn push_cmd(&mut self, cmd: Command<'a>) {
        self.commands.push(cmd);
    }

    pub fn directive(&mut self, directive: &str) -> std::io::Result<()> {
        self.flush()?;
        writeln!(self.output, "{}", directive)
    }

    pub fn label(&mut self, label: &str) -> std::io::Result<()> {
        self.flush()?;
        writeln!(self.output, "{}:", label)
    }

    pub fn comment(&mut self, comment: &str) -> std::io::Result<()> {
        self.flush()?;
        writeln!(self.output, "; {}", comment)
    }

    pub fn newline(&mut self) -> std::io::Result<()> {
        self.flush()?;
        writeln!(self.output)
    }

    pub fn output(mut self) -> Result<W, std::io::Error> {
        self.flush()?;
        Ok(self.output)
    }
}

pub struct Compiler<W: Write> {
    asm: Assambler<'static, W>,
}

impl<W: Write> Compiler<W> {
    pub fn new(output: W) -> Result<Self, Error> {
        Ok(Compiler {
            asm: Assambler::new(output),
        })
    }

    pub fn compile(mut self, tree: &Tree) -> Result<W, Error> {
        self.visit_stmt(&tree.program, tree)?;
        Ok(self.asm.output()?)
    }

    fn visit_stmt(&mut self, stmt: &StmtRef, tree: &Tree) -> Result<(), Error> {
        match tree.stmt_pool.get(*stmt) {
            Stmt::Program { name: _, block } => {
                self.asm.directive("section .data")?;
                self.asm.directive("fmt db \"%d\", 10, 0")?;
                self.asm.newline()?;
                self.asm.directive("section .text")?;
                self.asm.directive("global main")?;
                self.asm.directive("extern printf")?;
                self.asm.newline()?;
                self.asm.comment("main function entry point")?;
                self.asm.label("main")?;
                self.asm.push_cmd(Command::Sub {
                    dst: Value::Rsp,
                    src: Value::Integer(40),
                });
                self.visit_stmt(block, tree)?;
                self.asm.push_cmd(Command::Add {
                    dst: Value::Rsp,
                    src: Value::Integer(40),
                });
                self.asm.push_cmd(Command::Xor {
                    dst: Value::Eax,
                    src: Value::Eax,
                });
                self.asm.push_cmd(Command::Ret);
                Ok(())
            }
            Stmt::Block {
                declarations: _,
                statements,
            } => self.visit_stmt(statements, tree),
            Stmt::Compound(stmts) => stmts.iter().try_for_each(|v| {
                self.visit_stmt(v, tree)?;
                self.asm.newline()?;
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
                self.asm.comment("call writeln")?;
                for arg in args {
                    self.visit_expr(arg, tree)?;
                    self.asm.push_cmd(Command::Pop(Value::Rax));
                    self.asm.push_cmd(Command::Mov {
                        dst: Value::Rcx,
                        src: Value::Variable("fmt"),
                    });
                    self.asm.push_cmd(Command::Mov {
                        dst: Value::Rdx,
                        src: Value::Rax,
                    });
                    self.asm.push_cmd(Command::Call { name: "printf" });
                }
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn visit_expr(&mut self, expr: &ExprRef, tree: &Tree) -> Result<(), Error> {
        match tree.expr_pool.get(*expr) {
            Expr::LiteralInteger(i) => {
                self.asm.push_cmd(Command::Push(Value::Integer(*i)));
                Ok(())
            }
            Expr::UnaryOp { op, expr } => {
                self.visit_expr(expr, tree)?;
                match op {
                    TokenType::Plus => {}
                    TokenType::Minus => {
                        self.asm.push_cmd(Command::Pop(Value::Rax));
                        self.asm.push_cmd(Command::Neg { dst: Value::Rax });
                        self.asm.push_cmd(Command::Push(Value::Rax));
                    }
                    _ => todo!(),
                }
                Ok(())
            }
            Expr::BinOp { op, left, right } => {
                self.visit_expr(left, tree)?;
                self.visit_expr(right, tree)?;
                self.asm.push_cmd(Command::Pop(Value::Rbx));
                self.asm.push_cmd(Command::Pop(Value::Rax));
                match op {
                    TokenType::Plus => {
                        self.asm.push_cmd(Command::Add {
                            dst: Value::Rax,
                            src: Value::Rbx,
                        });
                    }
                    TokenType::Minus => {
                        self.asm.push_cmd(Command::Sub {
                            dst: Value::Rax,
                            src: Value::Rbx,
                        });
                    }
                    TokenType::Mul => {
                        self.asm.push_cmd(Command::Imul {
                            dst: Value::Rax,
                            src: Value::Rbx,
                        });
                    }
                    TokenType::IntegerDiv => {
                        self.asm.push_cmd(Command::Xor {
                            dst: Value::Rdx,
                            src: Value::Rdx,
                        });
                        self.asm.push_cmd(Command::Div(Value::Rbx));
                    }
                    _ => todo!(),
                }
                self.asm.push_cmd(Command::Push(Value::Rax));
                Ok(())
            }
            _ => todo!(),
        }
    }
}

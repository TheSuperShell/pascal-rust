use crate::{
    error::Error,
    parser::{Decl, Expr, ExprRef, Stmt, StmtRef, Tree},
    tokens::TokenType,
};
use std::io::Write;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Value<'a> {
    Integer(i32),
    Rax,
    Rbx,
    Rcx,
    Rdx,
    Rbp,

    Rsp,

    Eax,
    Ebx,
    Edx,

    Variable(&'a str),
}

impl<'a> Display for Value<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(i) => write!(f, "{i}"),
            Value::Rbp => write!(f, "rbp"),
            Value::Rax => write!(f, "rax"),
            Value::Rbx => write!(f, "rbx"),
            Value::Rcx => write!(f, "rcx"),
            Value::Rdx => write!(f, "rdx"),
            Value::Rsp => write!(f, "rsp"),
            Value::Eax => write!(f, "eax"),
            Value::Ebx => write!(f, "ebx"),
            Value::Edx => write!(f, "edx"),
            Value::Variable(v) => write!(f, "{}", v),
        }
    }
}

impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(i1), Value::Integer(i2)) => i1 == i2,
            (Value::Variable(v1), Value::Variable(v2)) => v1 == v2,
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryAddress<'a> {
    base: Value<'a>,
    offset: i32,
}

impl<'a> MemoryAddress<'a> {
    pub fn new(base: Value<'a>) -> Self {
        Self { base, offset: 0 }
    }
    pub fn with_offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }
}

impl<'a> Display for MemoryAddress<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.offset == 0 {
            write!(f, "[{}]", self.base)
        } else {
            write!(f, "qword [{} - {}]", self.base, self.offset)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand<'a> {
    Register(Value<'a>),
    Memory(MemoryAddress<'a>),
}

impl Display for Operand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::Register(r) => write!(f, "{}", r),
            Operand::Memory(m) => write!(f, "{}", m),
        }
    }
}

impl<'a> Into<Operand<'a>> for Value<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Register(self)
    }
}

impl<'a> Value<'a> {
    pub fn with_offset(self, offset: i32) -> Operand<'a> {
        Operand::Memory(MemoryAddress::new(self).with_offset(offset))
    }
}

#[derive(Debug, Clone)]
pub enum Command<'a> {
    Push(Operand<'a>),
    Pop(Operand<'a>),
    Mov { dst: Operand<'a>, src: Operand<'a> },
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
    peephole: bool,
    output: W,
    commands: Vec<Command<'a>>,
}

impl<'a, W: Write> Assambler<'a, W> {
    pub fn new(out: W, peekhole: bool) -> Self {
        Self {
            peephole: peekhole,
            output: out,
            commands: Vec::new(),
        }
    }

    /// Peephole optimization to remove redundant push-pop pairs and similar patterns
    /// - pattern:
    ///   push X
    ///   pop Y
    ///   =>
    ///   mov Y, X
    ///
    /// - pattern:
    ///   push X
    ///   push Y
    ///   pop A
    ///   pop B
    ///   =>
    ///   mov A, Y
    ///   mov B, X
    ///
    /// - pattern:
    ///   push X
    ///   push Y
    ///   pop A
    ///   pop X
    ///   =>
    ///   mov A, Y
    fn optimize(&mut self) {
        let mut optimized = Vec::with_capacity(self.commands.len());
        let mut i = 0;
        while i < self.commands.len() {
            match (
                &self.commands.get(i),
                &self.commands.get(i + 1),
                &self.commands.get(i + 2),
                &self.commands.get(i + 3),
            ) {
                (
                    Some(Command::Push(o1)),
                    Some(Command::Push(o2)),
                    Some(Command::Pop(o3)),
                    Some(Command::Pop(o4)),
                ) => {
                    if o3 != o2 {
                        optimized.push(Command::Mov {
                            dst: o3.clone(),
                            src: o2.clone(),
                        });
                    }
                    if o1 != o4 {
                        optimized.push(Command::Mov {
                            dst: o4.clone(),
                            src: o1.clone(),
                        });
                    }
                    i += 4;
                }
                (Some(Command::Push(o1)), Some(Command::Pop(o2)), _, _) => {
                    if o1 != o2 {
                        optimized.push(Command::Mov {
                            dst: o2.clone(),
                            src: o1.clone(),
                        });
                    }
                    i += 2;
                }
                _ => {
                    optimized.push(self.commands[i].clone());
                    i += 1;
                }
            }
        }
        self.commands = optimized;
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.peephole {
            self.optimize();
        }
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
    locals: HashMap<String, i32>,
}

impl<W: Write> Compiler<W> {
    pub fn new(output: W) -> Result<Self, Error> {
        Ok(Compiler {
            asm: Assambler::new(output, true),
            locals: HashMap::new(),
        })
    }

    fn offset(&self, ind: i32) -> i32 {
        (self.locals.len() as i32 + 1 - ind) * 8
    }

    fn var_offset(&self, name: &str) -> Option<i32> {
        self.locals.get(name).map(|ind| self.offset(*ind))
    }

    pub fn compile(mut self, tree: &Tree) -> Result<W, Error> {
        self.visit_stmt(&tree.program, tree)?;
        Ok(self.asm.output()?)
    }

    fn visit_decl(&mut self, decl: &Decl, tree: &Tree) -> Result<Option<ExprRef>, Error> {
        match decl {
            Decl::VarDecl {
                default_value,
                var,
                type_node: _,
            } => {
                let var_name = match tree.expr_pool.get(*var) {
                    Expr::Var { name } => name.lexem(tree.source_code),
                    _ => unreachable!(),
                };
                let offset = self.locals.len() as i32 + 1;
                self.locals.insert(var_name.to_string(), offset);
                Ok(*default_value)
            }
            _ => unimplemented!(),
        }
    }

    fn assign_default(&mut self, ind: i32, value: &ExprRef, tree: &Tree) -> Result<(), Error> {
        let offset = self.offset(ind);
        self.visit_expr(value, tree)?;
        self.asm
            .push_cmd(Command::Pop(Value::Rbp.with_offset(offset)));
        Ok(())
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
                self.visit_stmt(block, tree)?;
                self.asm.push_cmd(Command::Xor {
                    dst: Value::Eax,
                    src: Value::Eax,
                });
                self.asm.push_cmd(Command::Ret);
                Ok(())
            }
            Stmt::Block {
                declarations,
                statements,
            } => {
                self.asm.comment("block")?;
                self.asm.push_cmd(Command::Push(Value::Rbp.into()));
                self.asm.push_cmd(Command::Mov {
                    dst: Value::Rbp.into(),
                    src: Value::Rsp.into(),
                });
                let defaults = declarations
                    .iter()
                    .map(|decl| self.visit_decl(decl, tree))
                    .collect::<Result<Vec<_>, Error>>()?;
                let local_size = (self.locals.len() as i32) * 8;
                let aligned_local_size = ((local_size + 15) / 16) * 16;
                self.asm.push_cmd(Command::Sub {
                    dst: Value::Rsp,
                    src: Value::Integer(32 + aligned_local_size),
                });
                defaults
                    .into_iter()
                    .enumerate()
                    .try_for_each(|(i, v)| match v {
                        Some(value) => self.assign_default(i as i32 + 1, &value, tree),
                        None => Ok(()),
                    })?;
                self.visit_stmt(statements, tree)?;

                self.asm.directive("leave")?;
                self.asm.comment("end block")?;
                Ok(())
            }
            Stmt::Compound(stmts) => stmts.iter().try_for_each(|v| {
                self.visit_stmt(v, tree)?;
                self.asm.newline()?;
                Ok(())
            }),
            Stmt::NoOp => Ok(()),
            Stmt::Call { call } => self.visit_call(call, tree),
            Stmt::Assign { left, right } => {
                let var_name = match tree.expr_pool.get(*left) {
                    Expr::Var { name } => name.lexem(tree.source_code),
                    _ => unreachable!(),
                };
                let offset = self.var_offset(var_name).expect("expected value to exist");
                self.visit_expr(right, tree)?;
                self.asm
                    .push_cmd(Command::Pop(Value::Rbp.with_offset(offset)));
                Ok(())
            }
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
                    self.asm.push_cmd(Command::Pop(Value::Rax.into()));
                    self.asm.push_cmd(Command::Mov {
                        dst: Value::Rcx.into(),
                        src: Value::Variable("fmt").into(),
                    });
                    self.asm.push_cmd(Command::Mov {
                        dst: Value::Rdx.into(),
                        src: Value::Rax.into(),
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
            Expr::Var { name } => {
                let var_name = name.lexem(tree.source_code);
                self.asm
                    .push_cmd(Command::Push(Value::Rbp.with_offset(
                        self.var_offset(var_name).expect("expected value to exist"),
                    )));
                Ok(())
            }
            Expr::LiteralInteger(i) => {
                self.asm.push_cmd(Command::Push(Value::Integer(*i).into()));
                Ok(())
            }
            Expr::UnaryOp { op, expr } => {
                self.visit_expr(expr, tree)?;
                match op {
                    TokenType::Plus => {}
                    TokenType::Minus => {
                        self.asm.push_cmd(Command::Pop(Value::Rax.into()));
                        self.asm.push_cmd(Command::Neg { dst: Value::Rax });
                        self.asm.push_cmd(Command::Push(Value::Rax.into()));
                    }
                    _ => todo!(),
                }
                Ok(())
            }
            Expr::BinOp { op, left, right } => {
                self.visit_expr(left, tree)?;
                self.visit_expr(right, tree)?;
                self.asm.push_cmd(Command::Pop(Value::Rbx.into()));
                self.asm.push_cmd(Command::Pop(Value::Rax.into()));
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
                self.asm.push_cmd(Command::Push(Value::Rax.into()));
                Ok(())
            }
            _ => todo!(),
        }
    }
}

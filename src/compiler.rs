use crate::{
    error::Error,
    parser::{Decl, Expr, ExprRef, Stmt, StmtRef, Tree},
    tokens::TokenType,
};
use std::io::Write;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Clone)]
#[allow(dead_code)]
/// - Rsp -> stack top pointer
/// - Rbp -> stack base pointer
///
/// - 64 bits: Rax, Rbc, Rcx, Rdx, Rbp, Rsp
/// - 32 bits: Eax, Ebx, Edx
/// - 8 bits: Al, Bl
/// - 128 bits: Xmm0, Xmm1, Xmm2
pub enum Register<'a> {
    Integer(i32),
    Variable(&'a str),

    Rax,
    Rbx,
    Rcx,
    Rdx,

    Rbp,
    Rsp,

    Eax,
    Ebx,
    Edx,

    Al,
    Bl,

    Xmm0,
    Xmm1,
    Xmm2,
}

impl<'a> Display for Register<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Register::Integer(i) => write!(f, "{i}"),
            Register::Variable(v) => write!(f, "{}", v),
            Register::Rax => write!(f, "rax"),
            Register::Rbx => write!(f, "rbx"),
            Register::Rdx => write!(f, "rdx"),
            Register::Rbp => write!(f, "rbp"),
            Register::Rcx => write!(f, "rcx"),
            Register::Rsp => write!(f, "rsp"),
            Register::Eax => write!(f, "eax"),
            Register::Ebx => write!(f, "ebx"),
            Register::Edx => write!(f, "edx"),
            Register::Al => write!(f, "al"),
            Register::Bl => write!(f, "bl"),
            Register::Xmm0 => write!(f, "xmm0"),
            Register::Xmm1 => write!(f, "xmm1"),
            Register::Xmm2 => write!(f, "xmm2"),
        }
    }
}

impl PartialEq for Register<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Register::Integer(i1), Register::Integer(i2)) => i1 == i2,
            (Register::Variable(v1), Register::Variable(v2)) => v1 == v2,
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Memory<'a> {
    base: Register<'a>,
    offset: i32,
}

impl<'a> Memory<'a> {
    pub fn new(base: Register<'a>) -> Self {
        Self { base, offset: 0 }
    }

    pub fn with_offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }
}

impl Display for Memory<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.offset {
            0 => write!(f, "dword [{}]", self.base),
            _ => write!(f, "dword [{} - {}]", self.base, self.offset),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand<'a> {
    Register(Register<'a>),
    Memory(Memory<'a>),
}

impl Display for Operand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::Register(r) => write!(f, "{}", r),
            Operand::Memory(mem) => write!(f, "{}", mem),
        }
    }
}

impl<'a> Into<Operand<'a>> for Register<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Register(self)
    }
}

impl<'a> Into<Operand<'a>> for Memory<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Memory(self)
    }
}

impl<'a> Register<'a> {
    pub fn with_offset(self, offset: i32) -> Memory<'a> {
        Memory::new(self).with_offset(offset)
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
/// - Push(op) -> push op to the stack
/// - Pop(op) -> pop value from the stack into op
/// - Mov { dst, src } -> move value from src into dst
/// - Add { dst, src } -> add dst and src values and save result in dst
/// - Sub { dst, src } -> subtract src from dst and save result in dst
/// - Imul { dst, src } -> multiply src and dst values and save result in dst
/// - Div(reg) -> divide value in RAX by reg value
/// - Neg(reg) -> make value in reg negative
/// - Xor { dst, src } -> XOR operation on dst and src values and save result in dst
/// - Lea { dst, src } -> save address of src to dst
/// - Call { name } -> call a function by the name
/// - Ret -> return (exit)
/// - Cmp { op1, op1 } -> compare op1 and op2 values
/// - Je(l) -> jump to l if previous cmp returns equals
/// - Jne(l) -> jump to l if previous cmp returns not equals
pub enum Command<'a> {
    Push(Operand<'a>),
    Pop(Operand<'a>),
    Mov {
        dst: Operand<'a>,
        src: Operand<'a>,
    },
    Add {
        dst: Register<'a>,
        src: Register<'a>,
    },
    Sub {
        dst: Register<'a>,
        src: Register<'a>,
    },
    Imul {
        dst: Register<'a>,
        src: Register<'a>,
    },
    Div(Register<'a>),
    Neg(Register<'a>),
    Xor {
        dst: Register<'a>,
        src: Register<'a>,
    },
    And {
        dst: Register<'a>,
        src: Register<'a>,
    },
    Or {
        dst: Register<'a>,
        src: Register<'a>,
    },
    Lea {
        dst: Register<'a>,
        src: Memory<'a>,
    },
    Inc(Register<'a>),
    Dec(Register<'a>),
    Call {
        name: &'a str,
    },
    Ret,

    Cmp {
        op1: Operand<'a>,
        op2: Operand<'a>,
    },
    Je(String),
    Jne(String),
    Jg(String),
    Jge(String),
    Jl(String),
    Jle(String),
    Jmp(String),
    Sete(Register<'a>),
    Setne(Register<'a>),
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
                Command::Neg(dst) => {
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
                Command::And { dst, src } => writeln!(self.output, "and {}, {}", dst, src),
                Command::Or { dst, src } => writeln!(self.output, "or {}, {}", dst, src),
                Command::Pop(v) => {
                    writeln!(self.output, "pop {}", v)
                }
                Command::Inc(reg) => writeln!(self.output, "inc {}", reg),
                Command::Dec(reg) => writeln!(self.output, "dec {}", reg),
                Command::Lea { dst, src } => writeln!(self.output, "lea {}, {}", dst, src),
                Command::Cmp { op1, op2 } => writeln!(self.output, "cmp {}, {}", op1, op2),
                Command::Je(l) => writeln!(self.output, "je {l}"),
                Command::Jg(l) => writeln!(self.output, "jg {l}"),
                Command::Jge(l) => writeln!(self.output, "jge {l}"),
                Command::Jl(l) => writeln!(self.output, "jl {l}"),
                Command::Jle(l) => writeln!(self.output, "jle {l}"),
                Command::Jne(l) => writeln!(self.output, "jne {l}"),
                Command::Jmp(l) => writeln!(self.output, "jmp {l}"),
                Command::Sete(r) => writeln!(self.output, "sete {}", r),
                Command::Setne(r) => writeln!(self.output, "setne {}", r),
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

pub struct Compiler<'a, W: Write> {
    asm: Assambler<'a, W>,
    locals: HashMap<String, i32>,
    current_l_num: u64,
}

impl<'a, W: Write> Compiler<'a, W> {
    pub fn new(output: W) -> Result<Self, Error> {
        Ok(Compiler {
            asm: Assambler::new(output, true),
            locals: HashMap::new(),
            current_l_num: 0,
        })
    }

    fn next_l(&mut self, slug: &str) -> String {
        self.current_l_num += 1;
        format!(".L{}_{slug}", self.current_l_num - 1)
    }

    fn offset(&self, ind: i32) -> i32 {
        (self.locals.len() as i32 + 1 - ind) * 4
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
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        self.asm.push_cmd(Command::Mov {
            dst: Register::Rbp.with_offset(offset).into(),
            src: Register::Eax.into(),
        });
        Ok(())
    }

    fn visit_stmt<'s>(&'s mut self, stmt: &StmtRef, tree: &Tree) -> Result<(), Error> {
        match tree.stmt_pool.get(*stmt) {
            Stmt::Program { name: _, block } => {
                self.asm.directive("section .data")?;
                self.asm.directive("fmt db \"> %d\", 10, 0")?;
                self.asm.newline()?;
                self.asm.directive("section .text")?;
                self.asm.directive("global main")?;
                self.asm.directive("extern printf")?;
                self.asm.newline()?;
                self.asm.comment("main function entry point")?;
                self.asm.label("main")?;
                self.visit_stmt(block, tree)?;
                self.asm.push_cmd(Command::Xor {
                    dst: Register::Eax,
                    src: Register::Eax,
                });
                self.asm.push_cmd(Command::Ret);
                Ok(())
            }
            Stmt::Block {
                declarations,
                statements,
            } => {
                self.asm.comment("block")?;
                self.asm.push_cmd(Command::Push(Register::Rbp.into()));
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rbp.into(),
                    src: Register::Rsp.into(),
                });
                let defaults = declarations
                    .iter()
                    .map(|decl| self.visit_decl(decl, tree))
                    .collect::<Result<Vec<_>, Error>>()?;
                let local_size = (self.locals.len() as i32) * 4;
                let aligned_local_size = ((local_size + 15) / 16) * 16;
                self.asm.push_cmd(Command::Sub {
                    dst: Register::Rsp,
                    src: Register::Integer(32 + aligned_local_size),
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
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rbp.with_offset(offset).into(),
                    src: Register::Eax.into(),
                });
                Ok(())
            }
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => {
                self.visit_expr(&cond.cond, tree)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Cmp {
                    op1: Register::Rax.into(),
                    op2: Register::Integer(0).into(),
                });
                let mut else_l = self.next_l("else");
                let end_l = self.next_l("endif");
                self.asm.push_cmd(Command::Je(else_l.clone()));
                self.visit_stmt(&cond.expr, tree)?;
                let mut elifs = elifs.iter();
                while let Some(elif) = elifs.next() {
                    self.asm.push_cmd(Command::Jmp(end_l.clone()));
                    self.asm.label(&else_l)?;
                    else_l = self.next_l("else");
                    self.visit_expr(&elif.cond, tree)?;
                    self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                    self.asm.push_cmd(Command::Cmp {
                        op1: Register::Rax.into(),
                        op2: Register::Integer(0).into(),
                    });
                    self.asm.push_cmd(Command::Je(else_l.clone()));
                    self.visit_stmt(&elif.expr, tree)?;
                }
                if let Some(else_stmt) = else_statement {
                    self.asm.push_cmd(Command::Jmp(end_l.clone()));
                    self.asm.label(&else_l)?;
                    self.visit_stmt(else_stmt, tree)?;
                    self.asm.label(&end_l)?;
                } else {
                    self.asm.label(&else_l)?;
                }
                Ok(())
            }
            Stmt::While { cond, body } => {
                let loop_l = self.next_l("while");
                let loop_end_l = self.next_l("endwhile");
                self.asm.label(&loop_l)?;
                self.visit_expr(cond, tree)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Cmp {
                    op1: Register::Rax.into(),
                    op2: Register::Integer(0).into(),
                });
                self.asm.push_cmd(Command::Je(loop_end_l.clone()));
                self.visit_stmt(body, tree)?;
                self.asm.push_cmd(Command::Jmp(loop_l));
                self.asm.label(&loop_end_l)?;
                Ok(())
            }
            Stmt::For {
                var,
                init,
                end,
                body,
            } => {
                let var_offset = self
                    .var_offset(var.lexem(tree.source_code))
                    .expect("var should exist");
                self.visit_expr(init, tree)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Dec(Register::Eax));
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rbp.with_offset(var_offset).into(),
                    src: Register::Eax.into(),
                });
                self.visit_expr(end, tree)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Push(Register::Rax.into()));
                let l1 = self.next_l("for_body");
                self.asm.label(&l1)?;
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Eax.into(),
                    src: Register::Rbp.with_offset(var_offset).into(),
                });
                self.asm.push_cmd(Command::Inc(Register::Eax.into()));
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rbp.with_offset(var_offset).into(),
                    src: Register::Eax.into(),
                });
                self.asm.push_cmd(Command::Cmp {
                    op1: Register::Eax.into(),
                    op2: Memory::new(Register::Rsp).into(),
                });
                let l2 = self.next_l("endfor");
                self.asm.push_cmd(Command::Jg(l2.clone()));
                self.visit_stmt(body, tree)?;
                self.asm.push_cmd(Command::Jmp(l1.clone()));
                self.asm.label(&l2)?;
                self.asm.push_cmd(Command::Pop(Register::Rdx.into()));
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
                    self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Rcx.into(),
                        src: Register::Variable("fmt").into(),
                    });
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Rdx.into(),
                        src: Register::Rax.into(),
                    });
                    self.asm.push_cmd(Command::Sub {
                        dst: Register::Rsp,
                        src: Register::Integer(32),
                    });
                    self.asm.push_cmd(Command::Call { name: "printf" });
                    self.asm.push_cmd(Command::Add {
                        dst: Register::Rsp,
                        src: Register::Integer(32),
                    });
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
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Eax.into(),
                    src: Register::Rbp
                        .with_offset(
                            self.var_offset(var_name)
                                .expect("expected value to exist")
                                .into(),
                        )
                        .into(),
                });
                self.asm.push_cmd(Command::Push(Register::Rax.into()));
                Ok(())
            }
            Expr::LiteralInteger(i) => {
                self.asm
                    .push_cmd(Command::Push(Register::Integer(*i).into()));
                Ok(())
            }
            Expr::LiteralBool(b) => {
                let val = match b {
                    true => 1,
                    false => 0,
                };
                self.asm
                    .push_cmd(Command::Push(Register::Integer(val).into()));
                Ok(())
            }
            Expr::UnaryOp { op, expr } => {
                self.visit_expr(expr, tree)?;
                match op {
                    TokenType::Plus => {}
                    TokenType::Minus => {
                        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                        self.asm.push_cmd(Command::Neg(Register::Rax));
                        self.asm.push_cmd(Command::Push(Register::Rax.into()));
                    }
                    _ => todo!(),
                }
                Ok(())
            }
            Expr::BinOp { op, left, right } => {
                self.visit_expr(left, tree)?;
                self.visit_expr(right, tree)?;
                self.asm.push_cmd(Command::Pop(Register::Rbx.into()));
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                match op {
                    TokenType::Plus => {
                        self.asm.push_cmd(Command::Add {
                            dst: Register::Rax,
                            src: Register::Rbx,
                        });
                    }
                    TokenType::Minus => {
                        self.asm.push_cmd(Command::Sub {
                            dst: Register::Rax,
                            src: Register::Rbx,
                        });
                    }
                    TokenType::Mul => {
                        self.asm.push_cmd(Command::Imul {
                            dst: Register::Rax,
                            src: Register::Rbx,
                        });
                    }
                    TokenType::IntegerDiv => {
                        self.asm.push_cmd(Command::Xor {
                            dst: Register::Rdx,
                            src: Register::Rdx,
                        });
                        self.asm.push_cmd(Command::Div(Register::Rbx));
                    }
                    TokenType::And => {
                        self.asm.push_cmd(Command::And {
                            dst: Register::Al,
                            src: Register::Bl,
                        });
                    }
                    TokenType::Or => {
                        self.asm.push_cmd(Command::Or {
                            dst: Register::Al,
                            src: Register::Bl,
                        });
                    }
                    TokenType::Equal | TokenType::NotEqual => self.visit_comparison(&op)?,
                    _ => unreachable!(),
                }
                self.asm.push_cmd(Command::Push(Register::Rax.into()));
                Ok(())
            }
            _ => todo!(),
        }
    }

    fn visit_comparison(&mut self, cmp_token: &TokenType) -> Result<(), Error> {
        self.asm.push_cmd(Command::Cmp {
            op1: Register::Rax.into(),
            op2: Register::Rbx.into(),
        });
        match cmp_token {
            TokenType::Equal => {
                self.asm.push_cmd(Command::Sete(Register::Al));
            }
            TokenType::NotEqual => {
                self.asm.push_cmd(Command::Setne(Register::Al));
            }
            _ => unreachable!(),
        };
        Ok(())
    }
}

use crate::{
    error::Error,
    parser::{Decl, Expr, ExprRef, Param, Stmt, StmtRef, Tree},
    semantic_analyzer::SemanticMetadata,
    symbols::{ConstValue, VarSymbol, VarType},
    tokens::TokenType,
};
use std::fmt::Display;
use std::{collections::HashSet, io::Write};

#[derive(Debug, Clone)]
#[allow(dead_code)]
/// - Rsp -> stack top pointer
/// - Rbp -> stack base pointer
///
/// - 64 bits: Rax, Rbc, Rcx, Rdx, Rbp, Rsp
/// - 32 bits: Eax, Ebx, Edx
/// - 8 bits: Al, Bl
/// - 128 bits: Xmm0, Xmm1, Xmm2
enum Register<'a> {
    Integer(i32),
    Variable(&'a str),

    Rax,
    Rbx,
    Rcx,
    Rdx,
    R8,
    R9,

    Rbp,
    Rsp,

    Eax,
    Ebx,
    Edx,
    Ecx,
    R8d,
    R9d,

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
            Register::Ecx => write!(f, "ecx"),
            Register::R8d => write!(f, "r8d"),
            Register::R9d => write!(f, "r9d"),
            Register::R8 => write!(f, "r8"),
            Register::R9 => write!(f, "r9"),
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

impl<'a> Register<'a> {
    pub fn from_param_index64(i: usize) -> Self {
        match i {
            0 => Register::Rcx,
            1 => Register::Rdx,
            2 => Register::R8,
            4 => Register::R9,
            _ => unimplemented!("more input variables are not implemented yet"),
        }
    }
    pub fn from_param_index32(i: usize) -> Self {
        match i {
            0 => Register::Ecx,
            1 => Register::Edx,
            2 => Register::R8d,
            4 => Register::R9d,
            _ => unimplemented!("more input variables are not implemented yet"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct GlobalMemory<'a>(&'a str);

impl<'a> Display for GlobalMemory<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dword [rel {}]", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct StackMemory<'a> {
    base: Register<'a>,
    offset: usize,
}

impl<'a> StackMemory<'a> {
    pub fn new(base: Register<'a>) -> Self {
        Self { base, offset: 0 }
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }
}

impl Display for StackMemory<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.offset {
            0 => write!(f, "dword [{}]", self.base),
            _ => write!(f, "dword [{} - {}]", self.base, self.offset),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Operand<'a> {
    Register(Register<'a>),
    StackMemory(StackMemory<'a>),
    GlobalMemory(GlobalMemory<'a>),
}

impl Display for Operand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::Register(r) => write!(f, "{}", r),
            Operand::StackMemory(mem) => write!(f, "{}", mem),
            Operand::GlobalMemory(var) => write!(f, "{}", var),
        }
    }
}

impl<'a> Into<Operand<'a>> for Register<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Register(self)
    }
}

impl<'a> Into<Operand<'a>> for StackMemory<'a> {
    fn into(self) -> Operand<'a> {
        Operand::StackMemory(self)
    }
}

impl<'a> Into<Operand<'a>> for GlobalMemory<'a> {
    fn into(self) -> Operand<'a> {
        Operand::GlobalMemory(self)
    }
}

impl<'a> Register<'a> {
    pub fn with_offset(self, offset: usize) -> StackMemory<'a> {
        StackMemory::new(self).with_offset(offset)
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
enum Command<'a> {
    Push(Operand<'a>),
    Pop(Operand<'a>),
    Mov {
        dst: Operand<'a>,
        src: Operand<'a>,
    },
    Movzx {
        dst: Register<'a>,
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
    Not(Register<'a>),
    Lea {
        dst: Register<'a>,
        src: StackMemory<'a>,
    },
    Inc(Register<'a>),
    Dec(Register<'a>),
    Call {
        name: &'a str,
    },
    Ret,
    Leave,

    Cmp {
        op1: Operand<'a>,
        op2: Operand<'a>,
    },
    Test {
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
    Setg(Register<'a>),
    Setge(Register<'a>),
    Setl(Register<'a>),
    Setle(Register<'a>),
}

#[derive(Debug, Clone)]
struct Assambler<'a, W: Write> {
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
                Command::Movzx { dst, src } => writeln!(self.output, "movzx {}, {}", dst, src),
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
                Command::Ret => writeln!(self.output, "ret"),
                Command::Leave => writeln!(self.output, "leave"),
                Command::Call { name } => {
                    writeln!(self.output, "call {}", name)
                }
                Command::Xor { dst, src } => {
                    writeln!(self.output, "xor {}, {}", dst, src)
                }
                Command::Not(r) => {
                    writeln!(self.output, "not {}", r)
                }
                Command::Test { op1, op2 } => writeln!(self.output, "test {}, {}", op1, op2),
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
                Command::Setg(r) => writeln!(self.output, "setg {}", r),
                Command::Setge(r) => writeln!(self.output, "setge {}", r),
                Command::Setl(r) => writeln!(self.output, "setl {}", r),
                Command::Setle(r) => writeln!(self.output, "setle {}", r),
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

#[derive(Debug, Clone)]
struct ActivationRecord {
    members: Vec<String>,
    callables: HashSet<String>,
}

#[derive(Debug, Clone)]
struct CallStack(Vec<ActivationRecord>);

impl CallStack {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    pub fn push_global_ar(&mut self) {
        let ar = ActivationRecord {
            members: Vec::new(),
            callables: HashSet::new(),
        };
        self.0.push(ar);
    }
    #[inline]
    pub fn push_ar(&mut self) {
        let mut ar = ActivationRecord {
            members: Vec::new(),
            callables: HashSet::new(),
        };
        self.0.last().map(|ar| &ar.callables).map(|callables| {
            ar.callables.extend(callables.clone());
        });
        self.0.push(ar);
    }
    #[inline]
    pub fn pop_ar(&mut self) -> Option<ActivationRecord> {
        self.0.pop()
    }

    #[inline]
    pub fn push_var(&mut self, name: &str) {
        let last_ar = self.0.last_mut().unwrap();
        last_ar.members.push(name.into());
    }

    #[inline]
    pub fn lookup_var<'a>(&self, name: &'a str) -> StackMemory<'a> {
        self.0
            .last()
            .and_then(|ar| {
                ar.members
                    .iter()
                    .position(|v| v == name)
                    .map(|i| (ar.members.len(), i))
            })
            .map(|(s, i)| Register::Rbp.with_offset((s + 1 - i) * 4))
            .unwrap_or_else(|| panic!("unkown value {name}"))
    }

    #[inline]
    pub fn aligned_size(&self) -> usize {
        ((self.0.last().unwrap().members.len() * 4 + 15) / 16) * 16
    }

    #[inline]
    pub fn contains_callable(&self, name: &str) -> bool {
        self.0
            .last()
            .map(|ar| ar.callables.contains(name))
            .unwrap_or(false)
    }

    #[inline]
    pub fn push_callable(&mut self, name: &str) {
        self.0.last_mut().unwrap().callables.insert(name.into());
    }
}

#[derive(Debug, Clone)]
pub struct Compiler<'a, W: Write> {
    asm: Assambler<'a, W>,
    call_stack: CallStack,
    current_l_num: u64,
    loop_exit_labels: Vec<String>,
    loop_start_labels: Vec<String>,
}

impl<'a, W: Write> Compiler<'a, W> {
    pub fn new(output: W) -> Result<Self, Error> {
        Ok(Compiler {
            asm: Assambler::new(output, true),
            call_stack: CallStack::new(),
            current_l_num: 0,
            loop_exit_labels: Vec::new(),
            loop_start_labels: Vec::new(),
        })
    }

    pub fn compile(
        mut self,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<W, Error> {
        self.visit_stmt(&tree.program, tree, semantic_metadata)?;
        let output = self.asm.output()?;
        Ok(output)
    }

    #[inline]
    fn next_l(&mut self, slug: &str) -> String {
        self.current_l_num += 1;
        format!(".L{}_{slug}", self.current_l_num - 1)
    }

    #[inline]
    fn enter_loop(&mut self, start_label: &str, end_label: &str) {
        self.loop_start_labels.push(start_label.to_string());
        self.loop_exit_labels.push(end_label.to_string());
    }

    #[inline]
    fn exit_loop(&mut self) {
        self.loop_exit_labels.pop();
        self.loop_start_labels.pop();
    }

    fn visit_var_decl_local(
        &mut self,
        var: &Decl,
        tree: &'a Tree,
    ) -> Result<Option<(&'a str, ExprRef)>, Error> {
        match var {
            Decl::VarDecl {
                default_value,
                var,
                type_node: _,
            } => {
                let var_name = match tree.expr_pool.get(*var) {
                    Expr::Var { name } => name.lexem(tree.source_code),
                    _ => unreachable!(),
                };
                self.call_stack.push_var(var_name);
                Ok(default_value.map(|v| (var_name, v)))
            }
            _ => unreachable!(),
        }
    }

    fn visit_var_decl_global(&mut self, var: &Decl, tree: &'a Tree) -> Result<(), Error> {
        match var {
            Decl::VarDecl {
                default_value: _,
                var,
                type_node: _,
            } => {
                let var_name = match tree.expr_pool.get(*var) {
                    Expr::Var { name } => name.lexem(tree.source_code),
                    _ => unreachable!(),
                };
                self.asm.directive(&format!("{var_name} resd 1"))?;
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn visit_callable_decl(
        &mut self,
        callable: &Decl,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<(), Error> {
        match callable {
            Decl::Callable {
                params,
                name,
                return_type,
                block,
            } => {
                let func_name = name.lexem(tree.source_code);
                match tree.stmt_pool.get(*block) {
                    Stmt::Block {
                        declarations,
                        statements,
                    } => {
                        self.call_stack.push_callable(func_name);
                        self.enter_scope(
                            func_name,
                            params,
                            declarations,
                            statements,
                            return_type.is_some(),
                            tree,
                            semantic_metadata,
                        )
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    fn enter_func(&mut self, aligned_size: usize) -> Result<(), Error> {
        self.asm.push_cmd(Command::Push(Register::Rbp.into()));
        self.asm.push_cmd(Command::Mov {
            dst: Register::Rbp.into(),
            src: Register::Rsp.into(),
        });
        self.asm.push_cmd(Command::Sub {
            dst: Register::Rsp,
            src: Register::Integer(aligned_size as i32),
        });
        Ok(())
    }

    fn enter_scope(
        &mut self,
        scope_name: &str,
        params: &[Param],
        declarations: &[Decl],
        statements: &StmtRef,
        returns: bool,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<(), Error> {
        self.call_stack.push_ar();
        let global = scope_name == "main";
        if global {
            declarations
                .iter()
                .filter(|d| matches!(d, Decl::Callable { .. }))
                .try_for_each(|c| self.visit_callable_decl(c, tree, semantic_metadata))?;
        } else {
            self.call_stack.push_callable(scope_name);
        }
        self.asm
            .comment(&format!("{scope_name} function entry point"))?;
        self.asm.label(scope_name)?;
        self.asm.comment("block")?;
        let defaults = {
            let defaults = declarations
                .iter()
                .filter(|d| matches!(d, Decl::VarDecl { .. }))
                .map(|decl| self.visit_var_decl_local(decl, tree))
                .filter_map(Result::transpose)
                .collect::<Result<Vec<_>, Error>>()?;
            defaults.iter().for_each(|(name, _)| {
                self.call_stack.push_var(name);
            });
            defaults
        };
        let param_names: Vec<&str> = params
            .iter()
            .map(|param| match tree.expr_pool.get(param.var) {
                Expr::Var { name } => name.lexem(tree.source_code),
                _ => unreachable!(),
            })
            .collect();
        param_names.iter().for_each(|param_name| {
            self.call_stack.push_var(*param_name);
        });
        if returns {
            self.call_stack.push_var("result");
        }
        let local_size = self.call_stack.aligned_size();
        self.enter_func(local_size)?;
        param_names
            .iter()
            .enumerate()
            .try_for_each(|(i, param_name)| {
                let reg = Register::from_param_index32(i);
                let mem = self.call_stack.lookup_var(*param_name);
                self.asm.push_cmd(Command::Mov {
                    dst: mem.into(),
                    src: reg.into(),
                });
                Ok::<(), Error>(())
            })?;
        defaults.into_iter().try_for_each(|(var_name, v)| {
            self.visit_expr(&v, tree, semantic_metadata)?;
            self.asm.push_cmd(Command::Pop(Register::Rax.into()));
            self.asm.push_cmd(Command::Mov {
                dst: self.call_stack.lookup_var(var_name).into(),
                src: Register::Eax.into(),
            });
            Ok::<(), Error>(())
        })?;
        self.visit_stmt(statements, tree, semantic_metadata)?;
        if returns {
            self.asm.push_cmd(Command::Mov {
                dst: Register::Eax.into(),
                src: self.call_stack.lookup_var("result").into(),
            });
        } else {
            self.asm.push_cmd(Command::Xor {
                dst: Register::Eax,
                src: Register::Eax,
            });
        }
        self.asm.push_cmd(Command::Leave);
        self.asm.push_cmd(Command::Ret);
        self.asm.comment("end block")?;
        self.call_stack.pop_ar();
        Ok(())
    }

    fn enter_global_scope(
        &mut self,
        declarations: &[Decl],
        statements: &StmtRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<(), Error> {
        self.asm.directive("section .bss")?;
        declarations
            .iter()
            .filter(|d| matches!(d, Decl::VarDecl { .. }))
            .try_for_each(|d| self.visit_var_decl_global(d, tree))?;
        self.asm.newline()?;
        self.call_stack.push_global_ar();
        self.asm.directive("section .text")?;
        self.asm.directive("global main")?;
        self.asm.directive("extern printf")?;
        self.asm.newline()?;
        declarations
            .iter()
            .filter(|d| matches!(d, Decl::Callable { .. }))
            .try_for_each(|c| self.visit_callable_decl(c, tree, semantic_metadata))?;
        self.asm.comment("main entry point")?;
        self.asm.label("main")?;
        self.asm.comment("block")?;
        let local_size = self.call_stack.aligned_size();
        self.enter_func(local_size)?;
        self.visit_stmt(statements, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Xor {
            dst: Register::Eax,
            src: Register::Eax,
        });
        self.asm.push_cmd(Command::Leave);
        self.asm.push_cmd(Command::Ret);
        self.asm.comment("end block")?;
        self.call_stack.pop_ar();
        Ok(())
    }

    fn visit_stmt(
        &mut self,
        stmt: &StmtRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<(), Error> {
        match tree.stmt_pool.get(*stmt) {
            Stmt::Program { name: _, block } => {
                self.asm.directive("section .data")?;
                self.asm.directive("fmt db \"%d\", 0")?;
                self.asm.directive("newline db 10, 0")?;
                self.asm.newline()?;
                match tree.stmt_pool.get(*block) {
                    Stmt::Block {
                        declarations,
                        statements,
                    } => self.enter_global_scope(declarations, statements, tree, semantic_metadata),
                    _ => unreachable!(),
                }
            }
            Stmt::Compound(stmts) => stmts.iter().try_for_each(|v| {
                self.visit_stmt(v, tree, semantic_metadata)?;
                Ok(())
            }),
            Stmt::NoOp => Ok(()),
            Stmt::Call { call } => self.visit_call(call, tree, semantic_metadata),
            Stmt::Assign { left, right } => {
                let var_name = match tree.expr_pool.get(*left) {
                    Expr::Var { name } => name.lexem(tree.source_code),
                    _ => unreachable!(),
                };
                self.visit_expr(right, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                let source_op = match semantic_metadata.var_types.get(left).unwrap() {
                    VarType::Local => self.call_stack.lookup_var(var_name).into(),
                    VarType::Global => GlobalMemory(var_name).into(),
                };
                self.asm.push_cmd(Command::Mov {
                    dst: source_op,
                    src: Register::Eax.into(),
                });
                Ok(())
            }
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => {
                self.visit_expr(&cond.cond, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Cmp {
                    op1: Register::Al.into(),
                    op2: Register::Integer(0).into(),
                });
                let mut else_l = self.next_l("else");
                let end_l = self.next_l("endif");
                self.asm.push_cmd(Command::Je(else_l.clone()));
                self.visit_stmt(&cond.expr, tree, semantic_metadata)?;
                let mut elifs = elifs.iter();
                while let Some(elif) = elifs.next() {
                    self.asm.push_cmd(Command::Jmp(else_l.clone()));
                    self.asm.label(&else_l)?;
                    else_l = self.next_l("else");
                    self.visit_expr(&elif.cond, tree, semantic_metadata)?;
                    self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                    self.asm.push_cmd(Command::Cmp {
                        op1: Register::Al.into(),
                        op2: Register::Integer(0).into(),
                    });
                    self.asm.push_cmd(Command::Je(else_l.clone()));
                    self.visit_stmt(&elif.expr, tree, semantic_metadata)?;
                }
                if let Some(else_stmt) = else_statement {
                    self.asm.push_cmd(Command::Jmp(end_l.clone()));
                    self.asm.label(&else_l)?;
                    self.visit_stmt(else_stmt, tree, semantic_metadata)?;
                    self.asm.label(&end_l)?;
                } else {
                    self.asm.label(&else_l)?;
                }
                Ok(())
            }
            Stmt::While { cond, body } => {
                let loop_l = self.next_l("while");
                let loop_end_l = self.next_l("endwhile");
                self.enter_loop(&loop_l, &loop_end_l);
                self.asm.label(&loop_l)?;
                self.visit_expr(cond, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Cmp {
                    op1: Register::Rax.into(),
                    op2: Register::Integer(0).into(),
                });
                self.asm.push_cmd(Command::Je(loop_end_l.clone()));
                self.visit_stmt(body, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Jmp(loop_l));
                self.asm.label(&loop_end_l)?;
                self.exit_loop();
                Ok(())
            }
            Stmt::For {
                var,
                init,
                end,
                body,
            } => {
                let var_name = match tree.expr_pool.get(*var) {
                    Expr::Var { name } => name.lexem(tree.source_code),
                    _ => unreachable!(),
                };
                let var_mem: Operand = match semantic_metadata.var_types.get(var).unwrap() {
                    VarType::Local => self.call_stack.lookup_var(var_name).into(),
                    VarType::Global => GlobalMemory(var_name).into(),
                };
                self.visit_expr(init, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Dec(Register::Eax));
                self.asm.push_cmd(Command::Mov {
                    dst: var_mem.clone(),
                    src: Register::Eax.into(),
                });
                self.visit_expr(end, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                self.asm.push_cmd(Command::Push(Register::Rax.into()));
                let l1 = self.next_l("for_body");
                let l2 = self.next_l("endfor");
                self.enter_loop(&l1, &l2);
                self.asm.label(&l1)?;
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Eax.into(),
                    src: var_mem.clone().into(),
                });
                self.asm.push_cmd(Command::Inc(Register::Eax.into()));
                self.asm.push_cmd(Command::Mov {
                    dst: var_mem.into(),
                    src: Register::Eax.into(),
                });
                self.asm.push_cmd(Command::Cmp {
                    op1: Register::Eax.into(),
                    op2: StackMemory::new(Register::Rsp).into(),
                });
                self.asm.push_cmd(Command::Jg(l2.clone()));
                self.visit_stmt(body, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Jmp(l1.clone()));
                self.asm.label(&l2)?;
                self.asm.push_cmd(Command::Pop(Register::Rdx.into()));
                self.exit_loop();
                Ok(())
            }
            Stmt::Break => {
                let end_l = self
                    .loop_exit_labels
                    .last()
                    .expect("break should not be outside of the loop");
                self.asm.push_cmd(Command::Jmp(end_l.clone()));
                Ok(())
            }
            Stmt::Continue => {
                let start_l = self
                    .loop_start_labels
                    .last()
                    .expect("continue should be within a loop");
                self.asm.push_cmd(Command::Jmp(start_l.clone()));
                Ok(())
            }
            Stmt::Exit(val) => {
                if let Some(expr) = val {
                    self.visit_expr(expr, tree, semantic_metadata)?;
                    self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                } else {
                    self.asm.push_cmd(Command::Xor {
                        dst: Register::Eax,
                        src: Register::Eax,
                    });
                }
                self.asm.push_cmd(Command::Leave);
                self.asm.push_cmd(Command::Ret);
                Ok(())
            }
            _ => todo!(),
        }
    }

    fn visit_call(
        &mut self,
        call: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<(), Error> {
        match tree.expr_pool.get(*call) {
            Expr::Call { name, args } => {
                let func_name = name.lexem(tree.source_code);
                if self.call_stack.contains_callable(func_name) {
                    args.iter().try_for_each(|arg| {
                        self.visit_expr(arg, tree, semantic_metadata)?;
                        Ok::<(), Error>(())
                    })?;
                    (0..args.len()).into_iter().try_for_each(|i| {
                        let reg = Register::from_param_index64(i);
                        self.asm.push_cmd(Command::Pop(reg.into()));
                        Ok::<(), Error>(())
                    })?;
                    self.asm.push_cmd(Command::Call { name: func_name });
                    return Ok(());
                }
                if func_name != "writeln" {
                    unimplemented!("Only writeln is supported as a builtin funcion for now")
                }
                self.asm.comment("call writeln")?;
                for arg in args {
                    self.visit_expr(arg, tree, semantic_metadata)?;
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
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rcx.into(),
                    src: Register::Variable("newline").into(),
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
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn visit_expr(
        &mut self,
        expr: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<(), Error> {
        match tree.expr_pool.get(*expr) {
            Expr::Call { .. } => {
                self.visit_call(expr, tree, semantic_metadata)?;
            }
            Expr::Var { .. } => match semantic_metadata
                .vars
                .get(*semantic_metadata.var_symbols.get(expr).unwrap())
            {
                VarSymbol::Var {
                    name,
                    type_symbol: _,
                } => {
                    let source_op = match semantic_metadata.var_types.get(expr).unwrap() {
                        VarType::Local => self.call_stack.lookup_var(name).into(),
                        VarType::Global => GlobalMemory(name).into(),
                    };
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Eax.into(),
                        src: source_op,
                    });
                }
                VarSymbol::Const {
                    value,
                    type_symbol: _,
                } => match value {
                    ConstValue::Integer(i) => {
                        self.asm.push_cmd(Command::Mov {
                            dst: Register::Eax.into(),
                            src: Register::Integer(*i).into(),
                        });
                    }
                    _ => todo!(),
                },
            },
            Expr::LiteralInteger(i) => self.asm.push_cmd(Command::Mov {
                dst: Register::Rax.into(),
                src: Register::Integer(*i).into(),
            }),
            Expr::LiteralBool(b) => {
                let val = match b {
                    true => 1,
                    false => 0,
                };
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rax.into(),
                    src: Register::Integer(val).into(),
                })
            }
            Expr::UnaryOp { op, expr } => {
                self.visit_expr(expr, tree, semantic_metadata)?;
                self.asm.push_cmd(Command::Pop(Register::Rax.into()));
                match op {
                    TokenType::Plus => {}
                    TokenType::Minus => {
                        self.asm.push_cmd(Command::Neg(Register::Rax));
                    }
                    TokenType::Not => {
                        self.asm.push_cmd(Command::Xor {
                            dst: Register::Al,
                            src: Register::Integer(1),
                        });
                        self.asm.push_cmd(Command::Movzx {
                            dst: Register::Rax,
                            src: Register::Al.into(),
                        });
                    }
                    _ => todo!(),
                }
            }
            Expr::BinOp { op, left, right } => {
                self.visit_expr(left, tree, semantic_metadata)?;
                self.visit_expr(right, tree, semantic_metadata)?;
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
                    TokenType::Equal
                    | TokenType::NotEqual
                    | TokenType::GreaterEqual
                    | TokenType::GreaterThen
                    | TokenType::LessThen
                    | TokenType::LessEqual => self.visit_comparison(&op)?,
                    _ => unreachable!(),
                }
            }
            _ => todo!(),
        };
        self.asm.push_cmd(Command::Push(Register::Rax.into()));
        Ok(())
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
            TokenType::GreaterThen => {
                self.asm.push_cmd(Command::Setg(Register::Al));
            }
            TokenType::GreaterEqual => {
                self.asm.push_cmd(Command::Setge(Register::Al));
            }
            TokenType::LessThen => {
                self.asm.push_cmd(Command::Setl(Register::Al));
            }
            TokenType::LessEqual => {
                self.asm.push_cmd(Command::Setle(Register::Al));
            }
            _ => unreachable!(),
        };
        Ok(())
    }
}

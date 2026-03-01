use tracing::debug;

use crate::{
    error::{Error, Result},
    parser::{Condition, Decl, Expr, ExprRef, Param, Stmt, StmtRef, Tree},
    semantic_analyzer::SemanticMetadata,
    symbols::{ConstValue, TypeSymbol, TypeSymbolRef, VarLocality, VarPassMode, VarSymbol},
    tokens::TokenType,
    utils::Size,
};
use std::io::Write;
use std::{collections::HashMap, fmt::Display, sync::LazyLock};

const STD_DIV0_ERROR: &'static str = "std.error.div0_error";
const STD_ARR_INDEX_OUT_OF_BOUNDS_ERROR: &'static str = "std.error.index_out_of_bounds_error";

static BUILTIN_CALLABLES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("writeln", "std.io.writeln");
    map.insert("write", "std.io.write");
    map
});

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
    Dl,
    Cl,

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
            Register::Dl => write!(f, "dl"),
            Register::Cl => write!(f, "cl"),
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
            3 => Register::R9,
            _ => unimplemented!("more input variables are not implemented yet"),
        }
    }
    pub fn to_size(&self, size: &Size) -> Self {
        match (self, size) {
            (Self::Integer(i), _) => Self::Integer(*i),
            (Self::Variable(v), _) => Self::Variable(*v),
            (Register::Rax, Size::S64bit) => Self::Rax,
            (Register::Rax, Size::S8bit) => Self::Al,
            (Register::Rax, Size::S32bit) => Self::Eax,
            (Register::Rbx, Size::S64bit) => Self::Rbx,
            (Register::Rbx, Size::S8bit) => Self::Bl,
            (Register::Rbx, Size::S32bit) => Self::Ebx,
            (Register::Rcx, Size::S32bit) => Self::Ecx,
            (Register::Rdx, Size::S32bit) => Self::Edx,
            (Register::Rcx, Size::S64bit) => Self::Rcx,
            (Register::Rdx, Size::S64bit) => Self::Rdx,
            (Register::Rdx, Size::S8bit) => Self::Dl,
            (Register::R8, Size::S32bit) => Self::R8d,
            (Register::Rcx, Size::S8bit) => Self::Cl,
            (Register::R9, Size::S32bit) => Self::R9d,
            _ => unimplemented!("{} - {:?}", self, size),
        }
    }
}

impl Size {
    fn word(&self) -> Option<&str> {
        match self {
            Self::S8bit => Some("byte"),
            Self::S64bit => Some("qword"),
            Self::S32bit => Some("dword"),
            Self::S128bit => Some("oword"),
            Self::S16bit => Some("word"),
            _ => None,
        }
    }
    fn d(&self) -> Option<&str> {
        match self {
            Self::S8bit => Some("db"),
            Self::S64bit => Some("dq"),
            Self::S32bit => Some("dd"),
            Self::S128bit => Some("do"),
            Self::S16bit => Some("dw"),
            _ => None,
        }
    }
    fn res(&self) -> (&str, usize) {
        match self {
            Self::S8bit => ("resb", 1),
            Self::S16bit => ("resw", 1),
            Self::S32bit => ("resd", 1),
            Self::S64bit => ("resq", 1),
            Self::S128bit => ("reso", 1),
            Self::SArray {
                element_size,
                length,
            } => (element_size.res().0, *length),
        }
    }
    fn sign_extention<'a>(&self) -> Option<Command<'a>> {
        match self {
            Self::S16bit => Some(Command::Cwd),
            Self::S32bit => Some(Command::Cdq),
            Self::S64bit => Some(Command::Cqo),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct GlobalMemory<'a> {
    name: &'a str,
    size: Size,
}

impl<'a> GlobalMemory<'a> {
    pub fn new(name: &'a str, size: Size) -> Self {
        Self { name, size }
    }
}

impl<'a> Display for GlobalMemory<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [rel {}]",
            self.size
                .word()
                .expect("cannot access global memory for this size"),
            self.name
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
struct IndexMemory<'a> {
    register: Register<'a>,
    index: Register<'a>,
    size: Size,
}

impl<'a> Display for IndexMemory<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{} + {}*{}]",
            self.size.word().unwrap(),
            self.register,
            self.index,
            self.size.to_bytes()
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
struct StackMemory<'a> {
    base: Register<'a>,
    offset: usize,
    size: Size,
}

impl<'a> StackMemory<'a> {
    pub fn new(base: Register<'a>, size: Size) -> Self {
        Self {
            base,
            offset: 0,
            size: size,
        }
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }
}

impl Display for StackMemory<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.offset {
            0 => write!(
                f,
                "{} [{}]",
                self.size.word().expect("cannot access stack for this size"),
                self.base
            ),
            _ => write!(
                f,
                "{} [{} - {}]",
                self.size.word().expect("cannot access stack for this size"),
                self.base,
                self.offset
            ),
        }
    }
}

impl<'a> Register<'a> {
    pub fn as_addr(self, size: Size) -> StackMemory<'a> {
        StackMemory {
            base: self,
            offset: 0,
            size: size,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Memory<'a> {
    StackMemory(StackMemory<'a>),
    GlobalMemory(GlobalMemory<'a>),
    IndexMemory(IndexMemory<'a>),
}
impl Display for Memory<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Memory::StackMemory(mem) => write!(f, "{}", mem),
            Memory::GlobalMemory(var) => write!(f, "{}", var),
            Memory::IndexMemory(ind) => write!(f, "{}", ind),
        }
    }
}

impl<'a> Into<Memory<'a>> for StackMemory<'a> {
    fn into(self) -> Memory<'a> {
        Memory::StackMemory(self)
    }
}

impl<'a> Into<Memory<'a>> for GlobalMemory<'a> {
    fn into(self) -> Memory<'a> {
        Memory::GlobalMemory(self)
    }
}

impl<'a> Into<Memory<'a>> for IndexMemory<'a> {
    fn into(self) -> Memory<'a> {
        Memory::IndexMemory(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Operand<'a> {
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

impl<'a> Into<Operand<'a>> for Memory<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Memory(self)
    }
}

impl<'a> Into<Operand<'a>> for Register<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Register(self)
    }
}

impl<'a> Into<Operand<'a>> for StackMemory<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Memory(Memory::StackMemory(self))
    }
}

impl<'a> Into<Operand<'a>> for GlobalMemory<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Memory(Memory::GlobalMemory(self))
    }
}

impl<'a> Into<Operand<'a>> for IndexMemory<'a> {
    fn into(self) -> Operand<'a> {
        Operand::Memory(Memory::IndexMemory(self))
    }
}

impl<'a> Register<'a> {
    pub fn with_offset(self, size: Size, offset: usize) -> StackMemory<'a> {
        StackMemory::new(self, size).with_offset(offset)
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
    Movsx {
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
    IDiv(Register<'a>),
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
        src: Memory<'a>,
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
    Jz(String),
    Jle(String),
    Jmp(String),
    Sete(Register<'a>),
    Setne(Register<'a>),
    Setg(Register<'a>),
    Setge(Register<'a>),
    Setl(Register<'a>),
    Setle(Register<'a>),
    Cwd,
    Cdq,
    Cqo,
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
                Command::Cdq => writeln!(self.output, "cdq"),
                Command::Cqo => writeln!(self.output, "cqo"),
                Command::Cwd => writeln!(self.output, "cwd"),
                Command::Push(v) => writeln!(self.output, "push {}", v),
                Command::Mov { dst, src } => writeln!(self.output, "mov {}, {}", dst, src),
                Command::Movzx { dst, src } => writeln!(self.output, "movzx {}, {}", dst, src),
                Command::Movsx { dst, src } => writeln!(self.output, "movsx {}, {}", dst, src),
                Command::Add { dst, src } => writeln!(self.output, "add {}, {}", dst, src),
                Command::Sub { dst, src } => writeln!(self.output, "sub {}, {}", dst, src),
                Command::Imul { dst, src } => writeln!(self.output, "imul {}, {}", dst, src),
                Command::Div(v) => writeln!(self.output, "div {}", v),
                Command::IDiv(v) => writeln!(self.output, "idiv {}", v),
                Command::Neg(dst) => writeln!(self.output, "neg {}", dst),
                Command::Ret => writeln!(self.output, "ret"),
                Command::Leave => writeln!(self.output, "leave"),
                Command::Jz(l) => writeln!(self.output, "jz {l}"),
                Command::Call { name } => writeln!(self.output, "call {}", name),
                Command::Xor { dst, src } => writeln!(self.output, "xor {}, {}", dst, src),
                Command::Not(r) => writeln!(self.output, "not {}", r),
                Command::Test { op1, op2 } => writeln!(self.output, "test {}, {}", op1, op2),
                Command::And { dst, src } => writeln!(self.output, "and {}, {}", dst, src),
                Command::Or { dst, src } => writeln!(self.output, "or {}, {}", dst, src),
                Command::Pop(v) => writeln!(self.output, "pop {}", v),
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

    pub fn external(&mut self, external_function_name: &str) -> std::io::Result<()> {
        self.flush()?;
        writeln!(self.output, "extern {external_function_name}")
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

    pub fn output(mut self) -> std::io::Result<W> {
        self.flush()?;
        Ok(self.output)
    }
}

#[derive(Debug, Clone)]
struct ActivationRecord<'a> {
    scope_name: &'a str,
    scope_level: usize,
    local_variables: Vec<(String, Size)>,
}

impl<'a> ActivationRecord<'a> {
    #[inline]
    pub fn size(&self) -> usize {
        self.local_variables
            .iter()
            .fold(0, |v, (_, size)| v + size.to_bytes())
    }

    #[inline]
    pub fn aligned_size(&self) -> usize {
        ((self.size() + 15) / 16) * 16
    }

    #[inline]
    pub fn get_variable_offset(&self, var_name: &str) -> Option<(usize, &Size)> {
        let mut sum = 0;
        self.local_variables.iter().rev().find_map(|(n, size)| {
            if n == var_name {
                Some((sum, size))
            } else {
                sum += size.to_bytes();
                None
            }
        })
    }
}

const AR_VAR_STR: &'static str = "== Activation Record Contents ==";
static AR_VAR_SEP: LazyLock<String> = LazyLock::new(|| "-".repeat(55));

impl<'a> Display for ActivationRecord<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = format!(
            "Activation Record {} - level {}",
            self.scope_name, self.scope_level
        );
        let header_length = header.len();
        let sep = "=".repeat(header_length);
        writeln!(f, "{header}")?;
        writeln!(f, "{sep}")?;
        writeln!(f, ">   total size: {:>4} bytes", self.size())?;
        writeln!(f, "> aligned size: {:>4} bytes", self.aligned_size())?;
        if self.local_variables.len() > 0 {
            writeln!(f, "{AR_VAR_STR}")?;
            self.local_variables
                .iter()
                .try_for_each(|(var_name, size)| {
                    writeln!(f, "{}", var_name)?;
                    writeln!(f, "  |   size: {:>3} bytes |", size.to_bytes())?;
                    writeln!(
                        f,
                        "  | offset: {:>3} bytes |",
                        self.get_variable_offset(var_name).unwrap().0
                    )
                })?;
            writeln!(f, "{}", AR_VAR_SEP.as_str())?;
        } else {
            let sep = "-".repeat(header_length);
            writeln!(f, "{sep}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct CallStack<'a> {
    stack: Vec<ActivationRecord<'a>>,
}

impl<'a> CallStack<'a> {
    pub fn new() -> Self {
        Self {
            stack: vec![ActivationRecord {
                scope_name: "global",
                scope_level: 0,
                local_variables: Vec::new(),
            }],
        }
    }

    #[inline]
    pub fn push_ar(&mut self, scope_name: &'a str) {
        debug!(target: "pascal::compiler", "Entering {} scope", scope_name);
        self.stack.push(ActivationRecord {
            scope_name,
            scope_level: self.stack.last().map(|ar| ar.scope_level + 1).unwrap_or(0),
            local_variables: Vec::new(),
        });
    }
    #[inline]
    pub fn pop_ar(&mut self) {
        if let Some(ar) = self.stack.last() {
            debug!(target: "pascal::compiler", "Leaving {} scope", ar.scope_name);
        }
        self.stack.pop().unwrap();
        if let Some(ar) = self.stack.last() {
            debug!(target: "pascal::compiler", "Entering {} scope", ar.scope_name);
        }
    }

    #[inline]
    pub fn push_var(&mut self, name: &str, size: Size) {
        self.stack
            .last_mut()
            .unwrap()
            .local_variables
            .push((name.into(), size));
    }

    #[inline]
    pub fn push_ptr(&mut self, name: &str) {
        self.stack
            .last_mut()
            .unwrap()
            .local_variables
            .push((name.into(), Size::S64bit));
    }

    #[inline]
    pub fn lookup_var_mem<'s>(&self, name: &'s str) -> StackMemory<'s> {
        self.stack
            .last()
            .unwrap()
            .get_variable_offset(name)
            .map(|(offset, size)| {
                Register::Rbp
                    .with_offset(size.clone(), offset + size.to_bytes())
                    .into()
            })
            .unwrap()
    }

    #[inline]
    pub fn lookup_var_addr<'s>(&self, name: &'s str) -> StackMemory<'s> {
        self.stack
            .last()
            .unwrap()
            .get_variable_offset(name)
            .map(|(offset, size)| Register::Rbp.with_offset(size.clone(), offset + 8).into())
            .unwrap()
    }

    #[inline]
    pub fn aligned_size(&self) -> Option<usize> {
        self.stack.last().map(|ar| ar.aligned_size())
    }
}

impl<'a> Display for CallStack<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CALL STACK")?;
        self.stack.iter().try_for_each(|ar| writeln!(f, "{ar}"))
    }
}

#[derive(Debug, Clone)]
pub struct Compiler<'a, W: Write> {
    asm: Assambler<'a, W>,
    call_stack: CallStack<'a>,
    current_l_num: u64,
    loop_exit_labels: Vec<String>,
    loop_start_labels: Vec<String>,
}

impl<'a, W: Write> Compiler<'a, W> {
    pub fn new(output: W) -> Result<Self> {
        Ok(Compiler {
            asm: Assambler::new(output, true),
            call_stack: CallStack::new(),
            current_l_num: 0,
            loop_exit_labels: Vec::new(),
            loop_start_labels: Vec::new(),
        })
    }

    pub fn compile(mut self, tree: &'a Tree, semantic_metadata: &'a SemanticMetadata) -> Result<W> {
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
        default_value: Option<ExprRef>,
        var: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<Option<(&'a str, Size, ExprRef)>> {
        let var_name = tree.get_var_name(var).unwrap();
        let var_size = semantic_metadata
            .get_expr_type(var)
            .unwrap()
            .get_size(semantic_metadata)
            .unwrap();
        self.call_stack.push_var(var_name, var_size.clone());
        Ok(default_value.map(|v| (var_name, var_size, v)))
    }

    fn visit_callable_decl(
        &mut self,
        callable: &Decl,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
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
                    } => self.enter_scope(
                        func_name,
                        params,
                        declarations,
                        statements,
                        return_type.map(|t| *semantic_metadata.type_type_map.get(&t).unwrap()),
                        tree,
                        semantic_metadata,
                    ),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    fn enter_func(&mut self, aligned_size: usize) -> Result<()> {
        self.asm.push_cmd(Command::Push(Register::Rbp.into()));
        self.asm.push_cmd(Command::Mov {
            dst: Register::Rbp.into(),
            src: Register::Rsp.into(),
        });
        if aligned_size > 0 {
            self.asm.push_cmd(Command::Sub {
                dst: Register::Rsp,
                src: Register::Integer(aligned_size as i32),
            });
        }
        Ok(())
    }

    fn enter_scope(
        &mut self,
        scope_name: &'a str,
        params: &[Param],
        declarations: &[Decl],
        statements: &StmtRef,
        return_type: Option<TypeSymbolRef>,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        self.call_stack.push_ar(scope_name);
        self.asm
            .comment(&format!("{scope_name} function entry point"))?;
        self.asm.label(scope_name)?;
        self.asm.comment("block")?;
        let defaults = declarations
            .iter()
            .filter(|d| matches!(d, Decl::VarDecl { .. }))
            .map(|decl| match decl {
                Decl::VarDecl {
                    default_value,
                    var,
                    type_node: _,
                } => self.visit_var_decl_local(*default_value, var, tree, semantic_metadata),
                _ => unreachable!(),
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>>>()?;
        let param_names: Vec<(&str, Size, &VarPassMode)> = params
            .iter()
            .map(|param| {
                (
                    tree.get_var_name(&param.var).unwrap(),
                    semantic_metadata
                        .get_expr_type(&param.var)
                        .unwrap()
                        .get_size(semantic_metadata)
                        .unwrap(),
                    semantic_metadata.get_var_pass_mode(&param.var).unwrap(),
                )
            })
            .collect();
        param_names
            .iter()
            .for_each(|(param_name, size, pass_mode)| {
                if matches!(pass_mode, VarPassMode::Ref) {
                    self.call_stack.push_ptr(param_name);
                } else {
                    self.call_stack.push_var(param_name, size.clone());
                }
            });
        if let Some(return_type_ref) = return_type {
            let return_size = semantic_metadata
                .types
                .get(return_type_ref)
                .get_size(semantic_metadata)
                .unwrap();
            self.call_stack.push_var("result", return_size);
        }
        let local_size = self.call_stack.aligned_size().unwrap();
        self.enter_func(32 + local_size)?;
        param_names
            .iter()
            .enumerate()
            .try_for_each(|(i, (param_name, size, pass_mode))| {
                let reg = Register::from_param_index64(i);
                match pass_mode {
                    VarPassMode::Val => {
                        let mem = self.call_stack.lookup_var_mem(param_name);
                        self.asm.push_cmd(Command::Mov {
                            dst: mem.into(),
                            src: reg.to_size(&size).into(),
                        });
                    }
                    VarPassMode::Ref => {
                        let mem = self.call_stack.lookup_var_addr(param_name);
                        self.asm.push_cmd(Command::Mov {
                            dst: mem.into(),
                            src: reg.into(),
                        });
                    }
                }
                Ok::<(), Error>(())
            })?;
        defaults.into_iter().try_for_each(|(var_name, size, v)| {
            self.visit_expr(&v, tree, semantic_metadata)?;
            self.asm.push_cmd(Command::Pop(Register::Rax.into()));
            self.asm.push_cmd(Command::Mov {
                dst: self.call_stack.lookup_var_mem(var_name).into(),
                src: Register::Rax.to_size(&size).into(),
            });
            Ok::<(), Error>(())
        })?;
        debug!(target: "pascal::compiler", "{}", self.call_stack);
        self.visit_stmt(statements, tree, semantic_metadata)?;
        if let Some(return_type) = return_type {
            let return_size = semantic_metadata
                .types
                .get(return_type)
                .get_size(semantic_metadata)
                .unwrap();
            self.asm.push_cmd(Command::Mov {
                dst: Register::Rax.to_size(&return_size).into(),
                src: self.call_stack.lookup_var_mem("result").into(),
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
    ) -> Result<()> {
        debug!(target: "pascal::compiler", "Entering global scope");
        self.asm.directive("section .data")?;
        let global_vars = declarations
            .iter()
            .filter(|d| matches!(d, Decl::VarDecl { .. }))
            .map(|d| match d {
                Decl::VarDecl {
                    default_value,
                    var,
                    type_node: _,
                } => (
                    tree.get_var_name(var).unwrap(),
                    semantic_metadata
                        .get_expr_type(var)
                        .unwrap()
                        .get_size(semantic_metadata)
                        .expect("size is expected"),
                    default_value,
                ),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        global_vars
            .iter()
            .filter(|(.., default)| default.is_some())
            .map(|(name, size, value)| (name, size, value.unwrap()))
            .try_for_each(|(name, size, value)| {
                let value = tree.expr_pool.get(value).into_value(tree).unwrap();
                self.asm.directive(&format!(
                    "{name} {} {value}",
                    size.d()
                        .unwrap_or_else(|| panic!("cannot access d for size {:?}", size))
                ))?;
                Ok::<(), Error>(())
            })?;
        self.asm.newline()?;
        self.asm.directive("section .bss")?;
        global_vars
            .iter()
            .filter(|(.., defualt)| defualt.is_none())
            .try_for_each(|(name, size, _)| {
                let (res, l) = size.res();
                self.asm.directive(&format!("{name} {} {}", res, l))
            })?;
        self.asm.newline()?;
        self.asm.directive("section .text")?;
        self.asm.directive("global main")?;
        self.asm.external(STD_DIV0_ERROR)?;
        self.asm.external(STD_ARR_INDEX_OUT_OF_BOUNDS_ERROR)?;
        self.asm.external(BUILTIN_CALLABLES.get("write").unwrap())?;
        self.asm
            .external(BUILTIN_CALLABLES.get("writeln").unwrap())?;
        self.asm.newline()?;
        declarations
            .iter()
            .filter(|d| matches!(d, Decl::Callable { .. }))
            .try_for_each(|c| self.visit_callable_decl(c, tree, semantic_metadata))?;
        self.asm.comment("main entry point")?;
        self.asm.label("main")?;
        self.asm.comment("block")?;
        let local_size = self.call_stack.aligned_size().unwrap();
        self.enter_func(32 + local_size)?;
        self.visit_stmt(statements, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Xor {
            dst: Register::Eax,
            src: Register::Eax,
        });
        self.asm.push_cmd(Command::Add {
            dst: Register::Rsp,
            src: Register::Integer(32 + local_size as i32),
        });
        self.asm.push_cmd(Command::Leave);
        self.asm.push_cmd(Command::Ret);
        self.asm.comment("end block")?;
        Ok(())
    }

    fn get_variable_memory_address(
        &self,
        var: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Memory<'a> {
        let var_name = tree.get_var_name(var).unwrap();
        let var_size = semantic_metadata
            .get_expr_type(var)
            .unwrap()
            .get_size(semantic_metadata)
            .expect("size is expected");
        match semantic_metadata.var_types.get(var).unwrap() {
            VarLocality::Local => self.call_stack.lookup_var_mem(var_name).into(),
            VarLocality::Global => GlobalMemory::new(var_name, var_size).into(),
        }
    }

    fn visit_stmt(
        &mut self,
        stmt: &StmtRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        match tree.stmt_pool.get(*stmt) {
            Stmt::Program { name: _, block } => self.visit_stmt(block, tree, semantic_metadata),
            Stmt::Block {
                declarations,
                statements,
            } => self.enter_global_scope(declarations, statements, tree, semantic_metadata),
            Stmt::Compound(stmts) => stmts
                .iter()
                .try_for_each(|v| self.visit_stmt(v, tree, semantic_metadata).into()),
            Stmt::NoOp => Ok(()),
            Stmt::Call { call } => self.visit_call(call, tree, semantic_metadata),
            Stmt::Assign { left, right } => self.visit_assign(left, right, tree, semantic_metadata),
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => self.visit_if(
                cond,
                elifs,
                else_statement.as_ref(),
                tree,
                semantic_metadata,
            ),
            Stmt::While { cond, body } => self.visit_while(cond, body, tree, semantic_metadata),
            Stmt::For {
                var,
                init,
                end,
                body,
            } => self.visit_for(var, init, end, body, tree, semantic_metadata),
            Stmt::Break => Ok(self.visit_break()),
            Stmt::Continue => Ok(self.visit_continue()),
            Stmt::Exit(val) => self.visit_exit(val.as_ref(), tree, semantic_metadata),
        }
    }

    #[inline]
    fn visit_assign(
        &mut self,
        left: &ExprRef,
        right: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        match tree.expr_pool.get(*left) {
            Expr::Var { .. } => self.visit_assign_var(left, right, tree, semantic_metadata),
            Expr::Index {
                other_indicies: _,
                base,
                index_value,
            } => self.visit_assign_index(base, index_value, right, tree, semantic_metadata),
            _ => unreachable!("assign should only be for variable or index"),
        }
    }

    #[inline]
    fn setup_array_index(
        &mut self,
        base_type: &TypeSymbol,
        index_size: &Size,
        register: &'a Register,
    ) -> Result<()> {
        self.asm.push_cmd(Command::Pop(register.clone().into()));
        let (start_ord_index, end_ord_index) = base_type.get_limits().unwrap();
        if index_size < &Size::S64bit {
            self.asm.push_cmd(Command::Movsx {
                dst: register.clone(),
                src: register.clone().to_size(index_size).into(),
            });
        }
        self.asm.push_cmd(Command::Cmp {
            op1: register.clone().to_size(index_size).into(),
            op2: Register::Integer(start_ord_index).into(),
        });
        self.asm
            .push_cmd(Command::Jl(STD_ARR_INDEX_OUT_OF_BOUNDS_ERROR.into()));
        self.asm.push_cmd(Command::Cmp {
            op1: register.clone().to_size(index_size).into(),
            op2: Register::Integer(end_ord_index).into(),
        });
        self.asm
            .push_cmd(Command::Jg(STD_ARR_INDEX_OUT_OF_BOUNDS_ERROR.into()));
        if start_ord_index != 0 {
            self.asm.push_cmd(Command::Sub {
                dst: register.clone().to_size(index_size),
                src: Register::Integer(start_ord_index),
            });
        }
        Ok(())
    }

    #[inline]
    fn visit_assign_index(
        &mut self,
        base: &ExprRef,
        index_value: &ExprRef,
        right: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        let var_name = tree.get_var_name(base).unwrap();
        let left_type = semantic_metadata.get_expr_type(base).unwrap();
        let left_size = left_type.get_size(semantic_metadata).unwrap();
        let left_size = left_size.get_element_size().unwrap();
        let right_size = &semantic_metadata
            .get_expr_type(right)
            .unwrap()
            .get_size(semantic_metadata)
            .expect("size is expected");
        let pass_mode = semantic_metadata.get_var_pass_mode(base).unwrap();
        self.visit_expr(index_value, tree, semantic_metadata)?;
        self.visit_expr(right, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        self.setup_array_index(left_type, right_size, &Register::Rcx)?;
        match pass_mode {
            VarPassMode::Val => {
                self.asm.push_cmd(Command::Lea {
                    dst: Register::Rbx,
                    src: GlobalMemory::new(var_name, Size::S64bit).into(),
                });
                let source_op = match semantic_metadata.var_types.get(base).unwrap() {
                    VarLocality::Local => todo!(),
                    VarLocality::Global => IndexMemory {
                        register: Register::Rbx,
                        index: Register::Rcx,
                        size: left_size.clone(),
                    }
                    .into(),
                };
                if left_size > right_size {
                    self.asm.push_cmd(Command::Movsx {
                        dst: Register::Rax.to_size(&left_size).into(),
                        src: Register::Rax.to_size(&right_size).into(),
                    });
                    self.asm.push_cmd(Command::Mov {
                        dst: source_op,
                        src: Register::Rax.to_size(&left_size).into(),
                    });
                } else {
                    self.asm.push_cmd(Command::Mov {
                        dst: source_op,
                        src: Register::Rax.to_size(&left_size).into(),
                    });
                }
            }
            VarPassMode::Ref => {
                todo!();
                let var_addr = self.call_stack.lookup_var_addr(var_name);
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rbx.into(),
                    src: var_addr.into(),
                });
                if left_size > right_size {
                    self.asm.push_cmd(Command::Movsx {
                        dst: Register::Rax.to_size(&left_size).into(),
                        src: Register::Rax.to_size(&right_size).into(),
                    });
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Rbx.as_addr(left_size.clone()).into(),
                        src: Register::Rax.to_size(&left_size).into(),
                    });
                } else {
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Rbx.as_addr(left_size.clone()).into(),
                        src: Register::Rax.to_size(&left_size).into(),
                    });
                }
            }
        };
        Ok(())
    }

    #[inline]
    fn visit_assign_var(
        &mut self,
        left: &ExprRef,
        right: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        let var_name = tree.get_var_name(left).unwrap();
        let left_size = semantic_metadata
            .get_expr_type(left)
            .unwrap()
            .get_size(semantic_metadata)
            .expect("size is epxected");
        let right_size = semantic_metadata
            .get_expr_type(right)
            .unwrap()
            .get_size(semantic_metadata)
            .expect("size is expected");
        let pass_mode = semantic_metadata.get_var_pass_mode(left).unwrap();
        self.visit_expr(right, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        match pass_mode {
            VarPassMode::Val => {
                let source_op = match semantic_metadata.var_types.get(left).unwrap() {
                    VarLocality::Local => self.call_stack.lookup_var_mem(var_name).into(),
                    VarLocality::Global => GlobalMemory::new(var_name, left_size.clone()).into(),
                };
                if left_size > right_size {
                    self.asm.push_cmd(Command::Movsx {
                        dst: Register::Rax.to_size(&left_size).into(),
                        src: Register::Rax.to_size(&right_size).into(),
                    });
                    self.asm.push_cmd(Command::Mov {
                        dst: source_op,
                        src: Register::Rax.to_size(&left_size).into(),
                    });
                } else {
                    self.asm.push_cmd(Command::Mov {
                        dst: source_op,
                        src: Register::Rax.to_size(&left_size).into(),
                    });
                }
            }
            VarPassMode::Ref => {
                let var_addr = self.call_stack.lookup_var_addr(var_name);
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rbx.into(),
                    src: var_addr.into(),
                });
                if left_size > right_size {
                    self.asm.push_cmd(Command::Movsx {
                        dst: Register::Rdx.to_size(&left_size).into(),
                        src: Register::Rax.to_size(&right_size).into(),
                    });
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Rbx.as_addr(left_size.clone()).into(),
                        src: Register::Rdx.to_size(&left_size).into(),
                    });
                } else {
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Rbx.as_addr(left_size.clone()).into(),
                        src: Register::Rax.to_size(&left_size).into(),
                    });
                }
            }
        };
        Ok(())
    }

    #[inline]
    fn visit_if(
        &mut self,
        cond: &Condition,
        elifs: &[Condition],
        else_statement: Option<&StmtRef>,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
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
        let mut elifs_iter = elifs.iter().peekable();
        while let Some(elif) = elifs_iter.next() {
            self.asm.push_cmd(Command::Jmp(end_l.clone()));
            self.asm.label(&else_l)?;
            else_l = self.next_l("else");
            self.visit_expr(&elif.cond, tree, semantic_metadata)?;
            self.asm.push_cmd(Command::Pop(Register::Rax.into()));
            self.asm.push_cmd(Command::Cmp {
                op1: Register::Al.into(),
                op2: Register::Integer(0).into(),
            });
            if elifs_iter.peek().is_some() || else_statement.is_some() {
                self.asm.push_cmd(Command::Je(else_l.clone()));
            } else {
                self.asm.push_cmd(Command::Je(end_l.clone()));
            }
            self.visit_stmt(&elif.expr, tree, semantic_metadata)?;
        }
        if let Some(else_stmt) = else_statement {
            self.asm.push_cmd(Command::Jmp(end_l.clone()));
            self.asm.label(&else_l)?;
            self.visit_stmt(else_stmt, tree, semantic_metadata)?;
            self.asm.label(&end_l)?;
        } else if elifs.len() > 0 {
            self.asm.label(&end_l)?;
        } else {
            self.asm.label(&else_l)?;
        }
        Ok(())
    }

    #[inline]
    fn visit_while(
        &mut self,
        cond: &ExprRef,
        body: &StmtRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        let loop_l = self.next_l("while");
        let loop_end_l = self.next_l("endwhile");
        self.enter_loop(&loop_l, &loop_end_l);
        self.asm.label(&loop_l)?;
        self.visit_expr(cond, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        self.asm.push_cmd(Command::Cmp {
            op1: Register::Al.into(),
            op2: Register::Integer(0).into(),
        });
        self.asm.push_cmd(Command::Je(loop_end_l.clone()));
        self.visit_stmt(body, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Jmp(loop_l));
        self.asm.label(&loop_end_l)?;
        self.exit_loop();
        Ok(())
    }

    #[inline]
    fn visit_for(
        &mut self,
        var: &ExprRef,
        init: &ExprRef,
        end: &ExprRef,
        body: &StmtRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        let var_mem =
            Operand::Memory(self.get_variable_memory_address(var, tree, semantic_metadata));
        let var_size = semantic_metadata
            .get_expr_type(var)
            .unwrap()
            .get_size(semantic_metadata)
            .expect("size is expected");
        self.visit_expr(init, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        self.asm
            .push_cmd(Command::Dec(Register::Rax.to_size(&var_size)));
        self.asm.push_cmd(Command::Mov {
            dst: var_mem.clone(),
            src: Register::Rax.to_size(&var_size).into(),
        });
        self.visit_expr(end, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        self.asm.push_cmd(Command::Push(Register::Rax.into()));
        let l1 = self.next_l("for_body");
        let l2 = self.next_l("endfor");
        self.enter_loop(&l1, &l2);
        self.asm.label(&l1)?;
        self.asm.push_cmd(Command::Mov {
            dst: Register::Rax.to_size(&var_size).into(),
            src: var_mem.clone().into(),
        });
        self.asm
            .push_cmd(Command::Inc(Register::Rax.to_size(&var_size).into()));
        self.asm.push_cmd(Command::Mov {
            dst: var_mem.into(),
            src: Register::Rax.to_size(&var_size).into(),
        });
        self.asm.push_cmd(Command::Cmp {
            op1: Register::Rax.to_size(&var_size).into(),
            op2: StackMemory::new(Register::Rsp, var_size).into(),
        });
        self.asm.push_cmd(Command::Jg(l2.clone()));
        self.visit_stmt(body, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Jmp(l1.clone()));
        self.asm.label(&l2)?;
        self.asm.push_cmd(Command::Pop(Register::Rdx.into()));
        self.exit_loop();
        Ok(())
    }

    #[inline]
    fn visit_break(&mut self) {
        let end_l = self
            .loop_exit_labels
            .last()
            .expect("break should not be outside of the loop");
        self.asm.push_cmd(Command::Jmp(end_l.clone()));
    }

    #[inline]
    fn visit_continue(&mut self) {
        let start_l = self
            .loop_start_labels
            .last()
            .expect("continue should be within a loop");
        self.asm.push_cmd(Command::Jmp(start_l.clone()));
    }

    #[inline]
    fn visit_exit(
        &mut self,
        val: Option<&ExprRef>,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
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

    fn visit_expr(
        &mut self,
        expr: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        match tree.expr_pool.get(*expr) {
            Expr::LiteralInteger(i) => self.visit_literal_integer(i),
            Expr::LiteralBool(b) => self.visit_literal_bool(b),
            Expr::Var { .. } => self.visit_var(expr, semantic_metadata)?,
            Expr::UnaryOp { op, expr } => self.visit_unary_op(op, expr, tree, semantic_metadata)?,
            Expr::BinOp { op, left, right } => {
                self.visit_bin_op(op, left, right, tree, semantic_metadata)?
            }
            Expr::Call { .. } => self.visit_call(expr, tree, semantic_metadata)?,
            Expr::Index {
                other_indicies: _,
                base,
                index_value,
            } => self.visit_index(base, index_value, tree, semantic_metadata)?,
            _ => todo!(),
        };
        self.asm.push_cmd(Command::Push(Register::Rax.into()));
        Ok(())
    }

    #[inline]
    fn visit_index(
        &mut self,
        base: &ExprRef,
        index_value: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        let arr_kind = semantic_metadata.get_var_pass_mode(base).unwrap();
        match arr_kind {
            VarPassMode::Val => {
                let arr_name = tree.get_var_name(base).unwrap();
                let arr_type = semantic_metadata.get_expr_type(base).unwrap();
                let arr_size = arr_type.get_size(semantic_metadata).unwrap();
                let arr_element_size = arr_size.get_element_size().unwrap();
                let right_size = semantic_metadata
                    .get_expr_type(index_value)
                    .unwrap()
                    .get_size(semantic_metadata)
                    .unwrap();
                self.visit_expr(index_value, tree, semantic_metadata)?;
                self.setup_array_index(arr_type, &right_size, &Register::Rax)?;
                self.asm.push_cmd(Command::Lea {
                    dst: Register::Rbx,
                    src: GlobalMemory::new(arr_name, Size::S64bit).into(),
                });
                self.asm.push_cmd(Command::Mov {
                    dst: Register::Rax.to_size(arr_element_size).into(),
                    src: IndexMemory {
                        register: Register::Rbx,
                        index: Register::Rax,
                        size: arr_element_size.clone(),
                    }
                    .into(),
                });
            }
            VarPassMode::Ref => todo!(),
        }
        Ok(())
    }

    #[inline]
    fn visit_call(
        &mut self,
        call: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        match tree.expr_pool.get(*call) {
            Expr::Call { name, args } => {
                let mut func_name = name.lexem(tree.source_code);
                if let Some(&callable_name) = BUILTIN_CALLABLES.get(func_name) {
                    func_name = callable_name;
                }
                let callable_symbol = semantic_metadata.get_callable_symbol(call).unwrap();
                let params = callable_symbol
                    .params
                    .iter()
                    .cycle()
                    .take(args.len())
                    .zip(args)
                    .collect::<Vec<_>>();
                params
                    .iter()
                    .filter(|(v, _)| {
                        matches!(
                            semantic_metadata.vars.get(**v).pass_mode().unwrap(),
                            VarPassMode::Val
                        )
                    })
                    .try_for_each(|(_, arg)| {
                        self.visit_expr(arg, tree, semantic_metadata)?;
                        Ok::<(), Error>(())
                    })?;
                params
                    .iter()
                    .enumerate()
                    .rev()
                    .try_for_each(|(i, &(var_symbol, arg))| {
                        let symbol = semantic_metadata.vars.get(*var_symbol);
                        let reg = Register::from_param_index64(i);
                        let param_mode = symbol.pass_mode().unwrap();
                        let left_size = symbol.get_size(semantic_metadata).unwrap();
                        let right_size = semantic_metadata
                            .get_expr_type(arg)
                            .unwrap()
                            .get_size(semantic_metadata)
                            .expect("size is expected");
                        match param_mode {
                            VarPassMode::Val => {
                                self.asm.push_cmd(Command::Pop(reg.clone().into()));
                                if left_size > right_size {
                                    self.asm.push_cmd(Command::Movsx {
                                        dst: reg.clone().into(),
                                        src: reg.to_size(&right_size).into(),
                                    });
                                }
                            }
                            VarPassMode::Ref => {
                                let var_pass_mode =
                                    semantic_metadata.get_var_pass_mode(arg).unwrap();
                                match var_pass_mode {
                                    VarPassMode::Val => {
                                        self.asm.push_cmd(Command::Lea {
                                            dst: reg.into(),
                                            src: self.get_variable_memory_address(
                                                arg,
                                                tree,
                                                semantic_metadata,
                                            ),
                                        });
                                    }
                                    VarPassMode::Ref => {
                                        self.asm.push_cmd(Command::Mov {
                                            dst: reg.into(),
                                            src: Operand::Memory(self.get_variable_memory_address(
                                                arg,
                                                tree,
                                                semantic_metadata,
                                            )),
                                        });
                                    }
                                }
                            }
                        }
                        Ok::<(), Error>(())
                    })?;
                self.asm.push_cmd(Command::Call { name: func_name });
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    #[inline]
    fn visit_var(&mut self, expr: &ExprRef, semantic_metadata: &'a SemanticMetadata) -> Result<()> {
        match semantic_metadata.get_var_symbol(expr).unwrap() {
            VarSymbol::Var {
                name,
                pass_mode,
                type_symbol,
            } => {
                let var_size = semantic_metadata
                    .types
                    .get(*type_symbol)
                    .get_size(semantic_metadata)
                    .expect("size is expected");
                match pass_mode {
                    VarPassMode::Val => {
                        let source_op = match semantic_metadata.var_types.get(expr).unwrap() {
                            VarLocality::Local => self.call_stack.lookup_var_mem(name).into(),
                            VarLocality::Global => GlobalMemory::new(name, var_size.clone()).into(),
                        };
                        self.asm.push_cmd(Command::Mov {
                            dst: Register::Rax.to_size(&var_size).into(),
                            src: source_op,
                        });
                    }
                    VarPassMode::Ref => {
                        let var_mem = self.call_stack.lookup_var_addr(name);
                        self.asm.push_cmd(Command::Mov {
                            dst: Register::Rax.into(),
                            src: var_mem.into(),
                        });
                        self.asm.push_cmd(Command::Mov {
                            dst: Register::Rax.to_size(&var_size).into(),
                            src: Register::Rax.as_addr(var_size.clone()).into(),
                        });
                    }
                };
            }
            VarSymbol::Const {
                value,
                type_symbol: _,
            } => match value {
                ConstValue::Integer(i) => {
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Rax.into(),
                        src: Register::Integer(*i).into(),
                    });
                }
                ConstValue::Boolean(b) => {
                    let val = match b {
                        true => 1,
                        false => 0,
                    };
                    self.asm.push_cmd(Command::Mov {
                        dst: Register::Al.into(),
                        src: Register::Integer(val).into(),
                    });
                }
                _ => todo!(),
            },
        }
        Ok(())
    }

    #[inline]
    fn visit_literal_integer(&mut self, i: &i32) {
        self.asm.push_cmd(Command::Mov {
            dst: Register::Rax.into(),
            src: Register::Integer(*i).into(),
        })
    }

    #[inline]
    fn visit_literal_bool(&mut self, b: &bool) {
        let val = match b {
            true => 1,
            false => 0,
        };
        self.asm.push_cmd(Command::Mov {
            dst: Register::Rax.into(),
            src: Register::Integer(val).into(),
        });
    }

    #[inline]
    fn visit_unary_op(
        &mut self,
        op: &TokenType,
        expr: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        self.visit_expr(expr, tree, semantic_metadata)?;
        let var_size = semantic_metadata
            .get_expr_type(expr)
            .unwrap()
            .get_size(semantic_metadata)
            .expect("expected to have size");
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        match op {
            TokenType::Plus => {}
            TokenType::Minus => {
                self.asm
                    .push_cmd(Command::Neg(Register::Rax.to_size(&var_size)));
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
        };
        Ok(())
    }

    #[inline]
    fn visit_bin_op(
        &mut self,
        op: &TokenType,
        left: &ExprRef,
        right: &ExprRef,
        tree: &'a Tree,
        semantic_metadata: &'a SemanticMetadata,
    ) -> Result<()> {
        let left_size = semantic_metadata
            .get_expr_type(&left)
            .unwrap()
            .get_size(semantic_metadata)
            .unwrap();
        let right_size = semantic_metadata
            .get_expr_type(&right)
            .unwrap()
            .get_size(semantic_metadata)
            .expect("size is epxected");
        let expr_size = right_size.clone().max(left_size.clone());
        self.visit_expr(&left, tree, semantic_metadata)?;
        self.visit_expr(&right, tree, semantic_metadata)?;
        self.asm.push_cmd(Command::Pop(Register::Rbx.into()));
        self.asm.push_cmd(Command::Pop(Register::Rax.into()));
        if left_size > right_size {
            self.asm.push_cmd(Command::Movsx {
                dst: Register::Rbx.to_size(&expr_size).into(),
                src: Register::Rbx.to_size(&right_size).into(),
            });
        } else {
            self.asm.push_cmd(Command::Movsx {
                dst: Register::Rax.to_size(&expr_size).into(),
                src: Register::Rax.to_size(&left_size).into(),
            });
        }
        match op {
            TokenType::Plus => {
                self.asm.push_cmd(Command::Add {
                    dst: Register::Rax.to_size(&expr_size),
                    src: Register::Rbx.to_size(&expr_size),
                });
            }
            TokenType::Minus => {
                self.asm.push_cmd(Command::Sub {
                    dst: Register::Rax.to_size(&expr_size),
                    src: Register::Rbx.to_size(&expr_size),
                });
            }
            TokenType::Mul => {
                self.asm.push_cmd(Command::Imul {
                    dst: Register::Rax.to_size(&expr_size),
                    src: Register::Rbx.to_size(&expr_size),
                });
            }
            TokenType::IntegerDiv => {
                self.asm.push_cmd(expr_size.sign_extention().unwrap());
                self.asm.push_cmd(Command::Test {
                    op1: Register::Rbx.into(),
                    op2: Register::Rbx.into(),
                });
                self.asm.push_cmd(Command::Jz(STD_DIV0_ERROR.into()));
                self.asm
                    .push_cmd(Command::IDiv(Register::Rbx.to_size(&expr_size)));
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
            | TokenType::LessEqual => self.visit_comparison(&op, &expr_size)?,
            _ => unreachable!(),
        }
        Ok(())
    }

    #[inline]
    fn visit_comparison(&mut self, cmp_token: &TokenType, expr_size: &Size) -> Result<()> {
        self.asm.push_cmd(Command::Cmp {
            op1: Register::Rax.to_size(&expr_size).into(),
            op2: Register::Rbx.to_size(&expr_size).into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self};
    use std::process::{Command, Output};
    use std::sync::Once;

    static INIT: Once = Once::new();

    static ARTIFACT: &'static str = "test_artifacts";
    static GITIGNORE: &'static str = "test_artifacts/.gitignore";
    static TEST_CASES: &'static str = "test_cases/compiler";

    static STD_ERROR_SRC: &'static str = "lib/std.error.asm";
    static STD_ERROR_OBJ: &'static str = "test_artifacts/std.error.obj";
    static STD_IO_SRC: &'static str = "lib/std.io.asm";
    static STD_IO_OBJ: &'static str = "test_artifacts/std.io.obj";

    fn init() {
        INIT.call_once(|| {
            let _ = fs::create_dir(ARTIFACT);
            let mut gitignore = fs::File::create(GITIGNORE).unwrap();
            write!(gitignore, "*").unwrap();
            let result = Command::new("nasm")
                .arg("-f")
                .arg("win64")
                .arg("-g")
                .arg("-F")
                .arg("cv8")
                .arg("-o")
                .arg(STD_ERROR_OBJ)
                .arg(STD_ERROR_SRC)
                .output()
                .expect("failed to compile standard error");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8(result.stderr).unwrap()
            );
            let result = Command::new("nasm")
                .arg("-f")
                .arg("win64")
                .arg("-g")
                .arg("-F")
                .arg("cv8")
                .arg("-o")
                .arg(STD_IO_OBJ)
                .arg(STD_IO_SRC)
                .output()
                .expect("failed to compile standard io");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8(result.stderr).unwrap()
            );
        });
    }

    struct TestExecutable<'a>(&'a str);

    impl<'a> TestExecutable<'a> {
        fn create(name: &'a str) -> Self {
            init();
            let pas_path = format!("{}/{}.pas", TEST_CASES, name);
            let asm_path = format!("{}/{}.asm", ARTIFACT, name);
            let obj_path = format!("{}/{}.obj", ARTIFACT, name);
            let exe_path = format!("{}/{}.exe", ARTIFACT, name);
            let result = Command::new("cargo")
                .arg("run")
                .arg("compile")
                .arg(&pas_path)
                .arg(&asm_path)
                .output()
                .expect("failed to compile");
            assert!(
                result.status.success(),
                "failed to compile: {} - {}",
                String::from_utf8(result.stdout).unwrap(),
                String::from_utf8(result.stderr).unwrap()
            );
            let result = Command::new("nasm")
                .arg("-f")
                .arg("win64")
                .arg("-g")
                .arg("-F")
                .arg("cv8")
                .arg("-o")
                .arg(&obj_path)
                .arg(&asm_path)
                .output()
                .expect("failed to compile asm");
            assert!(
                result.status.success(),
                "failed to compile nasm: {}",
                String::from_utf8(result.stderr).unwrap()
            );
            let result = Command::new("gcc")
                .arg("-g")
                .arg("-o")
                .arg(&exe_path)
                .arg(&obj_path)
                .arg(STD_IO_OBJ)
                .arg(STD_ERROR_OBJ)
                .output()
                .expect("failed to create executable");
            assert!(
                result.status.success(),
                "failed to create executable: {}",
                String::from_utf8(result.stderr).unwrap()
            );
            Self(name)
        }

        fn run(&self) -> Output {
            Command::new(format!("{}/{}.exe", ARTIFACT, self.0))
                .output()
                .unwrap()
        }
    }

    impl<'a> Drop for TestExecutable<'a> {
        fn drop(&mut self) {
            Command::new("rm")
                // .arg(format!("{}/{}.exe", ARTIFACT, self.0))
                .arg(format!("{}/{}.obj", ARTIFACT, self.0))
                // .arg(format!("{}/{}.asm", ARTIFACT, self.0))
                .spawn()
                .expect("failed to remove the file");
        }
    }

    macro_rules! test_succ {
        (
            $(
                $name:ident ->
                [$($first_output:literal$(,$output:literal)*$(,)?)?],
            )+
        ) => {
            $(
                #[test]
                fn $name() {
                    let executable = TestExecutable::create(stringify!($name));
                    let output = executable.run();
                    assert!(output.status.success());
                    let mut _expected_output = Vec::<&str>::new();
                    $(
                        _expected_output.push($first_output);
                        $(
                            _expected_output.push($output);
                        )*
                    )?
                    let expected_output = _expected_output.join("\n");
                    let output = String::from_utf8(output.stdout).unwrap().trim().replace("\r", "");
                    assert_eq!(output, expected_output);
                }
            )+
        };
    }

    macro_rules! test_err {
        (
            $(
                $name:ident ->
                [$($first_err:literal$(,$err:literal)*$(,)?)?],
            )+
        ) => {
            $(
                #[test]
                fn $name() {
                    let executable = TestExecutable::create(stringify!($name));
                    let output = executable.run();
                    assert_eq!(output.status.code().unwrap(), 1);
                    let mut _expected_output = Vec::<&str>::new();
                    $(
                        _expected_output.push($first_err);
                        $(
                            _expected_output.push($err);
                        )*
                    )?
                    let expected_output = _expected_output.join("\n");
                    let output = String::from_utf8(output.stderr).unwrap().trim().replace("\r", "");
                    assert_eq!(output, expected_output);
                }
            )+
        };
    }

    test_succ! {
        test_simple_print -> ["-1"],
        test_simple_math -> ["35", "29", "-29", "96", "10"],
        test_expressions -> ["-16"],
        test_booleans -> ["0", "1"],
        test_if -> ["-1", "0", "1", "-1", "1", "1", "-1", "-1", "0"],
        test_loops -> ["-5", "-4", "-2", "-1", "5", "4", "3", "2", "1"],
        test_functions -> ["-10", "25"],
        test_out -> ["30", "30"],
        test_recursive -> ["120"],
        test_array -> ["10", "0", "1", "2", "3", "4", "5"],
    }

    test_err! {
        test_div0_error -> ["Runtime error: division by zero"],
        test_array_out_of_bounds_error1 -> ["Runtime error: array index is out of bounds"],
        test_array_out_of_bounds_error2 -> ["Runtime error: array index is out of bounds"],
        test_array_out_of_bounds_error3 -> ["Runtime error: array index is out of bounds"],
        test_array_out_of_bounds_error4 -> ["Runtime error: array index is out of bounds"],
    }
}

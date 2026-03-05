use std::{fmt::Display, hash::Hash, marker::PhantomData};

#[derive(Debug, Clone, Copy)]
pub struct Pos {
    pub row: u32,
    pub col: u32,
}

impl Pos {
    pub fn shift(&self, amount: u32) -> Self {
        Self {
            row: self.row,
            col: self.col - amount,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Span {
    start: u32,
    len: u32,
}

impl Span {
    pub fn new(start: u32, len: u32) -> Self {
        Self { start, len }
    }
    pub fn zero(pos: u32) -> Self {
        Self { start: pos, len: 0 }
    }

    pub fn lexem<'a>(&self, src: &'a str) -> &'a str {
        let s = self.start as usize;
        let e = s + self.len as usize;
        &src[s..e]
    }
    pub fn pos(&self, src: &str) -> Pos {
        let up_to = &src[0..self.start as usize];
        let rows: Vec<&str> = up_to.split("\n").collect();
        let row_count = rows.len() as u32;
        let col = rows.last().map_or(1, |r| r.len() + 1) as u32;
        Pos {
            row: row_count,
            col,
        }
    }
    pub fn total(&self, other: &Span) -> Span {
        Span {
            start: self.start,
            len: other.start + other.len - self.start,
        }
    }

    pub fn start(&self) -> u32 {
        self.start
    }
    #[cfg(test)]
    pub fn len(&self) -> u32 {
        self.len
    }
}

impl Hash for Span {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.start.hash(state);
        self.len.hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct NodePool<Ref, T> {
    items: Vec<T>,
    _id: PhantomData<Ref>,
}

impl<Ref, T> NodePool<Ref, T>
where
    Ref: From<usize> + Into<usize> + Copy,
{
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            _id: PhantomData,
        }
    }

    pub fn alloc(&mut self, node: T) -> Ref {
        let id = self.items.len();
        self.items.push(node);
        Ref::from(id)
    }

    pub fn get(&self, id: Ref) -> &T {
        &self.items[id.into()]
    }
}

#[derive(Debug, Clone)]
pub struct NodePoolWithSpan<Ref, T> {
    items: Vec<T>,
    spans: Vec<Span>,
    _id: PhantomData<Ref>,
}

impl<Ref, T> NodePoolWithSpan<Ref, T>
where
    Ref: From<usize> + Into<usize> + Copy,
{
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            spans: Vec::new(),
            _id: PhantomData,
        }
    }

    pub fn alloc(&mut self, node: T, span: Span) -> Ref {
        let id = self.items.len();
        self.items.push(node);
        self.spans.push(span);
        Ref::from(id)
    }

    pub fn get(&self, id: Ref) -> &T {
        &self.items[id.into()]
    }
    pub fn span(&self, id: Ref) -> Span {
        self.spans[id.into()]
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    #[cfg(test)]
    pub fn ids(&self) -> impl Iterator<Item = Ref> {
        (0..self.len()).map(|id| Ref::from(id))
    }
}

macro_rules! define_ref {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(u32);

        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(value as u32)
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> usize {
                value.0 as usize
            }
        }

        #[cfg(test)]
        impl Default for $name {
            fn default() -> Self {
                Self(0)
            }
        }
    };
}

pub(crate) use define_ref;
use itertools::Itertools;

use crate::parser::{Expr, Tree};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Size {
    SArray {
        element_size: Box<Size>,
        length: usize,
    },
    S8bit,
    S16bit,
    S32bit,
    S64bit,
    S128bit,
}

impl Size {
    pub fn get_element_size(&self) -> Option<&Size> {
        match self {
            Self::SArray {
                element_size,
                length: _,
            } => Some(element_size),
            _ => None,
        }
    }

    pub fn to_bytes(&self) -> usize {
        match self {
            Self::SArray {
                element_size,
                length,
            } => element_size.to_bytes() * length,
            Self::S8bit => 1,
            Self::S16bit => 2,
            Self::S32bit => 4,
            Self::S64bit => 8,
            Self::S128bit => 16,
        }
    }
}

impl Ord for Size {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let left = self.to_bytes();
        let right = other.to_bytes();
        left.cmp(&right)
    }
}

impl PartialOrd for Size {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Array(Vec<Option<Box<Value>>>),
    String(String),
    Integer(i32),
    Int64(i64),
    Real(f32),
    Char(char),
    Boolean(bool),
}

impl Value {
    pub fn repr(&self) -> String {
        match self {
            Self::Boolean(b) => b.to_string(),
            Self::Char(c) => c.to_string(),
            Self::Integer(i) => i.to_string(),
            Self::Int64(i) => i.to_string(),
            Self::Real(r) => r.to_string(),
            Self::String(s) => s.into(),
            Self::Array(vals) => format!(
                "[{}]",
                vals.iter()
                    .map(|v| v.as_deref().map_or("None".to_string(), |v| v.repr()))
                    .join(", ")
            ),
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int64(i) => write!(f, "{i}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Real(i) => write!(f, "{i}"),
            Value::String(i) => write!(f, "{i}"),
            Value::Boolean(b) => write!(
                f,
                "{}",
                match b {
                    true => 1,
                    false => 0,
                }
            ),
            Value::Char(c) => write!(f, "{c}"),
            _ => unimplemented!(),
        }
    }
}

impl Expr {
    pub fn as_value(&self, tree: &Tree) -> Option<Value> {
        match *self {
            Expr::LiteralBool(b) => Some(Value::Boolean(b)),
            Expr::LiteralChar(c) => Some(Value::Char(c)),
            Expr::LiteralInt64(i) => Some(Value::Int64(i)),
            Expr::LiteralInteger(i) => Some(Value::Integer(i)),
            Expr::LiteralReal(r) => Some(Value::Real(r)),
            Expr::Var { name } => Some(Value::String(name.lexem(tree.source_code).into())),
            _ => None,
        }
    }
}

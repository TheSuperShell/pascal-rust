use std::{hash::Hash, marker::PhantomData};

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
    pub fn len(&self) -> usize {
        self.items.len()
    }
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

        impl Into<usize> for $name {
            fn into(self) -> usize {
                self.0 as usize
            }
        }
    };
}

pub(crate) use define_ref;

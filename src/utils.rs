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
    pub start: u32,
    pub len: u32,
}

impl Span {
    pub fn lexem<'a>(&self, src: &'a str) -> &'a str {
        let s = self.start as usize;
        let e = s + self.len as usize;
        &src[s..e]
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

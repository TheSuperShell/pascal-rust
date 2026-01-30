use std::marker::PhantomData;

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

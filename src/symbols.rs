use std::collections::HashMap;

use crate::{parser::StmtRef, utils::define_ref};

define_ref!(TypeSymbolRef);
define_ref!(VarSymbolRef);
define_ref!(CallableSymbolRef);

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSymbol {
    Integer,
    Real,
    Boolean,
    String,
    Char,
    // Range(TypeSymbolRef),
    Array {
        index_type: TypeSymbolRef,
        value_type: TypeSymbolRef,
    },
    DynamicArray(TypeSymbolRef),
    Enum(Vec<String>),
}

#[derive(Debug, Clone)]
pub enum ConstValue {
    Integer(i64),
    Real(f64),
    String(String),
    Char(char),
    Boolean(bool),
}
#[derive(Debug, Clone)]
pub enum VarSymbol {
    Var {
        name: String,
        type_symbol: TypeSymbolRef,
    },
    Const {
        name: String,
        value: ConstValue,
    },
}

#[derive(Debug, Clone)]
pub enum CallableBody {
    BlockAST(StmtRef),
    Func(fn()),
}

#[derive(Debug, Clone)]
pub struct CallableSymbol {
    name: String,
    return_type: TypeSymbolRef,
    params: Vec<VarSymbolRef>,
    body: CallableBody,
}

#[derive(Debug, Clone)]
pub struct SymbolTable {
    type_symbols: HashMap<String, TypeSymbolRef>,
    var_symbols: HashMap<String, VarSymbolRef>,
    callable_symbols: HashMap<String, CallableSymbolRef>,
    scope_level: usize,
    scope_name: String,
    enclosing_scope: Option<Box<SymbolTable>>,
}

impl SymbolTable {
    pub fn new(
        scope_level: usize,
        scope_name: &str,
        enclosing_scope: Option<Box<SymbolTable>>,
    ) -> Self {
        Self {
            type_symbols: HashMap::new(),
            var_symbols: HashMap::new(),
            callable_symbols: HashMap::new(),
            scope_level,
            scope_name: scope_name.to_string(),
            enclosing_scope,
        }
    }

    pub fn define_type(&mut self, name: &str, symbol: TypeSymbolRef) {
        self.type_symbols.insert(name.to_string(), symbol);
    }

    pub fn define_var(&mut self, name: &str, symbol: VarSymbolRef) {
        self.var_symbols.insert(name.to_string(), symbol);
    }
    pub fn define_callable(&mut self, name: String, symbol: CallableSymbolRef) {
        self.callable_symbols.insert(name, symbol);
    }

    pub fn lookup_type(&self, name: &str, current_scope_only: bool) -> Option<TypeSymbolRef> {
        if self.type_symbols.contains_key(name) {
            return Some(self.type_symbols[name]);
        };
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_type(name, current_scope_only);
        };
        None
    }
    pub fn lookup_var(&self, name: &str, current_scope_only: bool) -> Option<VarSymbolRef> {
        if self.var_symbols.contains_key(name) {
            return Some(self.var_symbols[name]);
        };
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_var(name, current_scope_only);
        };
        None
    }
    pub fn lookup_callable(
        &self,
        name: &str,
        current_scope_only: bool,
    ) -> Option<CallableSymbolRef> {
        if self.var_symbols.contains_key(name) {
            return Some(self.callable_symbols[name]);
        };
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_callable(name, current_scope_only);
        };
        None
    }
}

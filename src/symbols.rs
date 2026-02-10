use std::collections::HashMap;

use crate::{
    error::Error,
    interpreter::{BuiltinCtx, Value},
    parser::StmtRef,
    utils::{NodePool, define_ref},
};

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
    Range(TypeSymbolRef),
    Array {
        index_type: TypeSymbolRef,
        value_type: TypeSymbolRef,
    },
    DynamicArray(TypeSymbolRef),
    Enum(Vec<String>),
    Any,
    Empty,
}

impl TypeSymbol {
    pub fn is_ordinal(&self) -> bool {
        matches!(
            self,
            Self::Integer | Self::Char | Self::Boolean | Self::Enum(_)
        )
    }
    pub fn oridnal_value(&self, index: i64) -> Value {
        match self {
            TypeSymbol::Integer => Value::Integer(index),
            TypeSymbol::Char => Value::Char(char::from_u32(index as u32).unwrap()),
            TypeSymbol::Boolean => Value::Boolean(index != 0),
            TypeSymbol::Enum(_) => Value::Integer(index),
            _ => unreachable!(),
        }
    }

    pub fn ordinal_rank(&self, value: &Value) -> i64 {
        match (self, value) {
            (Self::Enum(_), &Value::Integer(i)) => i,
            (Self::Integer, &Value::Integer(i)) => i,
            (Self::Char, &Value::Char(c)) => c as i64,
            (Self::Boolean, &Value::Boolean(b)) => b as i64,
            _ => unreachable!(),
        }
    }

    pub fn eq(
        node_pool: &NodePool<TypeSymbolRef, TypeSymbol>,
        left: &TypeSymbol,
        right: &TypeSymbol,
    ) -> bool {
        match (left, right) {
            (TypeSymbol::Range(t), _) => TypeSymbol::eq(node_pool, node_pool.get(*t), right),
            (_, TypeSymbol::Range(t)) => TypeSymbol::eq(node_pool, left, node_pool.get(*t)),
            (_, _) => left == right,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConstValue {
    Integer(i64),
    Real(f64),
    String(String),
    Char(char),
    Boolean(bool),
}

impl Into<Value> for ConstValue {
    fn into(self) -> Value {
        match self {
            ConstValue::Integer(i) => Value::Integer(i),
            ConstValue::Boolean(b) => Value::Boolean(b),
            ConstValue::Char(c) => Value::Char(c),
            ConstValue::String(s) => Value::String(s.into()),
            ConstValue::Real(r) => Value::Real(r),
        }
    }
}

#[derive(Debug, Clone)]
pub enum VarSymbol {
    Var {
        name: String,
        type_symbol: TypeSymbolRef,
    },
    Const {
        value: ConstValue,
        type_symbol: TypeSymbolRef,
    },
}

#[derive(Debug, Clone)]
pub struct RangeSymbol {
    lower_index: i64,
    higher_index: i64,
    type_symbol: TypeSymbol,
}

impl RangeSymbol {
    pub fn new(lower_value: &Value, upper_value: &Value, type_symbol: &TypeSymbol) -> Self {
        Self {
            lower_index: type_symbol.ordinal_rank(lower_value),
            higher_index: type_symbol.ordinal_rank(upper_value),
            type_symbol: type_symbol.clone(),
        }
    }

    pub fn len(&self) -> usize {
        (self.higher_index - self.lower_index).try_into().unwrap()
    }
    pub fn get_index(&self, value: &Value) -> usize {
        let ord = self.type_symbol.ordinal_rank(value);
        (ord - self.lower_index) as usize
    }
}

#[derive(Debug, Clone)]
pub enum LValue<'a> {
    Ref { name: &'a str },
    ArrIndex { name: &'a str, index: usize },
    Value(Value),
}

#[derive(Debug, Clone)]
pub enum CallableType {
    Custom {
        statement: StmtRef,
    },
    Builtin {
        func: fn(
            ctx: &mut dyn BuiltinCtx<Value = Value>,
            args: &[&LValue],
        ) -> Result<Option<Value>, Error>,
    },
}

#[derive(Debug, Clone)]
pub enum ParamMode {
    Var,
    Ref,
}

#[derive(Debug, Clone)]
pub enum ParamInputMode {
    Seq,
    Repeat,
}

#[derive(Debug, Clone)]
pub struct CallableSymbol {
    pub name: String,
    pub params: Vec<(VarSymbolRef, ParamMode)>,
    pub param_input_mode: ParamInputMode,
    pub body: CallableType,
    pub return_type: Option<TypeSymbolRef>,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    type_symbols: HashMap<String, TypeSymbolRef>,
    var_symbols: HashMap<String, VarSymbolRef>,
    callable_symbols: HashMap<String, CallableSymbolRef>,
    scope_name: String,
    scope_level: usize,
    enclosing_scope: Option<Box<SymbolTable>>,
}

impl ToString for SymbolTable {
    fn to_string(&self) -> String {
        format!("Scope {}", self.scope_name)
    }
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

    pub fn get_scope_level(&self) -> usize {
        self.scope_level
    }
    pub fn get_mut_enclosing_scope(&mut self) -> Option<&mut Box<SymbolTable>> {
        self.enclosing_scope.as_mut()
    }
    pub fn take_enclosing_scope(&mut self) -> Option<Box<SymbolTable>> {
        self.enclosing_scope.take()
    }

    pub fn define_type(&mut self, name: &str, symbol: TypeSymbolRef) {
        self.type_symbols.insert(name.to_lowercase(), symbol);
    }

    pub fn define_var(&mut self, name: &str, symbol: VarSymbolRef) {
        self.var_symbols.insert(name.to_lowercase(), symbol);
    }
    pub fn define_callable(&mut self, name: &str, symbol: CallableSymbolRef) {
        self.callable_symbols.insert(name.to_lowercase(), symbol);
    }

    pub fn lookup_type(&self, name: &str, current_scope_only: bool) -> Option<TypeSymbolRef> {
        let name = &name.to_lowercase();
        if self.type_symbols.contains_key(name) {
            return Some(self.type_symbols[name]);
        };
        if current_scope_only {
            return None;
        }
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_type(name, current_scope_only);
        };
        None
    }
    pub fn lookup_var(&self, name: &str, current_scope_only: bool) -> Option<VarSymbolRef> {
        let name = &name.to_lowercase();
        if self.var_symbols.contains_key(name) {
            return Some(self.var_symbols[name]);
        };
        if current_scope_only {
            return None;
        }
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
        let name = &name.to_lowercase();
        if self.callable_symbols.contains_key(name) {
            return Some(self.callable_symbols[name]);
        };
        if current_scope_only {
            return None;
        }
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_callable(name, current_scope_only);
        };
        None
    }
}

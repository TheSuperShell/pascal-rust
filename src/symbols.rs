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
}

impl TypeSymbol {
    pub fn is_ordinal(&self) -> bool {
        matches!(self, Self::Integer | Self::Char | Self::Boolean)
    }
    pub fn oridnal_value(&self, index: i64) -> Result<Value, Error> {
        match self {
            TypeSymbol::Integer => Ok(Value::Integer(index)),
            TypeSymbol::Char => Ok(Value::Char(char::from_u32(index as u32).unwrap())),
            TypeSymbol::Boolean => Ok(Value::Boolean(index != 0)),
            _ => Err(Error::InterpreterError {
                msg: format!("ordinal value is not supported for {:?}", self),
            }),
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
            ConstValue::String(s) => Value::String(s),
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
    },
}

#[derive(Debug, Clone)]
pub struct RangeSymbol<'a> {
    base_type: &'a TypeSymbol,
    lower_index: i64,
    higher_index: i64,
}

impl<'a> RangeSymbol<'a> {
    pub fn new(
        base_type: &'a TypeSymbol,
        lower_value: &Value,
        upper_value: &Value,
    ) -> Result<Self, Error> {
        Ok(Self {
            base_type,
            lower_index: lower_value.ordinal_rank()?,
            higher_index: upper_value.ordinal_rank()?,
        })
    }

    pub fn base_type(&self) -> &'a TypeSymbol {
        self.base_type
    }
    pub fn len(&self) -> usize {
        (self.higher_index - self.lower_index).try_into().unwrap()
    }
}

#[derive(Debug, Clone)]
pub enum LValue<'a> {
    Ref { name: &'a str },
    ArrIndex { name: &'a str, index: usize },
    Value(Value),
}

#[derive(Debug, Clone)]
pub enum CallableBody {
    BlockAST(StmtRef),
    Func(
        fn(
            ctx: &mut dyn BuiltinCtx<Value = Value>,
            args: &[&LValue],
        ) -> Result<Option<Value>, Error>,
    ),
}

#[derive(Debug, Clone)]
pub enum ParamMode {
    Var,
    Ref,
}

#[derive(Debug, Clone)]
pub struct CallableSymbol {
    pub name: String,
    pub return_type: Option<TypeSymbolRef>,
    pub params: Vec<(VarSymbolRef, ParamMode)>,
    pub body: CallableBody,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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

    pub fn get_scope_level(&self) -> usize {
        self.scope_level
    }
    pub fn get_mut_enclosing_scope(&mut self) -> Option<&mut Box<SymbolTable>> {
        self.enclosing_scope.as_mut()
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

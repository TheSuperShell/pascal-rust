use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    error::Error,
    interpreter::{BuiltinCtx, Value},
    parser::StmtRef,
    semantic_analyzer::SemanticMetadata,
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

    pub fn ordinal_rank(&self, value: &Value, semantic_metadata: &SemanticMetadata) -> i64 {
        match (self, value) {
            (&Self::Range(t), _) => semantic_metadata
                .types
                .get(t)
                .ordinal_rank(value, semantic_metadata),
            (Self::Enum(_), &Value::Integer(i)) => i,
            (Self::Integer, &Value::Integer(i)) => i,
            (Self::Char, &Value::Char(c)) => c as i64,
            (Self::Boolean, &Value::Boolean(b)) => b as i64,
            _ => unreachable!("incorrect ordinal invokation: {:?} <> {:?}", self, value),
        }
    }

    pub fn to_string(&self, value: Option<&Value>, semantic_metadata: &SemanticMetadata) -> String {
        match (self, value) {
            (&Self::Range(t), _) => semantic_metadata
                .types
                .get(t)
                .to_string(value, semantic_metadata),
            (Self::Integer, Some(Value::Integer(i))) => i.to_string(),
            (Self::Real, Some(Value::Real(r))) => r.to_string(),
            (Self::Boolean, Some(Value::Boolean(b))) => b.to_string(),
            (Self::Char, Some(Value::Char(c))) => c.to_string(),
            (Self::String, Some(Value::String(s))) => s.into(),
            (Self::Array { value_type, .. }, Some(Value::Array(vals))) => format!(
                "[{}]",
                vals.iter()
                    .map(|v| semantic_metadata
                        .types
                        .get(*value_type)
                        .to_string(v.as_deref(), semantic_metadata))
                    .join(", ")
            ),
            (Self::Enum(vals), Some(&Value::Integer(i))) => vals[i as usize].clone(),
            (_, None) => "None".to_string(),
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
    pub fn new(
        lower_value: &Value,
        upper_value: &Value,
        type_symbol: &TypeSymbol,
        semantic_metadata: &SemanticMetadata,
    ) -> Self {
        Self {
            lower_index: type_symbol.ordinal_rank(lower_value, semantic_metadata),
            higher_index: type_symbol.ordinal_rank(upper_value, semantic_metadata),
            type_symbol: type_symbol.clone(),
        }
    }

    pub fn len(&self) -> usize {
        (self.higher_index - self.lower_index).try_into().unwrap()
    }
    pub fn get_index(&self, value: &Value, semantic_metadata: &SemanticMetadata) -> usize {
        let ord = self.type_symbol.ordinal_rank(value, semantic_metadata);
        (ord - self.lower_index) as usize
    }
    pub fn within_bounds(&self, value: &Value, semantic_metadata: &SemanticMetadata) -> bool {
        let ind = self.type_symbol.ordinal_rank(value, semantic_metadata);
        ind >= self.lower_index && ind <= self.higher_index
    }
}

#[derive(Debug, Clone)]
pub enum LValue<'a> {
    Ref { name: &'a str },
    ArrIndex { name: &'a str, index: usize },
    Value(Value),
}

pub type BuiltinInput<'a> = &'a [(LValue<'a>, &'a TypeSymbol)];

#[derive(Debug, Clone)]
pub enum CallableType {
    Custom {
        statement: StmtRef,
    },
    Builtin {
        func: fn(
            ctx: &mut dyn BuiltinCtx<Value = Value>,
            semantic_metadata: &SemanticMetadata,
            args: BuiltinInput,
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

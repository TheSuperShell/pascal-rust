use std::fmt::Write;
use std::{collections::HashMap, sync::LazyLock};

use itertools::Itertools;
use tracing::debug;

use crate::utils::{Size, Value};
use crate::{
    error::Error,
    interpreter::BuiltinCtx,
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
    Int64,
    Real,
    Boolean,
    String,
    Char,
    Range {
        start_ord_index: i32,
        end_ord_index: i32,
        range_type: TypeSymbolRef,
    },
    Array {
        start_ord_index: i32,
        end_ord_index: i32,
        index_type: TypeSymbolRef,
        value_type: TypeSymbolRef,
    },
    DynamicArray(TypeSymbolRef),
    Enum(Vec<String>),
    Any,
    Empty,
}

impl TypeSymbol {
    pub fn get_limits(&self) -> Option<(i32, i32)> {
        match self {
            Self::Array {
                start_ord_index,
                end_ord_index,
                ..
            } => Some((*start_ord_index, *end_ord_index)),
            Self::Range {
                start_ord_index,
                end_ord_index,
                ..
            } => Some((*start_ord_index, *end_ord_index)),
            _ => None,
        }
    }

    pub fn get_size(&self, semantic_metadata: &SemanticMetadata) -> Option<Size> {
        match self {
            Self::Integer => Some(Size::S32bit),
            Self::Int64 => Some(Size::S64bit),
            Self::Real => Some(Size::S64bit),
            Self::Boolean => Some(Size::S8bit),
            Self::Char => Some(Size::S8bit),
            Self::Range {
                start_ord_index: _,
                end_ord_index: _,
                range_type,
            } => semantic_metadata
                .types
                .get(*range_type)
                .get_size(semantic_metadata),
            Self::Array {
                start_ord_index,
                end_ord_index,
                index_type: _,
                value_type,
            } => semantic_metadata
                .types
                .get(*value_type)
                .get_size(semantic_metadata)
                .map(|element_size| Size::SArray {
                    element_size: Box::new(element_size),
                    length: 1 + (end_ord_index - start_ord_index) as usize,
                }),
            Self::Enum(elements) => Some(Size::SArray {
                element_size: Box::new(Size::S8bit),
                length: elements.len(),
            }),
            Self::Any => Some(Size::S64bit),
            _ => None,
        }
    }
    pub fn is_ordinal(&self) -> bool {
        matches!(
            self,
            Self::Integer | Self::Int64 | Self::Char | Self::Boolean | Self::Enum(_)
        )
    }
    pub fn oridnal_value(&self, index: i32) -> Value {
        match self {
            TypeSymbol::Integer | TypeSymbol::Int64 => Value::Integer(index),
            TypeSymbol::Char => Value::Char(char::from_u32(index as u32).unwrap()),
            TypeSymbol::Boolean => Value::Boolean(index != 0),
            TypeSymbol::Enum(_) => Value::Integer(index),
            _ => unreachable!(),
        }
    }

    pub fn ordinal_rank(&self, value: &Value, semantic_metadata: &SemanticMetadata) -> i32 {
        match (self, value) {
            (
                &Self::Range {
                    start_ord_index: _,
                    end_ord_index: _,
                    range_type,
                },
                _,
            ) => semantic_metadata
                .types
                .get(range_type)
                .ordinal_rank(value, semantic_metadata),
            (Self::Enum(vals), Value::String(name)) => vals
                .iter()
                .enumerate()
                .find_map(|(i, val)| if val == name { Some(i as i32) } else { None })
                .unwrap(),
            (Self::Integer, &Value::Integer(i)) => i,
            (Self::Int64, &Value::Int64(i)) => i as i32,
            (Self::Char, &Value::Char(c)) => c as i32,
            (Self::Boolean, &Value::Boolean(b)) => b as i32,
            _ => unreachable!("incorrect ordinal invokation: {:?} <> {:?}", self, value),
        }
    }

    pub fn represent(&self, value: Option<&Value>, semantic_metadata: &SemanticMetadata) -> String {
        match (self, value) {
            (
                &Self::Range {
                    start_ord_index: _,
                    end_ord_index: _,
                    range_type,
                },
                _,
            ) => semantic_metadata
                .types
                .get(range_type)
                .represent(value, semantic_metadata),
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
                        .represent(v.as_deref(), semantic_metadata))
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
            (
                TypeSymbol::Range {
                    start_ord_index: _,
                    end_ord_index: _,
                    range_type: t,
                },
                _,
            ) => TypeSymbol::eq(node_pool, node_pool.get(*t), right),
            (
                _,
                TypeSymbol::Range {
                    start_ord_index: _,
                    end_ord_index: _,
                    range_type: t,
                },
            ) => TypeSymbol::eq(node_pool, left, node_pool.get(*t)),
            (_, _) => left == right,
        }
    }
    pub fn to_string(&self, semantic_metadata: &SemanticMetadata) -> String {
        let kind = match self {
            TypeSymbol::Boolean => "Boolean".into(),
            TypeSymbol::Any => "Any".into(),
            TypeSymbol::Array {
                start_ord_index: _,
                end_ord_index: _,
                index_type,
                value_type,
            } => format!(
                "Array[{}] of {}",
                semantic_metadata
                    .types
                    .get(*index_type)
                    .to_string(semantic_metadata),
                semantic_metadata
                    .types
                    .get(*value_type)
                    .to_string(semantic_metadata)
            ),
            TypeSymbol::Char => "Char".into(),
            TypeSymbol::DynamicArray(value_type) => format!(
                "Array of {}",
                semantic_metadata
                    .types
                    .get(*value_type)
                    .to_string(semantic_metadata)
            ),
            TypeSymbol::Empty => "Empty".into(),
            TypeSymbol::Enum(..) => "Enum".into(),
            TypeSymbol::Integer => "Integer".into(),
            TypeSymbol::Int64 => "Int64".into(),
            TypeSymbol::Range {
                start_ord_index: _,
                end_ord_index: _,
                range_type,
            } => format!(
                "Range of {}",
                semantic_metadata
                    .types
                    .get(*range_type)
                    .to_string(semantic_metadata)
            ),
            TypeSymbol::Real => "Real".into(),
            TypeSymbol::String => "String".into(),
        };
        format!("<Type Symbol:{}>", kind)
    }
}

#[derive(Debug, Clone)]
pub enum ConstValue {
    Integer(i32),
    Int64(i64),
    Real(f32),
    String(String),
    Char(char),
    Boolean(bool),
}

impl From<ConstValue> for Value {
    fn from(value: ConstValue) -> Self {
        match value {
            ConstValue::Int64(i) => Value::Int64(i),
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
        pass_mode: VarPassMode,
        type_symbol: TypeSymbolRef,
    },
    Const {
        value: ConstValue,
        type_symbol: TypeSymbolRef,
    },
}

impl VarSymbol {
    pub fn pass_mode(&self) -> Option<&VarPassMode> {
        match self {
            Self::Var { pass_mode, .. } => Some(pass_mode),
            _ => None,
        }
    }
    pub fn get_size(&self, semantic_metadata: &SemanticMetadata) -> Option<Size> {
        semantic_metadata
            .types
            .get(match self {
                Self::Const { type_symbol, .. } => *type_symbol,
                Self::Var { type_symbol, .. } => *type_symbol,
            })
            .get_size(semantic_metadata)
    }
    pub fn to_string(&self, semantic_metadata: &SemanticMetadata) -> String {
        match self {
            Self::Var {
                name,
                type_symbol,
                pass_mode,
            } => format!(
                "<Var{}:{}>:{}",
                match pass_mode {
                    VarPassMode::Val => "",
                    VarPassMode::Ref => " ref",
                },
                name,
                semantic_metadata
                    .types
                    .get(*type_symbol)
                    .to_string(semantic_metadata)
            ),
            Self::Const { value, type_symbol } => {
                let type_symbol = semantic_metadata.types.get(*type_symbol);
                format!(
                    "<Const:{}>:{}",
                    type_symbol.represent(Some(&value.clone().into()), semantic_metadata),
                    type_symbol.to_string(semantic_metadata)
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum VarLocality {
    Local,
    Global,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum VarPassMode {
    Val,
    Ref,
}

#[derive(Debug, Clone)]
pub struct RangeSymbol {
    lower_index: i32,
    higher_index: i32,
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
pub type BuiltinFunc = fn(
    ctx: &mut dyn BuiltinCtx<Value = Value>,
    semantic_metadata: &SemanticMetadata,
    args: BuiltinInput,
) -> Result<Option<Value>, Error>;

#[derive(Debug, Clone)]
pub enum CallableType {
    Custom { statement: StmtRef },
    Builtin { func: BuiltinFunc },
}

#[derive(Debug, Clone)]
pub enum ParamInputMode {
    Seq,
    Repeat,
}

#[derive(Debug, Clone)]
pub struct CallableSymbol {
    pub name: String,
    pub params: Vec<VarSymbolRef>,
    pub param_input_mode: ParamInputMode,
    pub body: CallableType,
    pub return_type: Option<TypeSymbolRef>,
}

impl CallableSymbol {
    pub fn to_string(&self, semantic_metadata: &SemanticMetadata) -> String {
        let mut buf = String::new();
        match self.body {
            CallableType::Builtin { .. } => write!(buf, "<BuilinCallable:{}>", self.name).unwrap(),
            CallableType::Custom { .. } => write!(buf, "<Callable:{}>", self.name).unwrap(),
        }
        let params = self
            .params
            .iter()
            .map(|v| semantic_metadata.vars.get(*v))
            .map(|v| {
                format!(
                    "{}{}",
                    v.to_string(semantic_metadata),
                    match v {
                        VarSymbol::Var { pass_mode, .. } => match pass_mode {
                            VarPassMode::Ref => "OUT",
                            VarPassMode::Val => "",
                        },
                        _ => unreachable!(),
                    }
                )
            })
            .join(", ");
        write!(buf, "({})", params).unwrap();
        if let Some(r) = self.return_type {
            write!(
                buf,
                " -> {}",
                semantic_metadata.types.get(r).to_string(semantic_metadata)
            )
            .unwrap();
        }
        buf
    }
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

const H1: &str = "SCOPE (SCOPED SYMBOL TABLE)";
const H2: &str = "Scope (SCOPED SYMBOL TABLE) contents";
static EQ_H1: LazyLock<String> =
    LazyLock::new(|| vec!["="].into_iter().cycle().take(H1.len()).join(""));
static EQ_H2: LazyLock<String> =
    LazyLock::new(|| vec!["-"].into_iter().cycle().take(H2.len()).join(""));

impl SymbolTable {
    pub fn to_string(&self, semantic_metadata: &SemanticMetadata) -> String {
        let mut buf = String::new();
        writeln!(buf, "\n{}\n{}", H1, EQ_H1.as_str()).unwrap();
        writeln!(buf, "{:<15}: {}", "Scope name", self.scope_name).unwrap();
        writeln!(buf, "{:<15}: {}", "Scope level", self.scope_level).unwrap();
        writeln!(
            buf,
            "{:<15}: {}",
            "Enclosing scope",
            self.enclosing_scope
                .as_ref()
                .map_or("None", |scope| &scope.scope_name)
        )
        .unwrap();
        writeln!(buf, "{}\n{}", H2, EQ_H2.as_str()).unwrap();
        self.type_symbols
            .iter()
            .map(|(n, t)| (n, semantic_metadata.types.get(*t)))
            .map(|(n, t)| (n, t.to_string(semantic_metadata)))
            .for_each(|(n, s)| writeln!(buf, "{:>7}: {}", n, s).unwrap());
        self.var_symbols
            .iter()
            .map(|(n, t)| (n, semantic_metadata.vars.get(*t)))
            .map(|(n, t)| (n, t.to_string(semantic_metadata)))
            .for_each(|(n, s)| writeln!(buf, "{:>7}: {}", n, s).unwrap());
        self.callable_symbols
            .iter()
            .map(|(n, t)| (n, semantic_metadata.callables.get(*t)))
            .map(|(n, t)| (n, t.to_string(semantic_metadata)))
            .for_each(|(n, s)| writeln!(buf, "{:>7}: {}", n, s).unwrap());
        buf
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
        debug!(target: "pascal::semantic", "Lookup type (scope name: {}): {}", self.scope_name, name);
        if self.type_symbols.contains_key(name) {
            debug!(target: "pascal::semantic", "type {} found in {} scope", name, self.scope_name);
            return Some(self.type_symbols[name]);
        };
        if current_scope_only {
            debug!(target: "pascal::semantic", "type {} was not found", name);
            return None;
        }
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_type(name, current_scope_only);
        };
        debug!(target: "pascal::semantic", "type {} was not found", name);
        None
    }
    pub fn lookup_var(
        &self,
        name: &str,
        current_scope_only: bool,
    ) -> Option<(VarSymbolRef, VarLocality)> {
        let var_type = match self.scope_level {
            1..2 => VarLocality::Global,
            _ => VarLocality::Local,
        };
        self.lookup_var_internal(name, current_scope_only, var_type)
    }

    fn lookup_var_internal(
        &self,
        name: &str,
        current_scope_only: bool,
        var_type: VarLocality,
    ) -> Option<(VarSymbolRef, VarLocality)> {
        let name = &name.to_lowercase();
        debug!(target: "pascal::semantic", "Lookup var (scope name: {}): {}", self.scope_name, name);
        if self.var_symbols.contains_key(name) {
            debug!(target: "pascal::semantic", "var {} found in {} scope; var type {:?}", name, self.scope_name, var_type);
            return Some((self.var_symbols[name], var_type));
        };
        if current_scope_only {
            debug!(target: "pascal::semantic", "var {} was not found", name);
            return None;
        }
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_var_internal(name, current_scope_only, VarLocality::Global);
        };
        debug!(target: "pascal::semantic", "var {} was not found", name);
        None
    }
    pub fn lookup_callable(
        &self,
        name: &str,
        current_scope_only: bool,
    ) -> Option<CallableSymbolRef> {
        let name = &name.to_lowercase();
        debug!(target: "pascal::semantic", "Lookup callable (scope name: {}): {}", self.scope_name, name);
        if self.callable_symbols.contains_key(name) {
            debug!(target: "pascal::semantic", "callable {} found in {} scope", name, self.scope_name);
            return Some(self.callable_symbols[name]);
        };
        if current_scope_only {
            debug!(target: "pascal::semantic", "callable {} was not found", name);
            return None;
        }
        if let Some(table) = &self.enclosing_scope {
            return table.lookup_callable(name, current_scope_only);
        };
        debug!(target: "pascal::semantic", "callable {} was not found", name);
        None
    }

    pub fn scope_name(&self) -> &str {
        &self.scope_name
    }
}

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    ops::ControlFlow,
};

use itertools::Itertools;

use crate::{
    error::Error,
    parser::{Decl, Expr, ExprRef, Stmt, StmtRef, Tree, Type, TypeRef},
    semantic_analyzer::SemanticMetadata,
    symbols::{CallableBody, LValue, ParamMode, RangeSymbol, TypeSymbol, TypeSymbolRef, VarSymbol},
    tokens::Token,
    utils::NodePool,
};

#[derive(Debug, Clone)]
pub enum Signal {
    Break,
    Continue,
    Exit(Option<Value>),
}

pub type Exec<T> = Result<ControlFlow<Signal, T>, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Real(f64),
    Boolean(bool),
    String(String),
    Char(char),
    Array(Vec<Option<Box<Value>>>),
}

impl Value {
    pub fn ordinal_rank(&self) -> Result<i64, Error> {
        match self {
            Value::Integer(i) => Ok(*i),
            Value::Char(c) => Ok(*c as i64),
            Value::Boolean(b) => Ok(*b as i64),
            _ => Err(Error::InterpreterError {
                msg: format!("ordinal rank is not supported for {:?}", self),
            }),
        }
    }
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Value::Integer(v) => format!("{v}"),
            Value::Real(v) => format!("{v}"),
            Value::Boolean(v) => format!("{v}"),
            Value::String(v) => v.to_owned(),
            Value::Char(c) => c.to_string(),
            Value::Array(vals) => format!(
                "[{}]",
                vals.iter()
                    .map(|v| v.as_ref().map_or("None".to_string(), |v| v.to_string()))
                    .join(", ")
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ref {
    value: Option<Value>,
}

impl Ref {
    pub fn new() -> Self {
        Self { value: None }
    }
    pub fn get(&self) -> Option<&Value> {
        self.value.as_ref()
    }
    pub fn get_mut(&mut self) -> Option<&mut Value> {
        self.value.as_mut()
    }
    pub fn set(&mut self, value: Value) {
        self.value = Some(value);
    }
}

impl Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.get() {
            Some(v) => write!(f, "{}", v.to_string()),
            None => write!(f, "Null"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivationRecord {
    name: String,
    nesting_level: usize,
    members: HashMap<String, Ref>,
}

impl ActivationRecord {
    pub fn new(name: &str, nesting_level: usize) -> Self {
        Self {
            name: name.to_string(),
            nesting_level,
            members: HashMap::new(),
        }
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Ref> {
        self.members.get_mut(name)
    }
    pub fn get_value(&self, name: &str) -> Option<&Value> {
        self.members.get(name).and_then(|f| f.get())
    }
    pub fn set(&mut self, name: &str) {
        self.members.insert(name.to_string(), Ref::new());
    }
    pub fn set_value(&mut self, name: &str, value: Value) {
        self.members
            .get_mut(name)
            .expect(&format!(
                "unkown variable {}, semantic analyzer should've handeled this",
                name
            ))
            .set(value);
    }
    pub fn contains(&self, name: &str) -> bool {
        self.members.contains_key(name)
    }
}

pub trait BuiltinCtx {
    type Value;

    fn read<'a>(&'a self, builtin_input: &'a LValue) -> Result<&'a Self::Value, Error>;
    fn write(&mut self, builtin_input: &LValue, value: Value) -> Result<(), Error>;
}

pub struct CallStack {
    records: Vec<ActivationRecord>,
}

impl CallStack {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }
    pub fn current_nesting(&self) -> usize {
        self.records.len()
    }
    pub fn pop(&mut self) -> Option<ActivationRecord> {
        self.records.pop()
    }
    pub fn push(&mut self, ar: ActivationRecord) {
        self.records.push(ar);
    }
    pub fn peek(&self) -> &ActivationRecord {
        self.records.last().unwrap()
    }
    pub fn peek_mut(&mut self) -> &mut ActivationRecord {
        self.records.last_mut().unwrap()
    }
    pub fn lookup_mut(&mut self, key: &str) -> Option<&mut Ref> {
        for ar in self.records.iter_mut().rev() {
            if ar.contains(key) {
                return ar.get_mut(key);
            }
        }
        None
    }
    pub fn lookup_value(&self, key: &str) -> Option<&Value> {
        for ar in self.records.iter().rev() {
            if ar.contains(key) {
                return ar.get_value(key);
            }
        }
        None
    }
}

impl BuiltinCtx for CallStack {
    type Value = Value;

    fn read<'a>(&'a self, builtin_input: &'a LValue) -> Result<&'a Self::Value, Error> {
        match builtin_input {
            LValue::Value(v) => Ok(v),
            LValue::Ref { name } => self.lookup_value(name).ok_or(Error::InterpreterError {
                msg: "name was not found".into(),
            }),
            LValue::ArrIndex { name, index } => self
                .lookup_value(name)
                .ok_or(Error::InterpreterError {
                    msg: "name was not found".into(),
                })
                .and_then(|v| match v {
                    Value::Array(a) => a[*index].as_deref().ok_or(Error::InterpreterError {
                        msg: "value is None".into(),
                    }),
                    _ => Err(Error::InterpreterError {
                        msg: "value should be array".into(),
                    }),
                }),
        }
    }

    fn write(&mut self, builtin_input: &LValue, value: Self::Value) -> Result<(), Error> {
        match builtin_input {
            LValue::Value(_) => Err(Error::InterpreterError {
                msg: "cannot write to a value".into(),
            }),
            LValue::Ref { name } => {
                self.lookup_mut(name)
                    .ok_or(Error::InterpreterError {
                        msg: "name not found".into(),
                    })?
                    .set(value);
                Ok(())
            }
            LValue::ArrIndex { name, index } => self
                .lookup_mut(name)
                .ok_or(Error::InterpreterError {
                    msg: "name not found".into(),
                })?
                .get_mut()
                .ok_or(Error::InterpreterError {
                    msg: "var is not defined".into(),
                })
                .map(|v| match v {
                    Value::Array(a) => {
                        a[*index] = Some(Box::new(value));
                        Ok(())
                    }
                    _ => Err(Error::InterpreterError {
                        msg: "var is not array".into(),
                    }),
                })?,
        }
    }
}

pub struct Interpreter {
    call_stack: CallStack,

    range_symbols: NodePool<TypeSymbolRef, RangeSymbol>,
    type_range_map: HashMap<TypeSymbolRef, TypeSymbolRef>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            call_stack: CallStack::new(),
            range_symbols: NodePool::new(),
            type_range_map: HashMap::new(),
        }
    }

    pub fn interperet(
        &mut self,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<(), Error> {
        self.call_stack.push(ActivationRecord::new("gloabl", 0));
        tree.program
            .block
            .declarations
            .iter()
            .map(|d| self.visit_declaration(d, tree, semantic_metadata))
            .collect::<Result<(), Error>>()?;
        let _ = self.visit_stmt(tree.program.block.statements, tree, semantic_metadata)?;
        Ok(())
    }

    fn visit_stmt(
        &mut self,
        stmt: StmtRef,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Exec<()> {
        match tree.stmt_pool.get(stmt) {
            Stmt::Compound(stmts) => {
                for s in stmts {
                    match self.visit_stmt(*s, tree, semantic_metadata)? {
                        ControlFlow::Continue(()) => {}
                        ControlFlow::Break(sig) => return Ok(ControlFlow::Break(sig)),
                    }
                }
                Ok(ControlFlow::Continue(()))
            }
            Stmt::Assign { left, right } => {
                let val = self.visit_expr(*right, tree, semantic_metadata)?;
                match tree.expr_pool.get(*left) {
                    Expr::Var { name } => self.call_stack.peek_mut().set_value(name, val),
                    Expr::Index {
                        base,
                        index_value,
                        other_indicies: _,
                    } => {
                        let index_value =
                            match self.visit_expr(*index_value, tree, semantic_metadata)? {
                                Value::Integer(i) => i as usize,
                                _ => panic!(),
                            };
                        match tree.expr_pool.get(*base) {
                            Expr::Var { name } => self.call_stack.write(
                                &LValue::ArrIndex {
                                    name: name,
                                    index: index_value,
                                },
                                val,
                            )?,
                            _ => panic!(),
                        };
                    }
                    _ => panic!("unreachable"),
                };
                Ok(ControlFlow::Continue(()))
            }
            Stmt::NoOp => Ok(ControlFlow::Continue(())),
            Stmt::While { cond, body } => {
                loop {
                    let c = match self.visit_expr(*cond, tree, semantic_metadata)? {
                        Value::Boolean(b) => b,
                        _ => panic!("unreachable"),
                    };
                    if !c {
                        break;
                    }
                    match self.visit_stmt(*body, tree, semantic_metadata)? {
                        ControlFlow::Continue(()) => {}
                        ControlFlow::Break(Signal::Continue) => continue,
                        ControlFlow::Break(Signal::Break) => break,
                        ControlFlow::Break(sig @ Signal::Exit(_)) => {
                            return Ok(ControlFlow::Break(sig));
                        }
                    }
                }
                Ok(ControlFlow::Continue(()))
            }
            Stmt::Continue => Ok(ControlFlow::Break(Signal::Continue)),
            Stmt::Break => Ok(ControlFlow::Break(Signal::Break)),
            Stmt::Exit(e) => {
                let v = if let Some(e) = e {
                    Some(self.visit_expr(*e, tree, semantic_metadata)?)
                } else {
                    None
                };
                Ok(ControlFlow::Break(Signal::Exit(v)))
            }
            Stmt::Call { call } => match tree.expr_pool.get(*call) {
                Expr::Call { name, args } => {
                    self.visit_callable(call, name, args, tree, semantic_metadata)?;
                    Ok(ControlFlow::Continue(()))
                }
                _ => panic!("unreachable"),
            },
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => todo!(),
            Stmt::For {
                var,
                init,
                end,
                body,
            } => {
                let init_val = self.visit_expr(*init, tree, semantic_metadata)?;
                let end_val = self.visit_expr(*end, tree, semantic_metadata)?;
                let type_symbol = semantic_metadata.get_expr_type(init).unwrap();
                self.call_stack
                    .write(&LValue::Ref { name: var }, init_val.clone())?;
                let mut i = init_val.ordinal_rank()?;
                while i != end_val.ordinal_rank()? {
                    let cr = self.visit_stmt(*body, tree, semantic_metadata)?;
                    match cr {
                        ControlFlow::Continue(()) => {}
                        ControlFlow::Break(Signal::Continue) => {}
                        ControlFlow::Break(Signal::Break) => break,
                        ControlFlow::Break(sig @ Signal::Exit(_)) => {
                            return Ok(ControlFlow::Break(sig));
                        }
                    }
                    i += 1;
                    self.call_stack
                        .write(&LValue::Ref { name: var }, type_symbol.oridnal_value(i)?)?;
                }
                Ok(ControlFlow::Continue(()))
            }
        }
    }

    fn visit_expr(
        &mut self,
        expr: ExprRef,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<Value, Error> {
        match tree.expr_pool.get(expr) {
            Expr::LiteralBool(b) => Ok(Value::Boolean(*b)),
            Expr::LiteralChar(c) => Ok(Value::Char(*c)),
            Expr::LiteralInteger(i) => Ok(Value::Integer(*i)),
            Expr::LiteralReal(r) => Ok(Value::Real(*r)),
            Expr::LiteralString(s) => Ok(Value::String(s.to_owned())),
            Expr::Var { name: _ } => {
                let var_symbol_ref = semantic_metadata
                    .var_symbols
                    .get(&expr)
                    .expect("should have var symbol ref");
                match semantic_metadata.vars.get(*var_symbol_ref) {
                    VarSymbol::Var {
                        name,
                        type_symbol: _,
                    } => self
                        .call_stack
                        .peek()
                        .get_value(name)
                        .map(|v| v.clone())
                        .ok_or(Error::InterpreterError {
                            msg: format!("undefined var {}", name),
                        }),
                    VarSymbol::Const { value } => Ok(value.clone().into()),
                }
            }
            Expr::UnaryOp { op, expr } => {
                match (op, self.visit_expr(*expr, tree, semantic_metadata)?) {
                    (Token::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
                    (Token::Minus, Value::Integer(v)) => Ok(Value::Integer(-v)),
                    (Token::Minus, Value::Real(v)) => Ok(Value::Real(-v)),
                    (Token::Plus, Value::Integer(v)) => Ok(Value::Integer(v)),
                    (Token::Plus, Value::Real(v)) => Ok(Value::Real(v)),
                    _ => panic!("unreachable"),
                }
            }
            Expr::BinOp { op, left, right } => {
                let v_l = self.visit_expr(*left, tree, semantic_metadata)?;
                let v_r = self.visit_expr(*right, tree, semantic_metadata)?;
                Ok(bin_op(op, v_l, v_r))
            }
            Expr::Index {
                base,
                index_value,
                other_indicies: _,
            } => {
                let var_name = match tree.expr_pool.get(*base) {
                    Expr::Var { name } => name,
                    _ => panic!("unreachable"),
                };
                let index_value = self.visit_expr(*index_value, tree, semantic_metadata)?;
                let arr_value = self
                    .call_stack
                    .lookup_value(var_name)
                    .expect("should exist");
                match (arr_value, index_value) {
                    (Value::Array(_), Value::Integer(i)) => Ok(self
                        .call_stack
                        .read(&LValue::ArrIndex {
                            name: var_name,
                            index: i as usize,
                        })?
                        .clone()),
                    _ => todo!(),
                }
            }
            Expr::Call { name, args } => {
                match self.visit_callable(&expr, name, args, tree, semantic_metadata)? {
                    Some(v) => Ok(v),
                    None => Err(Error::InterpreterError {
                        msg: "function returned none".to_string(),
                    }),
                }
            }
        }
    }

    fn visit_declaration(
        &mut self,
        decl: &Decl,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<(), Error> {
        match decl {
            Decl::VarDecl {
                var,
                type_node,
                default_value,
            } => {
                let var_expr = tree.expr_pool.get(*var);
                let var_name = match var_expr {
                    Expr::Var { name } => name,
                    _ => panic!("unreachable"),
                };
                self.call_stack.peek_mut().set(var_name);
                let type_symbol = semantic_metadata.types.get(
                    *semantic_metadata
                        .expr_type_map
                        .get(var)
                        .expect("should exist"),
                );
                if let TypeSymbol::Array {
                    index_type: _,
                    value_type: _,
                } = type_symbol
                {
                    self.visit_type(*type_node, tree, semantic_metadata)?;
                    let range_length = self
                        .range_symbols
                        .get(
                            *self
                                .type_range_map
                                .get(semantic_metadata.type_type_map.get(type_node).unwrap())
                                .unwrap(),
                        )
                        .len();
                    self.call_stack
                        .peek_mut()
                        .set_value(var_name, Value::Array(vec![None; range_length]));
                } else if let Some(val) = default_value {
                    let value = self.visit_expr(*val, tree, semantic_metadata)?;
                    self.call_stack.peek_mut().set_value(var_name, value);
                };
                Ok(())
            }
            Decl::Callable {
                name: _,
                block: _,
                params: _,
                return_type: _,
            } => Ok(()),
            Decl::ConstDecl { var: _, literal: _ } => Ok(()),
            Decl::TypeDecl {
                var: _,
                type_node: _,
            } => Ok(()),
        }
    }
    fn visit_callable(
        &mut self,
        node: &ExprRef,
        name: &str,
        args: &Vec<ExprRef>,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<Option<Value>, Error> {
        let symbol = semantic_metadata
            .callable_symbols
            .get(node)
            .expect("call should exist");
        match symbol.body {
            CallableBody::BlockAST(node) => {
                let mut ar = ActivationRecord::new(name, self.call_stack.current_nesting() + 1);
                for ((inp, mode), arg) in symbol.params.iter().zip(args) {
                    match mode {
                        ParamMode::Var => {
                            let var_symbol = semantic_metadata.vars.get(*inp);
                            let var_name = match var_symbol {
                                VarSymbol::Var {
                                    name,
                                    type_symbol: _,
                                } => name,
                                _ => panic!("unreachable"),
                            };
                            let value = self.visit_expr(*arg, tree, semantic_metadata)?;
                            ar.set(var_name);
                            ar.set_value(var_name, value);
                        }
                        ParamMode::Ref => {}
                    }
                }
                if let Some(_) = symbol.return_type {
                    ar.set("result");
                    ar.set(&symbol.name);
                }
                self.call_stack.push(ar);
                let cf = self.visit_stmt(node, tree, semantic_metadata)?;
                let result = match symbol.return_type {
                    Some(_) => {
                        if let ControlFlow::Break(Signal::Exit(Some(val))) = cf {
                            Some(val.clone())
                        } else if let Some(val) = self.call_stack.peek().get_value("result") {
                            Some(val.clone())
                        } else {
                            self.call_stack.peek().get_value(&symbol.name).cloned()
                        }
                    }
                    None => None,
                };
                self.call_stack.pop();
                Ok(result)
            }
            CallableBody::Func(f) => {
                let values: Vec<LValue> = symbol
                    .params
                    .iter()
                    .zip(args)
                    .map(|((_, m), a)| match m {
                        ParamMode::Var => self
                            .visit_expr(*a, tree, semantic_metadata)
                            .map(|e| LValue::Value(e)),
                        ParamMode::Ref => match tree.expr_pool.get(*a) {
                            Expr::Var { name } => Ok(LValue::Ref { name }),
                            _ => panic!("unreachable"),
                        },
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                let value_refs: Vec<&LValue> = values.iter().collect();
                f(&mut self.call_stack, &value_refs)
            }
        }
    }

    fn visit_type(
        &mut self,
        type_node: TypeRef,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<(), Error> {
        match tree.type_pool.get(type_node) {
            Type::Range { start_val, end_val } => {
                // let type_symbol = semantic_metadata
                //     .get_type_type(&type_node)
                //     .expect("should exist");
                let init_val = self.visit_expr(*start_val, tree, semantic_metadata)?;
                let end_val = self.visit_expr(*end_val, tree, semantic_metadata)?;
                self.type_range_map.insert(
                    *semantic_metadata.type_type_map.get(&type_node).unwrap(),
                    self.range_symbols
                        .alloc(RangeSymbol::new(&init_val, &end_val)?),
                );
                Ok(())
            }
            Type::Alias(_) => {
                let type_ref = semantic_metadata.type_type_map.get(&type_node).unwrap();
                let range_symbol_ref = self.type_range_map.get(type_ref);
                if let Some(range_ref) = range_symbol_ref {
                    self.type_range_map.insert(*type_ref, *range_ref);
                }
                Ok(())
            }
            Type::Array {
                index_type,
                element_type: _,
            } => {
                self.visit_type(*index_type, tree, semantic_metadata)?;
                let type_ref = semantic_metadata.type_type_map.get(index_type).unwrap();
                let range_symbol_ref = self.type_range_map.get(type_ref);
                if let Some(range_ref) = range_symbol_ref {
                    self.type_range_map.insert(
                        *semantic_metadata.type_type_map.get(&type_node).unwrap(),
                        *range_ref,
                    );
                } else {
                    panic!()
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn bin_op(op: &Token, v_l: Value, v_r: Value) -> Value {
    match op {
        Token::Plus => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Integer(v_l + v_r),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Real(v_l as f64 + v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Real(v_l + v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Real(v_l + v_r),
            (Value::String(v_l), Value::String(v_r)) => Value::String(v_l + &v_r),
            (Value::String(v_l), Value::Char(v_r)) => Value::String(v_l + &v_r.to_string()),
            _ => panic!("unreachable"),
        },
        Token::Minus => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Integer(v_l - v_r),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Real(v_l as f64 - v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Real(v_l - v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Real(v_l + v_r),
            _ => panic!("unreachable"),
        },
        Token::Mul => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Integer(v_l * v_r),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Real(v_l as f64 * v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Real(v_l * v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Real(v_l * v_r),
            _ => panic!("unreachable"),
        },
        Token::RealDiv => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Real(v_l as f64 / v_r as f64),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Real(v_l as f64 / v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Real(v_l / v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Real(v_l / v_r),
            _ => panic!("unreachable"),
        },
        Token::IntegerDiv => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Integer(v_l / v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => {
                Value::Integer((v_l / v_r as f64).floor() as i64)
            }
            _ => panic!("Unreachable"),
        },
        Token::Equal => Value::Boolean(v_l == v_r),
        Token::NotEqual => Value::Boolean(v_l != v_r),
        Token::And => match (v_l, v_r) {
            (Value::Boolean(v_l), Value::Boolean(v_r)) => Value::Boolean(v_l && v_r),
            _ => panic!("unreachable"),
        },
        Token::Or => match (v_l, v_r) {
            (Value::Boolean(v_l), Value::Boolean(v_r)) => Value::Boolean(v_l || v_r),
            _ => panic!("unreachable"),
        },
        Token::GreaterThen => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Boolean(v_l > v_r),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Boolean(v_l as f64 > v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Boolean(v_l > v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Boolean(v_l > v_r),
            _ => panic!("unreachable"),
        },
        Token::LessThen => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Boolean(v_l < v_r),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Boolean((v_l as f64) < v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Boolean(v_l < v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Boolean(v_l < v_r),
            _ => panic!("unreachable"),
        },
        Token::GreaterEqual => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Boolean(v_l >= v_r),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Boolean((v_l as f64) >= v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Boolean(v_l >= v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Boolean(v_l >= v_r),
            _ => panic!("unreachable"),
        },
        Token::LessEqual => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Value::Boolean(v_l <= v_r),
            (Value::Integer(v_l), Value::Real(v_r)) => Value::Boolean((v_l as f64) <= v_r),
            (Value::Real(v_l), Value::Integer(v_r)) => Value::Boolean(v_l <= v_r as f64),
            (Value::Real(v_l), Value::Real(v_r)) => Value::Boolean(v_l <= v_r),
            _ => panic!("unreachable"),
        },
        _ => panic!("unreachable"),
    }
}

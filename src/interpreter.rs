use std::{
    collections::HashMap,
    fmt::Debug,
    io::{BufRead, BufReader, BufWriter, Stdin, Stdout, Write, stdin, stdout},
    ops::ControlFlow,
};

use crate::{
    error::{Error, ErrorCode},
    parser::{Condition, Decl, Expr, ExprRef, NodeRef, Stmt, StmtRef, Tree, Type, TypeRef},
    semantic_analyzer::SemanticMetadata,
    symbols::{CallableType, LValue, ParamMode, RangeSymbol, TypeSymbol, TypeSymbolRef, VarSymbol},
    tokens::{Token, TokenType},
    utils::{NodePool, Pos},
};

#[derive(Debug, Clone)]
pub enum Signal {
    Exit(Option<Value>),
    Break,
    Continue,
}

pub type Exec<T> = Result<ControlFlow<Signal, T>, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Array(Vec<Option<Box<Value>>>),
    String(String),
    Integer(i64),
    Real(f64),
    Char(char),
    Boolean(bool),
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

#[derive(Debug, Clone)]
pub struct ActivationRecord {
    members: HashMap<String, Ref>,
    name: String,
    nesting_level: usize,
}

impl ToString for ActivationRecord {
    fn to_string(&self) -> String {
        format!("Activation Record {} - {}", self.name, self.nesting_level)
    }
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

    fn read<'a>(&'a self, builtin_input: &'a LValue) -> &'a Self::Value;
    fn write(&mut self, builtin_input: &LValue, value: Value);
    fn output(&mut self) -> &mut dyn Write;
    fn input(&mut self) -> &mut dyn BufRead;
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

impl<R: BufRead, W: Write> BuiltinCtx for Interpreter<R, W> {
    type Value = Value;

    fn read<'a>(&'a self, builtin_input: &'a LValue) -> &'a Self::Value {
        match builtin_input {
            LValue::Value(v) => v,
            LValue::Ref { name } => self.call_stack.lookup_value(name).unwrap(),
            LValue::ArrIndex { name, index } => match self.call_stack.lookup_value(name).unwrap() {
                Value::Array(a) => a[*index].as_deref().unwrap(),
                _ => unreachable!(),
            },
        }
    }

    fn write(&mut self, builtin_input: &LValue, value: Self::Value) {
        match builtin_input {
            LValue::Value(_) => panic!(),
            LValue::Ref { name } => {
                self.call_stack.lookup_mut(name).unwrap().set(value);
            }
            LValue::ArrIndex { name, index } => {
                match self.call_stack.lookup_mut(name).unwrap().get_mut().unwrap() {
                    Value::Array(a) => {
                        a[*index] = Some(Box::new(value));
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    fn output(&mut self) -> &mut dyn Write {
        &mut self.out_buf
    }

    fn input(&mut self) -> &mut dyn BufRead {
        &mut self.in_buf
    }
}

pub struct Interpreter<R: BufRead, W: Write> {
    type_range_map: HashMap<TypeSymbolRef, TypeSymbolRef>,
    call_stack: CallStack,
    range_symbols: NodePool<TypeSymbolRef, RangeSymbol>,
    in_buf: R,
    out_buf: W,
}

#[cfg(test)]
impl<R: BufRead, W: Write> Interpreter<R, W> {
    fn new_test(in_buf: R, out_buf: W) -> Self {
        Self {
            type_range_map: HashMap::new(),
            call_stack: CallStack::new(),
            range_symbols: NodePool::new(),
            in_buf,
            out_buf,
        }
    }
}

impl Interpreter<BufReader<Stdin>, BufWriter<Stdout>> {
    pub fn new() -> Self {
        Self {
            call_stack: CallStack::new(),
            range_symbols: NodePool::new(),
            type_range_map: HashMap::new(),
            in_buf: BufReader::new(stdin()),
            out_buf: BufWriter::new(stdout()),
        }
    }
}

impl<R: BufRead, W: Write> Interpreter<R, W> {
    #[cfg(test)]
    fn interpret_test(
        mut self,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<W, Error> {
        self.interperet(tree, semantic_metadata)?;
        Ok(self.out_buf)
    }

    pub fn interperet(
        &mut self,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<(), Error> {
        self.call_stack.push(ActivationRecord::new("gloabl", 0));
        let _ = self.visit_stmt(tree.program, tree, semantic_metadata)?;
        Ok(())
    }

    fn visit_stmt(
        &mut self,
        stmt: StmtRef,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Exec<()> {
        match tree.stmt_pool.get(stmt) {
            Stmt::Program { name: _, block } => self.visit_stmt(*block, tree, semantic_metadata),
            Stmt::Block {
                declarations,
                statements,
            } => {
                declarations
                    .iter()
                    .map(|d| self.visit_declaration(d, tree, semantic_metadata))
                    .collect::<Result<(), Error>>()?;
                self.visit_stmt(*statements, tree, semantic_metadata)
            }
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
                    Expr::Var { name } => self
                        .call_stack
                        .peek_mut()
                        .set_value(name.lexem(tree.source_code), val),
                    Expr::Index {
                        base,
                        index_value: index_value_ref,
                        other_indicies: _,
                    } => {
                        let index_value =
                            self.visit_expr(*index_value_ref, tree, semantic_metadata)?;
                        let arr_type = semantic_metadata
                            .types
                            .get(*semantic_metadata.expr_type_map.get(base).unwrap());
                        let range_symbol = match arr_type {
                            TypeSymbol::Array {
                                index_type,
                                value_type: _,
                            } => self
                                .range_symbols
                                .get(*self.type_range_map.get(index_type).unwrap()),
                            _ => panic!(),
                        };
                        let index_value = range_symbol.get_index(&index_value, semantic_metadata);
                        match tree.expr_pool.get(*base) {
                            Expr::Var { name } => self.write(
                                &LValue::ArrIndex {
                                    name: name.lexem(tree.source_code),
                                    index: index_value,
                                },
                                val,
                            ),
                            _ => panic!(),
                        };
                    }
                    _ => unreachable!(),
                };
                Ok(ControlFlow::Continue(()))
            }
            Stmt::NoOp => Ok(ControlFlow::Continue(())),
            Stmt::While { cond, body } => {
                loop {
                    let c = match self.visit_expr(*cond, tree, semantic_metadata)? {
                        Value::Boolean(b) => b,
                        _ => unreachable!(),
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
                _ => unreachable!(),
            },
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => {
                let (cond, cr) = self.visit_condition(cond, tree, semantic_metadata)?;
                if cond {
                    return Ok(cr);
                }
                for cond in elifs {
                    let (cond, cr) = self.visit_condition(cond, tree, semantic_metadata)?;
                    if cond {
                        return Ok(cr);
                    }
                }
                if let Some(else_stmt) = else_statement {
                    return self.visit_stmt(*else_stmt, tree, semantic_metadata);
                }
                Ok(ControlFlow::Continue(()))
            }
            Stmt::For {
                var,
                init,
                end,
                body,
            } => {
                let init_val = self.visit_expr(*init, tree, semantic_metadata)?;
                let end_val = self.visit_expr(*end, tree, semantic_metadata)?;
                let type_symbol = semantic_metadata.get_expr_type(init).unwrap();
                self.write(
                    &LValue::Ref {
                        name: var.lexem(tree.source_code),
                    },
                    init_val.clone(),
                );
                let mut i = type_symbol.ordinal_rank(&init_val, semantic_metadata);
                while i != type_symbol.ordinal_rank(&end_val, semantic_metadata) {
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
                    let result = type_symbol.oridnal_value(i);
                    self.write(
                        &LValue::Ref {
                            name: var.lexem(tree.source_code),
                        },
                        result,
                    );
                }
                Ok(ControlFlow::Continue(()))
            }
        }
    }

    fn visit_condition(
        &mut self,
        condition: &Condition,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<(bool, ControlFlow<Signal, ()>), Error> {
        let cond = match self.visit_expr(condition.cond, tree, semantic_metadata)? {
            Value::Boolean(b) => b,
            _ => panic!(),
        };
        if cond {
            return self
                .visit_stmt(condition.expr, tree, semantic_metadata)
                .map(|cr| (cond, cr));
        }
        Ok((cond, ControlFlow::Continue(())))
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
            Expr::LiteralString(s) => Ok(Value::String(s.lexem(tree.source_code).into())),
            Expr::Var { name: _ } => {
                let var_symbol_ref = semantic_metadata
                    .var_symbols
                    .get(&expr)
                    .expect("should have var symbol ref");
                match semantic_metadata.vars.get(*var_symbol_ref) {
                    VarSymbol::Var {
                        name,
                        type_symbol: _,
                    } => Ok(self
                        .call_stack
                        .peek()
                        .get_value(name)
                        .map(|v| v.clone())
                        .unwrap()),
                    VarSymbol::Const { value, .. } => Ok(value.clone().into()),
                }
            }
            Expr::UnaryOp { op, expr } => {
                match (op, self.visit_expr(*expr, tree, semantic_metadata)?) {
                    (TokenType::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
                    (TokenType::Minus, Value::Integer(v)) => Ok(Value::Integer(-v)),
                    (TokenType::Minus, Value::Real(v)) => Ok(Value::Real(-v)),
                    (TokenType::Plus, Value::Integer(v)) => Ok(Value::Integer(v)),
                    (TokenType::Plus, Value::Real(v)) => Ok(Value::Real(v)),
                    _ => unreachable!(),
                }
            }
            Expr::BinOp { op, left, right } => {
                let v_l = self.visit_expr(*left, tree, semantic_metadata)?;
                let v_r = self.visit_expr(*right, tree, semantic_metadata)?;
                let pos = tree.node_pos(NodeRef::ExprRef(*right));
                bin_op(pos, op, v_l, v_r)
            }
            Expr::Index {
                base,
                index_value: index_value_ref,
                other_indicies: _,
            } => {
                let index_value = self.visit_expr(*index_value_ref, tree, semantic_metadata)?;
                let index_type_ref = semantic_metadata
                    .expr_type_map
                    .get(index_value_ref)
                    .unwrap();
                let index_type_symbol = semantic_metadata.types.get(*index_type_ref);
                let var_name = match tree.expr_pool.get(*base) {
                    Expr::Var { name } => name,
                    _ => unreachable!(),
                };
                let index_value = index_type_symbol.ordinal_rank(&index_value, semantic_metadata);
                let arr_value = self
                    .call_stack
                    .lookup_value(var_name.lexem(tree.source_code))
                    .expect("should exist");
                match arr_value {
                    Value::Array(_) => Ok(self
                        .read(&LValue::ArrIndex {
                            name: var_name.lexem(tree.source_code),
                            index: index_value as usize,
                        })
                        .clone()),
                    _ => todo!(),
                }
            }
            Expr::Call { name, args } => self
                .visit_callable(&expr, name, args, tree, semantic_metadata)
                .map(|v| v.unwrap()),
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
                    _ => unreachable!(),
                };
                self.call_stack
                    .peek_mut()
                    .set(var_name.lexem(tree.source_code));
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
                    self.call_stack.peek_mut().set_value(
                        var_name.lexem(tree.source_code),
                        Value::Array(vec![None; range_length]),
                    );
                } else if let Some(val) = default_value {
                    let value = self.visit_expr(*val, tree, semantic_metadata)?;
                    self.call_stack
                        .peek_mut()
                        .set_value(var_name.lexem(tree.source_code), value);
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
        name: &Token,
        args: &Vec<ExprRef>,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<Option<Value>, Error> {
        let symbol = semantic_metadata.callables.get(
            *semantic_metadata
                .callable_symbols
                .get(node)
                .expect("call should exist"),
        );
        let params = symbol.params.iter().cycle().take(args.len()).zip(args);
        match symbol.body {
            CallableType::Custom {
                statement: node, ..
            } => {
                let mut ar = ActivationRecord::new(
                    name.lexem(tree.source_code),
                    self.call_stack.current_nesting() + 1,
                );
                for ((inp, mode), arg) in params {
                    match mode {
                        ParamMode::Var => {
                            let var_symbol = semantic_metadata.vars.get(*inp);
                            let var_name = match var_symbol {
                                VarSymbol::Var {
                                    name,
                                    type_symbol: _,
                                } => name,
                                _ => unreachable!(),
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
            CallableType::Builtin { func: f } => {
                let values: Vec<(LValue, &TypeSymbol)> = params
                    .map(|((_, m), a)| match m {
                        ParamMode::Var => self.visit_expr(*a, tree, semantic_metadata).map(|e| {
                            (
                                LValue::Value(e),
                                semantic_metadata
                                    .types
                                    .get(*semantic_metadata.expr_type_map.get(a).unwrap()),
                            )
                        }),
                        ParamMode::Ref => match tree.expr_pool.get(*a) {
                            Expr::Var { name } => Ok((
                                LValue::Ref {
                                    name: name.lexem(tree.source_code),
                                },
                                semantic_metadata
                                    .types
                                    .get(*semantic_metadata.expr_type_map.get(a).unwrap()),
                            )),
                            _ => unreachable!(),
                        },
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                // let value_refs: Vec<(&LValue, &TypeSymbol)> = values.iter().collect();
                f(self, semantic_metadata, &values)
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
                let type_symbol_ref = semantic_metadata.type_type_map.get(&type_node).unwrap();
                let type_symbol = semantic_metadata.types.get(*type_symbol_ref);
                self.type_range_map.insert(
                    *type_symbol_ref,
                    self.range_symbols.alloc(RangeSymbol::new(
                        &init_val,
                        &end_val,
                        type_symbol,
                        semantic_metadata,
                    )),
                );
                Ok(())
            }
            Type::Alias(_) => {
                let type_ref = semantic_metadata
                    .type_type_map
                    .get(&type_node)
                    .unwrap_or_else(|| {
                        panic!(
                            "type {:?} should have type ref",
                            tree.type_pool.get(type_node)
                        )
                    });
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

fn bin_op(pos: Pos, op: &TokenType, v_l: Value, v_r: Value) -> Result<Value, Error> {
    match op {
        TokenType::Plus => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Integer(v_l + v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l as f64 + v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Real(v_l + v_r as f64)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l + v_r)),
            (Value::String(v_l), Value::String(v_r)) => Ok(Value::String(v_l + &v_r)),
            (Value::String(v_l), Value::Char(v_r)) => Ok(Value::String(v_l + &v_r.to_string())),
            _ => unreachable!(),
        },
        TokenType::Minus => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Integer(v_l - v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l as f64 - v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Real(v_l - v_r as f64)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l + v_r)),
            _ => unreachable!(),
        },
        TokenType::Mul => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Integer(v_l * v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l as f64 * v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Real(v_l * v_r as f64)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l * v_r)),
            _ => unreachable!(),
        },
        TokenType::RealDiv => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => {
                if v_r == 0 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Real(v_l as f64 / v_r as f64))
            }
            (Value::Integer(v_l), Value::Real(v_r)) => {
                if v_r.abs() < 0.0000000001 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Real(v_l as f64 / v_r))
            }
            (Value::Real(v_l), Value::Integer(v_r)) => {
                if v_r == 0 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Real(v_l / v_r as f64))
            }
            (Value::Real(v_l), Value::Real(v_r)) => {
                if v_r.abs() < 0.0000000001 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Real(v_l / v_r))
            }
            _ => unreachable!(),
        },
        TokenType::IntegerDiv => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Integer(v_l / v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => {
                Ok(Value::Integer((v_l / v_r as f64).floor() as i64))
            }
            _ => unreachable!(),
        },
        TokenType::Equal => Ok(Value::Boolean(v_l == v_r)),
        TokenType::NotEqual => Ok(Value::Boolean(v_l != v_r)),
        TokenType::And => match (v_l, v_r) {
            (Value::Boolean(v_l), Value::Boolean(v_r)) => Ok(Value::Boolean(v_l && v_r)),
            _ => unreachable!(),
        },
        TokenType::Or => match (v_l, v_r) {
            (Value::Boolean(v_l), Value::Boolean(v_r)) => Ok(Value::Boolean(v_l || v_r)),
            _ => unreachable!(),
        },
        TokenType::GreaterThen => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l > v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l as f64 > v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l > v_r as f64)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l > v_r)),
            _ => unreachable!(),
        },
        TokenType::LessThen => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l < v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean((v_l as f64) < v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l < v_r as f64)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l < v_r)),
            _ => unreachable!(),
        },
        TokenType::GreaterEqual => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l >= v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean((v_l as f64) >= v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l >= v_r as f64)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l >= v_r)),
            _ => unreachable!(),
        },
        TokenType::LessEqual => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l <= v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean((v_l as f64) <= v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l <= v_r as f64)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l <= v_r)),
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{lexer::Lexer, parser::Parser, semantic_analyzer::SemanticAnalyzer};

    use super::*;

    macro_rules! test_succ {
        ($(
            $name:ident
            ($($first_input:literal$(,$input:literal)*$(,)?)?)
            $(->[$first_output:literal$(,$output:literal)*$(,)?])?,
        )+) => {
            $(
                #[test]
                fn $name() {
                    let source_path = "test_cases\\interpreter\\".to_string() + stringify!($name) + ".pas";
                    let source_code = std::fs::read_to_string(&source_path).unwrap_or_else(|e| panic!("no file {source_path}: {e}"));
                    let lexer = Lexer::new(&source_code);
                    let tree = Parser::new(lexer).unwrap().parse().unwrap();
                    let semantic_metadata = SemanticAnalyzer::new().analyze(&tree).unwrap();
                    let mut _inp: Vec<&str> = Vec::new();
                    $(
                        _inp.push($first_input);
                        $(
                            _inp.push($input);
                        )*
                    )?
                    let inp = _inp.join("\n") + "\n";
                    let inp = inp.as_bytes();
                    let inp = Cursor::new(inp);
                    let out = BufWriter::new(Vec::new());
                    let out = Interpreter::new_test(inp, out)
                        .interpret_test(&tree, &semantic_metadata)
                        .unwrap()
                        .into_inner()
                        .unwrap();
                    let out_text = String::from_utf8(out).unwrap();
                    let mut _expected_out: Vec<&str> = Vec::new();
                    $(
                        _expected_out.push($first_output);
                        $(
                            _expected_out.push($output);
                        )*
                    )?
                    let expected_out = _expected_out.join("\n") + "\n";
                    assert_eq!(out_text, expected_out);
                }
            )+
        };
    }

    test_succ! {
        test_print() -> ["Hello, World!"],
        test_inp("Hello", "World") -> ["Hello", "World"],
    }
}

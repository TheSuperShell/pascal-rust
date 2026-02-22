use std::fmt::Write as _;
use std::{
    collections::HashMap,
    fmt::Debug,
    io::{BufRead, BufReader, BufWriter, Stdin, Stdout, Write, stdin, stdout},
    ops::ControlFlow,
};

use itertools::Itertools;
use tracing::debug;

use crate::{
    error::{Error, ErrorCode},
    parser::{Condition, Decl, Expr, ExprRef, NodeRef, Stmt, StmtRef, Tree, Type, TypeRef},
    semantic_analyzer::SemanticMetadata,
    symbols::{
        CallableType, LValue, RangeSymbol, TypeSymbol, TypeSymbolRef, VarPassMode, VarSymbol,
    },
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
    Integer(i32),
    Real(f32),
    Char(char),
    Boolean(bool),
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Self::Boolean(b) => b.to_string(),
            Self::Char(c) => c.to_string(),
            Self::Integer(i) => i.to_string(),
            Self::Real(r) => r.to_string(),
            Self::String(s) => s.into(),
            Self::Array(vals) => format!(
                "[{}]",
                vals.iter()
                    .map(|v| v.as_deref().map_or("None".to_string(), |v| v.to_string()))
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

impl ToString for Ref {
    fn to_string(&self) -> String {
        match &self.value {
            None => "None".into(),
            Some(v) => v.to_string(),
        }
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
        let mut buf = String::new();
        writeln!(
            buf,
            "Activation Record {} - {}",
            self.name, self.nesting_level
        )
        .unwrap();
        self.members
            .iter()
            .for_each(|(n, r)| writeln!(buf, "    {:<20}: {}", n, r.to_string()).unwrap());
        buf
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

impl ToString for CallStack {
    fn to_string(&self) -> String {
        let mut buf = String::new();
        writeln!(buf, "CALL STACK").unwrap();
        self.records
            .iter()
            .for_each(|r| writeln!(buf, "{}", r.to_string()).unwrap());
        buf
    }
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

        debug!(target: "pascal::interp", "ENTER Program");
        debug!(target: "pascal::interp", "{}", self.call_stack.to_string());

        let _ = self.visit_stmt(tree.program, tree, semantic_metadata)?;

        debug!(target: "pascal::interp", "LEAVE Program");
        debug!(target: "pascal::interp", "{}", self.call_stack.to_string());
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
                let left_type_ref = semantic_metadata.expr_type_map.get(left).unwrap();
                if let Some(&sym) = self.type_range_map.get(left_type_ref) {
                    let range_symbol = self.range_symbols.get(sym);
                    if !range_symbol.within_bounds(&val, semantic_metadata) {
                        return Err(Error::RuntimeError {
                            msg: format!(
                                "value {:?} is outside of range {:?} bounds",
                                val, range_symbol
                            ),
                            pos: tree.node_pos(NodeRef::ExprRef(*right)),
                            error_code: ErrorCode::RangeOutOfBounds,
                        });
                    }
                }
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
                            _ => unreachable!(),
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
                let var_symbol = semantic_metadata
                    .vars
                    .get(*semantic_metadata.var_symbols.get(var).unwrap());
                let name = match var_symbol {
                    VarSymbol::Var { name, .. } => name,
                    _ => unreachable!(),
                };
                let init_val = self.visit_expr(*init, tree, semantic_metadata)?;
                let end_val = self.visit_expr(*end, tree, semantic_metadata)?;
                let type_symbol = semantic_metadata.get_expr_type(init).unwrap();
                self.write(&LValue::Ref { name }, init_val.clone());
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
                    self.write(&LValue::Ref { name }, result);
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
                        ..
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
                self.visit_type(*type_node, tree, semantic_metadata)?;
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
                if let TypeSymbol::Array { .. } = type_symbol {
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
            Decl::Callable { params, .. } => params
                .iter()
                .filter(|p| !p.out)
                .map(|p| p.type_node)
                .map(|t| self.visit_type(t, tree, semantic_metadata))
                .collect::<Result<(), Error>>(),
            Decl::ConstDecl { .. } => Ok(()),
            Decl::TypeDecl { type_node, .. } => {
                self.visit_type(*type_node, tree, semantic_metadata)
            }
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
                for (inp, arg) in params {
                    let mode = semantic_metadata.vars.get(*inp).pass_mode().unwrap();
                    match mode {
                        VarPassMode::Val => {
                            let var_symbol = semantic_metadata.vars.get(*inp);
                            let value = self.visit_expr(*arg, tree, semantic_metadata)?;
                            let var_name = match var_symbol {
                                VarSymbol::Var {
                                    name, type_symbol, ..
                                } => {
                                    if let Some(r) = self.type_range_map.get(type_symbol) {
                                        let range_symbol = self.range_symbols.get(*r);
                                        if !range_symbol.within_bounds(&value, semantic_metadata) {
                                            return Err(Error::RuntimeError {
                                                msg: format!(
                                                    "value {:?} is outside of bounds of range {:?}",
                                                    value, range_symbol
                                                ),
                                                pos: tree.node_pos(NodeRef::ExprRef(*arg)),
                                                error_code: ErrorCode::RangeOutOfBounds,
                                            });
                                        }
                                    }
                                    name
                                }
                                _ => unreachable!(),
                            };
                            ar.set(var_name);
                            ar.set_value(var_name, value);
                        }
                        VarPassMode::Ref => {}
                    }
                }
                if let Some(_) = symbol.return_type {
                    ar.set("result");
                    ar.set(&symbol.name);
                }
                self.call_stack.push(ar);

                debug!(target: "pascal::interp", "ENTER CALLABLE: {}", symbol.name);
                debug!(target: "pascal::interp", "{}", self.call_stack.to_string());

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
                debug!(target: "pascal::interp", "LEAVE CALLABLE: {}", symbol.name);
                debug!(target: "pascal::interp", "{}", self.call_stack.to_string());
                self.call_stack.pop();
                Ok(result)
            }
            CallableType::Builtin { func: f } => {
                let values: Vec<(LValue, &TypeSymbol)> = params
                    .map(|(v, a)| {
                        let m = semantic_metadata.vars.get(*v).pass_mode().unwrap();
                        match m {
                        VarPassMode::Val => self.visit_expr(*a, tree, semantic_metadata).and_then(|e| {
                            match semantic_metadata.vars.get(*v) {
                                VarSymbol::Var { type_symbol, .. } => {
                                    if let Some(r) = self.type_range_map.get(type_symbol) {
                                        let range_symbol = self.range_symbols.get(*r);
                                        if !range_symbol.within_bounds(&e, semantic_metadata) {
                                            return Err(Error::RuntimeError {
                                                msg: format!(
                                                    "value {:?} is outside of bounds of range {:?}",
                                                    e, range_symbol
                                                ),
                                                pos: tree.node_pos(NodeRef::ExprRef(*a)),
                                                error_code: ErrorCode::RangeOutOfBounds,
                                            });
                                        }
                                    }
                                }
                                _ => unreachable!(),
                            };
                            Ok((
                                LValue::Value(e),
                                semantic_metadata
                                    .types
                                    .get(*semantic_metadata.expr_type_map.get(a).unwrap()),
                            ))
                        }),
                        VarPassMode::Ref => match tree.expr_pool.get(*a) {
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
                    }
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                // let value_refs: Vec<(&LValue, &TypeSymbol)> = values.iter().collect();
                let res = f(self, semantic_metadata, &values);
                if let Err(Error::BuiltinFunctionError { function_name, msg }) = res {
                    return Err(Error::RuntimeError {
                        msg: format!("{}: {}", function_name, msg),
                        pos: tree.node_pos(NodeRef::ExprRef(*node)),
                        error_code: ErrorCode::BuiltinFunctionError,
                    });
                }
                res
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
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l as f32 + v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Real(v_l + v_r as f32)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l + v_r)),
            (Value::String(v_l), Value::String(v_r)) => Ok(Value::String(v_l + &v_r)),
            (Value::String(v_l), Value::Char(v_r)) => Ok(Value::String(v_l + &v_r.to_string())),
            _ => unreachable!(),
        },
        TokenType::Minus => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Integer(v_l - v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l as f32 - v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Real(v_l - v_r as f32)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l + v_r)),
            _ => unreachable!(),
        },
        TokenType::Mul => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Integer(v_l * v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Real(v_l as f32 * v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Real(v_l * v_r as f32)),
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
                Ok(Value::Real(v_l as f32 / v_r as f32))
            }
            (Value::Integer(v_l), Value::Real(v_r)) => {
                if v_r.abs() < 0.0000000001 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Real(v_l as f32 / v_r))
            }
            (Value::Real(v_l), Value::Integer(v_r)) => {
                if v_r == 0 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Real(v_l / v_r as f32))
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
            (Value::Integer(v_l), Value::Integer(v_r)) => {
                if v_r == 0 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Integer(v_l / v_r))
            }
            (Value::Real(v_l), Value::Integer(v_r)) => {
                if v_r == 0 {
                    return Err(Error::RuntimeError {
                        msg: format!("division by zero"),
                        pos,
                        error_code: ErrorCode::DivisionByZero,
                    });
                }
                Ok(Value::Integer((v_l / v_r as f32).floor() as i32))
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
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l as f32 > v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l > v_r as f32)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l > v_r)),
            _ => unreachable!(),
        },
        TokenType::LessThen => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l < v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean((v_l as f32) < v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l < v_r as f32)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l < v_r)),
            _ => unreachable!(),
        },
        TokenType::GreaterEqual => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l >= v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean((v_l as f32) >= v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l >= v_r as f32)),
            (Value::Real(v_l), Value::Real(v_r)) => Ok(Value::Boolean(v_l >= v_r)),
            _ => unreachable!(),
        },
        TokenType::LessEqual => match (v_l, v_r) {
            (Value::Integer(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l <= v_r)),
            (Value::Integer(v_l), Value::Real(v_r)) => Ok(Value::Boolean((v_l as f32) <= v_r)),
            (Value::Real(v_l), Value::Integer(v_r)) => Ok(Value::Boolean(v_l <= v_r as f32)),
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
                    let mut expected_out = _expected_out.join("\n");
                    if !expected_out.is_empty() {
                        expected_out.push_str("\n");
                    }
                    assert_eq!(out_text, expected_out);

                }
            )+
        };

    }

    macro_rules! test_fail {
        ($(
            $name:ident$(<$file_name:ident>)?
            ($($first_input:literal$(,$input:literal)*$(,)?)?)
            -> $err:path,
        )+) => {
            $(
                #[test]
                fn $name() {
                    let source_path = test_fail!(@impl $name, $($file_name)?);
                    let source_code = std::fs::read_to_string(&source_path).unwrap_or_else(|r| panic!("file {source_path} does not exist: {r}"));
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
                    let result = Interpreter::new_test(inp, out).interperet(&tree, &semantic_metadata);
                    assert!(result.is_err(), "expected interpreter to error, instead no error was produced");
                    let err = result.unwrap_err();
                    assert!(matches!(err, Error::RuntimeError{error_code: $err, ..}), "expected runtime error with error code {}, got error {}", stringify!($err), err);
                }
            )+
        };
        (@impl $name:ident,) => {
            "test_cases\\interpreter\\".to_string() + &stringify!($name) + ".pas"
        };

        (@impl $name:ident, $file_name:ident) => {
            "test_cases\\interpreter\\".to_string() + &stringify!($file_name) + ".pas"
        };
    }

    test_fail! {
        test_div_zero_1<test_div_zero_fail>("int/int") -> ErrorCode::DivisionByZero,
        test_div_zero_2<test_div_zero_fail>("real/int") -> ErrorCode::DivisionByZero,
        test_div_zero_3<test_div_zero_fail>("int div int") -> ErrorCode::DivisionByZero,
        test_div_zero_4<test_div_zero_fail>("real div int") -> ErrorCode::DivisionByZero,
        test_div_zero_5<test_div_zero_fail>("int div real") -> ErrorCode::DivisionByZero,
        test_div_zero_6<test_div_zero_fail>("real div real") -> ErrorCode::DivisionByZero,
        test_range_bound_1<test_range_bound_fail>("var") -> ErrorCode::RangeOutOfBounds,
        test_range_bound_2<test_range_bound_fail>("func") -> ErrorCode::RangeOutOfBounds,
    }

    test_succ! {
        test_print() -> ["Hello, World!"],
        test_inp("Hello", "World") -> ["Hello", "World"],
        test_range(),
    }
}

use std::{
    collections::{HashMap, VecDeque},
    fmt::{Debug, Display},
};

use itertools::Itertools;

use crate::{
    error::Error,
    parser::{Decl, Expr, ExprRef, Stmt, StmtRef, Tree},
    semantic_analyzer::SemanticMetadata,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Real(f64),
    Boolean(bool),
    String(String),
    Char(char),
    Array(Vec<Box<Value>>),
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Value::Integer(v) => format!("{v}"),
            Value::Real(v) => format!("{v}"),
            Value::Boolean(v) => format!("{v}"),
            Value::String(v) => v.to_owned(),
            Value::Char(c) => c.to_string(),
            Value::Array(vals) => format!("[{}]", vals.iter().map(|v| v.to_string()).join(", ")),
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
    pub fn get(&self, name: &str) -> Option<&Ref> {
        self.members.get(name)
    }
    pub fn get_value(&self, name: &str) -> Option<&Value> {
        self.members.get(name).and_then(|f| f.get())
    }
    pub fn set(&mut self, name: &str) {
        self.members.insert(name.to_string(), Ref::new());
    }
    pub fn set_value(&mut self, name: &str, value: Value) {
        if !self.members.contains_key(name) {
            self.set(name);
        };
        self.members
            .get_mut(name)
            .expect("should never fail")
            .set(value);
    }
    pub fn contains(&self, name: &str) -> bool {
        self.members.contains_key(name)
    }
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
    pub fn lookup(&self, key: &str) -> Option<&Ref> {
        for ar in self.records.iter().rev() {
            if ar.contains(key) {
                return ar.get(key);
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

pub struct Interpreter {
    call_stack: CallStack,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            call_stack: CallStack::new(),
        }
    }

    pub fn interperet(
        &mut self,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<(), Error> {
        tree.program
            .block
            .declarations
            .iter()
            .map(|d| self.visit_declaration(d, tree, semantic_metadata))
            .collect::<Result<(), Error>>()?;
        self.visit_stmt(tree.program.block.statements, tree, semantic_metadata)?;
        Ok(())
    }

    fn visit_stmt(
        &mut self,
        stmt: StmtRef,
        tree: &Tree,
        semantic_metadata: &SemanticMetadata,
    ) -> Result<(), Error> {
        match tree.stmt_pool.get(stmt) {
            Stmt::Compound(stmts) => Ok(stmts
                .iter()
                .map(|stmt| self.visit_stmt(*stmt, tree, semantic_metadata))
                .collect::<Result<(), Error>>()?),
            _ => Err(Error::InterpreterError {
                msg: "not implemented".to_string(),
            }),
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
            Expr::Var { name } => self
                .call_stack
                .peek()
                .get_value(name)
                .map(|v| v.clone())
                .ok_or(Error::InterpreterError {
                    msg: "undefined var".to_string(),
                }),
            Expr::UnaryOp { op, expr } => {
                todo!()
            }
            _ => Err(Error::InterpreterError {
                msg: "not implemented".to_string(),
            }),
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
                type_node: _,
                default_value,
            } => {
                let var_expr = tree.expr_pool.get(*var);
                let var_name = match var_expr {
                    Expr::Var { name } => name,
                    _ => panic!("unreachable"),
                };
                self.call_stack.peek_mut().set(var_name);
                if let Some(val) = default_value {
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
}

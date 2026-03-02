use std::collections::HashMap;

use crate::{
    error::{Error, ErrorCode, Errors},
    parser::{Condition, Decl, Expr, ExprRef, NodeRef, Stmt, StmtRef, Tree, Type, TypeRef},
    symbols::{
        CallableSymbol, CallableSymbolRef, CallableType, ConstValue, ParamInputMode, SymbolTable,
        TypeSymbol, TypeSymbolRef, VarLocality, VarPassMode, VarSymbol, VarSymbolRef,
    },
    tokens::{Token, TokenType},
    utils::{NodePool, Size},
};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct SemanticMetadata {
    pub expr_type_map: HashMap<ExprRef, TypeSymbolRef>,
    pub type_type_map: HashMap<TypeRef, TypeSymbolRef>,
    pub callable_symbols: HashMap<ExprRef, CallableSymbolRef>,
    pub var_symbols: HashMap<ExprRef, VarSymbolRef>,
    pub var_types: HashMap<ExprRef, VarLocality>,
    pub type_sizes: HashMap<TypeSymbolRef, Size>,

    pub types: NodePool<TypeSymbolRef, TypeSymbol>,
    pub vars: NodePool<VarSymbolRef, VarSymbol>,
    pub callables: NodePool<CallableSymbolRef, CallableSymbol>,
}

impl SemanticMetadata {
    pub fn get_expr_type(&self, expr_ref: &ExprRef) -> Option<&TypeSymbol> {
        self.expr_type_map.get(expr_ref).map(|r| self.types.get(*r))
    }
    pub fn get_expr_size(&self, expr_ref: &ExprRef) -> Option<&Size> {
        self.expr_type_map
            .get(expr_ref)
            .and_then(|t| self.type_sizes.get(t))
    }

    pub fn get_callable_symbol(&self, expr_ref: &ExprRef) -> Option<&CallableSymbol> {
        self.callable_symbols
            .get(expr_ref)
            .map(|s| self.callables.get(*s))
    }
    pub fn get_var_symbol(&self, expr_ref: &ExprRef) -> Option<&VarSymbol> {
        self.var_symbols.get(expr_ref).map(|s| self.vars.get(*s))
    }
    pub fn get_var_pass_mode(&self, expr_ref: &ExprRef) -> Option<&VarPassMode> {
        self.get_var_symbol(expr_ref).and_then(|s| match s {
            VarSymbol::Var { pass_mode, .. } => Some(pass_mode),
            _ => None,
        })
    }

    fn alloc_type(&mut self, type_symbol: TypeSymbol) -> TypeSymbolRef {
        let size = type_symbol.get_size(self);
        let type_ref = self.types.alloc(type_symbol);
        if let Some(size) = size {
            self.type_sizes.insert(type_ref, size);
        }
        type_ref
    }

    #[cfg(test)]
    fn get_expr_type_from_marker(&self, marker: usize, tree: &Tree) -> &TypeSymbol {
        use itertools::Itertools;

        let closest_expr = tree
            .expr_pool
            .ids()
            .filter(|&id| {
                let sp = tree.expr_pool.span(id);
                sp.start() < marker as u32
            })
            .sorted_by_key(|&id| {
                let sp = tree.expr_pool.span(id);
                sp.len()
            })
            .rev()
            .max_by_key(|&id| {
                let sp = tree.expr_pool.span(id);
                sp.start()
            })
            .expect("no expression found");
        let expr_type_ref = self
            .expr_type_map
            .get(&closest_expr)
            .expect("all expressions should have type");
        self.types.get(*expr_type_ref)
    }
}

#[derive(Debug, Clone)]
pub struct SemanticAnalyzer {
    semantic_metadata: SemanticMetadata,
    current_scope: Box<SymbolTable>,
    loop_depth: usize,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut callables = NodePool::new();
        let mut vars = NodePool::new();
        let mut types = NodePool::new();
        debug!(target: "pascal::semantic", "ENTER scope: builtin");
        let current_scope = SymbolTable::with_builtins(&mut types, &mut vars, &mut callables);
        let semantic_metadata = SemanticMetadata {
            types,
            vars,
            callables,
            type_type_map: HashMap::new(),
            expr_type_map: HashMap::new(),
            callable_symbols: HashMap::new(),
            var_symbols: HashMap::new(),
            var_types: HashMap::new(),
            type_sizes: HashMap::new(),
        };
        debug!(target: "pascal::semantic", "{}", current_scope.to_string(&semantic_metadata));
        let current_scope = SymbolTable::new(1, "global", Some(Box::new(current_scope)));
        Self {
            semantic_metadata,
            current_scope: Box::new(current_scope),
            loop_depth: 0,
        }
    }

    pub fn analyze(mut self, tree: &Tree) -> Result<SemanticMetadata, Error> {
        info!(target: "pascal::semantic", "Starting the Semantic analisys");
        debug!(target: "pascal::semantic", "ENTER scope: global");
        self.visit_stmt(tree.program, tree)?;
        debug!(target: "pascal::semantic", "{}", self.current_scope.to_string(&self.semantic_metadata));
        debug!(target: "pascal::semantic", "LEAVE scope: global");
        Ok(self.semantic_metadata)
    }

    fn visit_stmt(&mut self, node: StmtRef, tree: &Tree) -> Result<(), Error> {
        let pos = tree.node_pos(NodeRef::StmtRef(node));
        match tree.stmt_pool.get(node) {
            Stmt::Program { name: _, block } => self.visit_stmt(*block, tree),
            Stmt::Block {
                declarations,
                statements,
            } => {
                let errs: Errors = declarations
                    .iter()
                    .map(|d| self.visit_declaraction(d, tree))
                    .filter_map(Result::err)
                    .collect::<Vec<Error>>()
                    .into();
                let stmt_result = self.visit_stmt(*statements, tree);
                errs.add(stmt_result).into()
            }
            Stmt::Assign { left, right } => {
                let left_expr = tree.expr_pool.get(*left);
                let left_type_ref = self.visit_expr(*left, tree)?;
                match left_expr {
                    Expr::Var { name: _ } => {
                        let var_symbol = self
                            .semantic_metadata
                            .var_symbols
                            .get(left)
                            .expect("variable should have a symbol");
                        match self.semantic_metadata.vars.get(*var_symbol) {
                            VarSymbol::Var { .. } => (),
                            VarSymbol::Const { .. } => {
                                return Err(Error::SemanticError {
                                    msg: format!("cannot assign to const {:?}", var_symbol),
                                    pos,
                                    error_code: ErrorCode::AssignmentError,
                                });
                            }
                        }
                    }
                    Expr::Index {
                        base: _,
                        index_value: _,
                        other_indicies: _,
                    } => (),
                    _ => unreachable!(),
                }
                let right_type_ref = self.visit_expr(*right, tree)?;
                let left_type = self.semantic_metadata.types.get(left_type_ref);
                let right_type = self.semantic_metadata.types.get(right_type_ref);
                if !assinable(&self.semantic_metadata.types, &left_type, &right_type) {
                    return Err(Error::SemanticError {
                        msg: format!(
                            "value of type {:?} is not assignable to {:?}",
                            right_type, left_type
                        ),
                        pos,
                        error_code: ErrorCode::AssignmentError,
                    });
                }
                Ok(())
            }
            Stmt::Break => {
                if self.loop_depth <= 0 {
                    return Err(Error::SemanticError {
                        msg: "break should be within loop".to_string(),
                        pos,
                        error_code: ErrorCode::BreakOutsideLoop,
                    });
                };
                Ok(())
            }
            Stmt::Continue => {
                if self.loop_depth <= 0 {
                    return Err(Error::SemanticError {
                        msg: "continue should be within loop".to_string(),
                        pos,
                        error_code: ErrorCode::ContinueOutsideLoop,
                    });
                };
                Ok(())
            }
            Stmt::Call { call } => {
                let callable_expr = tree.expr_pool.get(*call);
                match callable_expr {
                    Expr::Call { name, args } => {
                        self.visit_callable(&call, tree, name, args)?;
                        let none_ref = self.semantic_metadata.alloc_type(TypeSymbol::Empty);
                        self.semantic_metadata.expr_type_map.insert(*call, none_ref);
                        Ok(())
                    }
                    _ => panic!("unreachable"),
                }
            }
            Stmt::Compound(stmts) => {
                let errs: Errors = stmts
                    .iter()
                    .map(|s| self.visit_stmt(*s, tree))
                    .filter_map(Result::err)
                    .collect::<Vec<Error>>()
                    .into();
                errs.into()
            }
            Stmt::Exit(v) => {
                if let Some(expr_ref) = v {
                    self.visit_expr(*expr_ref, tree)?;
                }
                Ok(())
            }
            Stmt::NoOp => Ok(()),
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => {
                self.visit_condition(cond, tree)?;
                elifs
                    .iter()
                    .map(|c| self.visit_condition(c, tree))
                    .collect::<Result<Vec<_>, Error>>()?;
                if let Some(stmt) = else_statement {
                    self.visit_stmt(*stmt, tree)?;
                }
                Ok(())
            }
            Stmt::While { cond, body } => {
                let type_ref = self.visit_expr(*cond, tree)?;
                let type_symbol = self.semantic_metadata.types.get(type_ref);
                if !matches!(type_symbol, TypeSymbol::Boolean) {
                    return Err(Error::SemanticError {
                        msg: format!("condition should be a boolean, got {:?}", type_symbol),
                        pos: tree.node_pos(NodeRef::StmtRef(node)),
                        error_code: ErrorCode::ConditionNotBoolean,
                    });
                };
                self.loop_depth += 1;
                self.visit_stmt(*body, tree)?;
                self.loop_depth -= 1;
                Ok(())
            }
            Stmt::For {
                var,
                init,
                end,
                body,
            } => {
                let init_state_type_ref = self.visit_expr(*init, tree)?;
                let end_state_type_ref = self.visit_expr(*end, tree)?;
                let var_type_ref = self.visit_expr(*var, tree)?;
                let var_type = self.semantic_metadata.types.get(var_type_ref);
                let init_state_type = self.semantic_metadata.types.get(init_state_type_ref);
                let end_state_type = self.semantic_metadata.types.get(end_state_type_ref);
                let var_symbol = self
                    .semantic_metadata
                    .vars
                    .get(*self.semantic_metadata.var_symbols.get(var).unwrap());
                match var_symbol {
                    VarSymbol::Var { name, .. } => {
                        if self.current_scope.lookup_var(name, false).is_none() {
                            return Err(Error::SemanticError {
                                msg: format!("unkown variable {name}"),
                                pos,
                                error_code: ErrorCode::UnkownVariable,
                            });
                        }
                    }
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("expected var, got {:?}", var_symbol),
                            pos,
                            error_code: ErrorCode::ExpectedVar,
                        });
                    }
                }
                if !TypeSymbol::eq(
                    &self.semantic_metadata.types,
                    &init_state_type,
                    &end_state_type,
                ) {
                    return Err(Error::SemanticError {
                        msg: format!(
                            "initial and end of for loop should have the same type, got {:?} and {:?}",
                            init_state_type, end_state_type
                        ),
                        pos: tree.node_pos(NodeRef::StmtRef(node)),
                        error_code: ErrorCode::IncompatibleTypes,
                    });
                }
                if !TypeSymbol::eq(&self.semantic_metadata.types, &var_type, &init_state_type) {
                    return Err(Error::SemanticError {
                        msg: format!(
                            "variable type of for loop should be the same as limit types, limit is {:?}, but variable is {:?}",
                            init_state_type, var_type
                        ),
                        pos: tree.node_pos(NodeRef::StmtRef(node)),
                        error_code: ErrorCode::IncompatibleTypes,
                    });
                }
                self.loop_depth += 1;
                self.visit_stmt(*body, tree)?;
                self.loop_depth -= 1;
                Ok(())
            }
        }
    }
    fn visit_expr(&mut self, node: ExprRef, tree: &Tree) -> Result<TypeSymbolRef, Error> {
        let pos = tree.node_pos(NodeRef::ExprRef(node));
        let type_symbol = match tree.expr_pool.get(node) {
            Expr::LiteralBool(_) => Ok::<TypeSymbol, Error>(TypeSymbol::Boolean),
            Expr::LiteralChar(_) => Ok(TypeSymbol::Char),
            Expr::LiteralInteger(_) => Ok(TypeSymbol::Integer),
            Expr::LiteralInt64(_) => Ok(TypeSymbol::Int64),
            Expr::LiteralReal(_) => Ok(TypeSymbol::Real),
            Expr::LiteralString(_) => Ok(TypeSymbol::String),
            Expr::Var { name } => {
                let (var_symbol_ref, var_kind) = self
                    .current_scope
                    .lookup_var(name.lexem(tree.source_code), false)
                    .ok_or(Error::SemanticError {
                        msg: format!("var {} is unkown", name.lexem(tree.source_code)),
                        pos,
                        error_code: ErrorCode::UnkownVariable,
                    })?;
                self.semantic_metadata.var_types.insert(node, var_kind);
                let var_symbol = self.semantic_metadata.vars.get(var_symbol_ref);
                self.semantic_metadata
                    .var_symbols
                    .insert(node, var_symbol_ref);
                let type_symbol = match var_symbol {
                    &VarSymbol::Var { type_symbol, .. } => type_symbol,
                    &VarSymbol::Const { type_symbol, .. } => type_symbol,
                };
                self.semantic_metadata
                    .expr_type_map
                    .insert(node, type_symbol);
                return Ok(type_symbol);
            }
            Expr::BinOp { op, left, right } => {
                let left_type_ref = self.visit_expr(*left, tree)?;
                let right_type_ref = self.visit_expr(*right, tree)?;
                let left_type = self.semantic_metadata.types.get(left_type_ref);
                let right_type = self.semantic_metadata.types.get(right_type_ref);
                let type_symbol = match op {
                    TokenType::Minus | TokenType::RealDiv | TokenType::Mul => {
                        match (&left_type, &right_type) {
                            (TypeSymbol::Integer, TypeSymbol::Integer) => Ok(TypeSymbol::Integer),
                            (
                                TypeSymbol::Int64 | TypeSymbol::Integer,
                                TypeSymbol::Int64 | TypeSymbol::Integer,
                            ) => Ok(TypeSymbol::Int64),
                            (
                                TypeSymbol::Real | TypeSymbol::Integer | TypeSymbol::Int64,
                                TypeSymbol::Real | TypeSymbol::Integer | TypeSymbol::Int64,
                            ) => Ok(TypeSymbol::Real),

                            _ => Err(Error::SemanticError {
                                msg: format!(
                                    "operator {:?} is not supported for {:?} and {:?}",
                                    op, left_type, right_type
                                ),
                                pos,
                                error_code: ErrorCode::UnsupportedBinaryOperation,
                            }),
                        }
                    }
                    TokenType::IntegerDiv => match (&left_type, &right_type) {
                        (TypeSymbol::Integer | TypeSymbol::Real, TypeSymbol::Integer) => {
                            Ok(TypeSymbol::Integer)
                        }
                        (
                            TypeSymbol::Int64 | TypeSymbol::Real | TypeSymbol::Integer,
                            TypeSymbol::Int64,
                        ) => Ok(TypeSymbol::Int64),
                        (TypeSymbol::Int64, TypeSymbol::Integer) => Ok(TypeSymbol::Int64),
                        _ => Err(Error::SemanticError {
                            msg: format!(
                                "integer division is not supported for {:?} and {:?}",
                                left_type, right_type
                            ),
                            pos,
                            error_code: ErrorCode::UnsupportedBinaryOperation,
                        }),
                    },
                    TokenType::Plus => match (&left_type, &right_type) {
                        (TypeSymbol::String, TypeSymbol::String | TypeSymbol::Char) => {
                            Ok(TypeSymbol::String)
                        }
                        (TypeSymbol::Integer, TypeSymbol::Integer) => Ok(TypeSymbol::Integer),
                        (
                            TypeSymbol::Int64 | TypeSymbol::Integer,
                            TypeSymbol::Int64 | TypeSymbol::Integer,
                        ) => Ok(TypeSymbol::Int64),
                        (
                            TypeSymbol::Real | TypeSymbol::Integer | TypeSymbol::Int64,
                            TypeSymbol::Real | TypeSymbol::Integer | TypeSymbol::Int64,
                        ) => Ok(TypeSymbol::Real),
                        _ => Err(Error::SemanticError {
                            msg: format!(
                                "+ is not supported for {:?} and {:?}",
                                left_type, right_type
                            ),
                            pos,
                            error_code: ErrorCode::UnsupportedBinaryOperation,
                        }),
                    },
                    TokenType::GreaterThen
                    | TokenType::GreaterEqual
                    | TokenType::LessEqual
                    | TokenType::LessThen => match (left_type, right_type) {
                        (
                            TypeSymbol::Integer | TypeSymbol::Real | TypeSymbol::Int64,
                            TypeSymbol::Integer | TypeSymbol::Real | TypeSymbol::Int64,
                        ) => Ok(TypeSymbol::Boolean),
                        _ => Err(Error::SemanticError {
                            msg: format!(
                                "compare operator is not supported for {:?} and {:?}",
                                left_type, right_type
                            ),
                            pos,
                            error_code: ErrorCode::UnsupportedBinaryOperation,
                        }),
                    },
                    TokenType::Equal | TokenType::NotEqual => Ok(TypeSymbol::Boolean),
                    TokenType::And | TokenType::Or => match (left_type, right_type) {
                        (TypeSymbol::Boolean, TypeSymbol::Boolean) => Ok(TypeSymbol::Boolean),
                        _ => Err(Error::SemanticError {
                            msg: format!(
                                "operator AND/OR is not supported for {:?} and {:?}",
                                left_type, right_type
                            ),
                            pos,
                            error_code: ErrorCode::UnsupportedBinaryOperation,
                        }),
                    },
                    _ => unreachable!(),
                }?;
                Ok(type_symbol)
            }
            Expr::UnaryOp { op, expr: expr_ref } => {
                let expr_type_ref = self.visit_expr(*expr_ref, tree)?;
                let expr_type = self.semantic_metadata.types.get(expr_type_ref);
                let type_symbol = match (op, expr_type) {
                    (TokenType::Not, TypeSymbol::Boolean) => Ok(TypeSymbol::Boolean),
                    (TokenType::Minus | TokenType::Plus, TypeSymbol::Integer) => {
                        Ok(TypeSymbol::Integer)
                    }
                    (TokenType::Minus | TokenType::Plus, TypeSymbol::Int64) => {
                        Ok(TypeSymbol::Int64)
                    }
                    (TokenType::Minus | TokenType::Plus, TypeSymbol::Real) => Ok(TypeSymbol::Real),
                    (_, _) => Err(Error::SemanticError {
                        msg: format!(
                            "unary operator {:?} is not applicable for {:?}",
                            op, expr_type
                        ),
                        pos,
                        error_code: ErrorCode::UnsupportedUnaryOperator,
                    }),
                }?;
                Ok(type_symbol)
            }
            Expr::Call { name, args } => {
                let type_ref =
                    self.visit_callable(&node, tree, name, args)?
                        .ok_or(Error::SemanticError {
                            msg: "procedure cannot be used in an expression".to_string(),
                            pos: tree.node_pos(NodeRef::ExprRef(node)),
                            error_code: ErrorCode::IncorrectUseOfProcedure,
                        })?;
                self.semantic_metadata.expr_type_map.insert(node, type_ref);
                return Ok(type_ref);
            }
            Expr::Index {
                base,
                index_value,
                other_indicies: _, // TODO: handle other indicies
            } => {
                let actual_index_type_ref = self.visit_expr(*index_value, tree)?;
                let var_type_ref = self.visit_expr(*base, tree)?;
                let actual_index_type = self.semantic_metadata.types.get(actual_index_type_ref);
                let var_type = self.semantic_metadata.types.get(var_type_ref);
                let base_type_ref = match var_type {
                    TypeSymbol::Array {
                        index_type: index_type_ref,
                        value_type,
                        ..
                    } => {
                        let index_type = self.semantic_metadata.types.get(*index_type_ref);
                        if !TypeSymbol::eq(
                            &self.semantic_metadata.types,
                            index_type,
                            &actual_index_type,
                        ) {
                            return Err(Error::SemanticError {
                                msg: format!(
                                    "expected {:?} as index, got {:?}",
                                    index_type, actual_index_type
                                ),
                                pos,
                                error_code: ErrorCode::IncorrectIndexType,
                            });
                        }
                        value_type
                    }
                    TypeSymbol::DynamicArray(v) => {
                        let actual_index_type = match actual_index_type {
                            &TypeSymbol::Range {
                                start_ord_index: _,
                                end_ord_index: _,
                                range_type: t,
                            } => self.semantic_metadata.types.get(t),
                            _ => actual_index_type,
                        };
                        if !matches!(actual_index_type, TypeSymbol::Integer) {
                            return Err(Error::SemanticError {
                                msg: format!(
                                    "dynamic array index should be integer, got {:?}",
                                    actual_index_type
                                ),
                                pos,
                                error_code: ErrorCode::IncorrectIndexType,
                            });
                        }
                        v
                    }
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("base of indexable should be array, got {:?}", var_type),
                            pos,
                            error_code: ErrorCode::IncorrectBaseType,
                        });
                    }
                };
                self.semantic_metadata
                    .expr_type_map
                    .insert(node, *base_type_ref);
                return Ok(*base_type_ref);
            }
        }?;
        let type_symbol_ref = self.semantic_metadata.alloc_type(type_symbol);
        self.semantic_metadata
            .expr_type_map
            .insert(node, type_symbol_ref);
        Ok(type_symbol_ref)
    }

    fn visit_type(&mut self, node: TypeRef, tree: &Tree) -> Result<TypeSymbolRef, Error> {
        let pos = tree.node_pos(NodeRef::TypeRef(node));
        let type_symbol = match tree.type_pool.get(node) {
            Type::Integer => Ok::<TypeSymbol, Error>(TypeSymbol::Integer),
            Type::Int64 => Ok(TypeSymbol::Int64),
            Type::Real => Ok(TypeSymbol::Real),
            Type::Boolean => Ok(TypeSymbol::Boolean),
            Type::Char => Ok(TypeSymbol::Char),
            Type::String => Ok(TypeSymbol::String),
            Type::Enum { items } => {
                let type_symbol = TypeSymbol::Enum(
                    items
                        .iter()
                        .map(|t| t.lexem(tree.source_code).to_string())
                        .collect(),
                );
                let type_symbol_ref = self.semantic_metadata.alloc_type(type_symbol);
                let errors: Errors = items
                    .iter()
                    .map(|t| t.lexem(tree.source_code))
                    .enumerate()
                    .map(|(i, n)| {
                        if let Some(_) = self.current_scope.lookup_var(n, false) {
                            return Err(Error::SemanticError {
                                msg: format!(
                                    "enum value share the same name with a const value {n}"
                                ),
                                pos,
                                error_code: ErrorCode::DuplicateVarDefinition,
                            });
                        }
                        let c = self.semantic_metadata.vars.alloc(VarSymbol::Const {
                            value: ConstValue::Integer(i as i32),
                            type_symbol: type_symbol_ref,
                        });
                        self.current_scope.define_var(n, c);
                        Ok(())
                    })
                    .filter_map(Result::err)
                    .collect::<Vec<Error>>()
                    .into();
                return errors.result(type_symbol_ref);
            }
            Type::Alias(v) => {
                let alias = self
                    .current_scope
                    .lookup_type(v.lexem(tree.source_code), false)
                    .ok_or(Error::SemanticError {
                        msg: format!("type {:?} is unknown", v),
                        pos,
                        error_code: ErrorCode::UnkownType,
                    })?;
                self.semantic_metadata.type_type_map.insert(node, alias);
                return Ok(alias);
            }
            Type::Array {
                index_type,
                element_type,
            } => {
                let index_type_ref = self.visit_type(*index_type, tree)?;
                let index_type = self.semantic_metadata.types.get(index_type_ref);
                let (start_ord_index, end_ord_index) = match index_type {
                    &TypeSymbol::Range {
                        start_ord_index,
                        end_ord_index,
                        range_type: _,
                    } => (start_ord_index, end_ord_index),
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("array index type should be range, got {:?}", index_type),
                            pos,
                            error_code: ErrorCode::IncorrectIndexType,
                        });
                    }
                };
                let element_type = self.visit_type(*element_type, tree)?;
                Ok(TypeSymbol::Array {
                    start_ord_index,
                    end_ord_index,
                    index_type: index_type_ref,
                    value_type: element_type,
                })
            }
            Type::DynamicArray { element_type } => {
                let element_type = self.visit_type(*element_type, tree)?;
                Ok(TypeSymbol::DynamicArray(element_type))
            }
            Type::Range { start_val, end_val } => {
                let start_val_type_ref = self.visit_expr(*start_val, tree)?;
                let end_val_type_ref = self.visit_expr(*end_val, tree)?;
                let start_val_type = self.semantic_metadata.types.get(start_val_type_ref);
                let end_val_type = self.semantic_metadata.types.get(end_val_type_ref);

                if !TypeSymbol::eq(
                    &self.semantic_metadata.types,
                    &start_val_type,
                    &end_val_type,
                ) {
                    return Err(Error::SemanticError {
                        msg: format!(
                            "range limits should be of the same type, got {:?} and {:?}",
                            start_val_type, end_val_type
                        ),
                        pos,
                        error_code: ErrorCode::IncompatibleTypes,
                    });
                }
                if !start_val_type.is_ordinal() {
                    return Err(Error::SemanticError {
                        msg: format!("range limits should be ordinal, got {:?}", start_val_type),
                        pos,
                        error_code: ErrorCode::RangeLimitsNotOrdinal,
                    });
                }
                let start_ord_index = start_val_type.ordinal_rank(
                    &tree.expr_pool.get(*start_val).into_value(tree).unwrap(),
                    &self.semantic_metadata,
                );
                let end_ord_index = start_val_type.ordinal_rank(
                    &tree.expr_pool.get(*end_val).into_value(tree).unwrap(),
                    &self.semantic_metadata,
                );

                Ok(TypeSymbol::Range {
                    start_ord_index,
                    end_ord_index,
                    range_type: start_val_type_ref,
                })
            }
        }?;
        let type_symbol_ref = self.semantic_metadata.alloc_type(type_symbol);
        self.semantic_metadata
            .type_type_map
            .insert(node, type_symbol_ref);
        Ok(type_symbol_ref)
    }

    fn visit_declaraction(&mut self, decl: &Decl, tree: &Tree) -> Result<(), Error> {
        match decl {
            Decl::ConstDecl { var, literal } => {
                let var_expr = tree.expr_pool.get(*var);
                let var_name = match var_expr {
                    Expr::Var { name } => name,
                    _ => unreachable!(),
                };
                let literal_expr = tree.expr_pool.get(*literal);
                let const_type = match literal_expr {
                    Expr::LiteralInteger(v) => ConstValue::Integer(*v),
                    Expr::LiteralInt64(v) => ConstValue::Int64(*v),
                    Expr::LiteralBool(v) => ConstValue::Boolean(*v),
                    Expr::LiteralReal(v) => ConstValue::Real(*v),
                    Expr::LiteralString(v) => ConstValue::String(v.lexem(tree.source_code).into()),
                    Expr::LiteralChar(c) => ConstValue::Char(*c),
                    _ => unreachable!(),
                };
                let type_symbol = self.visit_expr(*literal, tree)?;
                let const_symbol =
                    self.semantic_metadata
                        .vars
                        .alloc(crate::symbols::VarSymbol::Const {
                            value: const_type,
                            type_symbol,
                        });
                self.current_scope
                    .define_var(var_name.lexem(tree.source_code), const_symbol);
                Ok(())
            }
            Decl::Callable {
                name,
                block,
                params,
                return_type,
            } => {
                let new_scope_level = self.current_scope.get_scope_level() + 1;
                let old =
                    std::mem::replace(&mut self.current_scope, Box::new(SymbolTable::default()));
                self.current_scope = Box::new(SymbolTable::new(
                    new_scope_level,
                    name.lexem(tree.source_code),
                    Some(old),
                ));
                debug!(target: "pascal::semantic", "ENTER scope: {}", self.current_scope.scope_name());
                let mut params_vec: Vec<VarSymbolRef> = Vec::with_capacity(params.len());
                for param in params {
                    let var_expr = tree.expr_pool.get(param.var);
                    let var_name = match var_expr {
                        Expr::Var { name } => name,
                        _ => unreachable!(),
                    };
                    let type_symbol_ref = self.visit_type(param.type_node, tree)?;
                    self.semantic_metadata
                        .expr_type_map
                        .insert(param.var, type_symbol_ref);
                    let param_mode = match param.out {
                        true => VarPassMode::Ref,
                        false => VarPassMode::Val,
                    };
                    let var_symbol = VarSymbol::Var {
                        name: var_name.lexem(tree.source_code).to_string(),
                        pass_mode: param_mode,
                        type_symbol: type_symbol_ref,
                    };
                    let var_symbol_ref = self.semantic_metadata.vars.alloc(var_symbol);
                    self.semantic_metadata
                        .var_symbols
                        .insert(param.var, var_symbol_ref);
                    self.current_scope
                        .define_var(var_name.lexem(tree.source_code), var_symbol_ref);
                    params_vec.push(var_symbol_ref);
                }
                let return_type = match return_type {
                    Some(return_type_ref) => Some(self.visit_type(*return_type_ref, tree)?),
                    None => None,
                };
                let callable_symbol = CallableSymbol {
                    name: name.lexem(tree.source_code).into(),
                    params: params_vec,
                    param_input_mode: ParamInputMode::Seq,
                    return_type,
                    body: CallableType::Custom { statement: *block },
                };
                let callable_symbol_ref = self.semantic_metadata.callables.alloc(callable_symbol);
                self.current_scope
                    .get_mut_enclosing_scope()
                    .expect("there is always enclosing scope here")
                    .define_callable(name.lexem(tree.source_code), callable_symbol_ref);
                if let Some(return_type_ref) = return_type {
                    let (return_assigned, can_fallthrough) =
                        analyze_function(tree, name.lexem(tree.source_code), *block, false)?;
                    if can_fallthrough && !return_assigned {
                        return Err(Error::SemanticError {
                            msg: format!(
                                "function {} may not return a result",
                                name.lexem(tree.source_code)
                            ),
                            pos: name.pos(),
                            error_code: ErrorCode::FunctionMayNotReturn,
                        });
                    }
                    let return_var = self.semantic_metadata.vars.alloc(VarSymbol::Var {
                        name: "result".to_string(),
                        pass_mode: VarPassMode::Val,
                        type_symbol: return_type_ref,
                    });
                    self.current_scope.define_var("result", return_var);
                    let return_var = self.semantic_metadata.vars.alloc(VarSymbol::Var {
                        name: name.lexem(tree.source_code).into(),
                        pass_mode: VarPassMode::Val,
                        type_symbol: return_type_ref,
                    });
                    self.current_scope
                        .define_var(name.lexem(tree.source_code), return_var);
                }
                self.visit_stmt(*block, tree)?;
                debug!(target: "pascal::semantic", "{}", self.current_scope.to_string(&self.semantic_metadata));
                debug!(target: "pascal::semantic", "LEAVE scope: {}", self.current_scope.scope_name());
                let enclosing_scope = self
                    .current_scope
                    .take_enclosing_scope()
                    .expect("there is always enclosing scope here");
                self.current_scope = enclosing_scope;
                Ok(())
            }
            Decl::TypeDecl { var, type_node } => {
                let var_expr = tree.type_pool.get(*var);
                let var_name = match var_expr {
                    &Type::Alias(name) => name,
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("expected var, got {:?}", var_expr),
                            pos: tree.node_pos(NodeRef::TypeRef(*var)),
                            error_code: ErrorCode::ExpectedVar,
                        });
                    }
                };
                if let Some(_) = self
                    .current_scope
                    .lookup_type(var_name.lexem(tree.source_code), true)
                {
                    return Err(Error::SemanticError {
                        msg: format!("type {:?} is already defined", var_name),
                        pos: tree.node_pos(NodeRef::TypeRef(*var)),
                        error_code: ErrorCode::DuplicateTypeDefinition,
                    });
                }
                let type_symbol_ref = self.visit_type(*type_node, tree)?;
                self.current_scope
                    .define_type(var_name.lexem(tree.source_code), type_symbol_ref);
                Ok(())
            }
            Decl::VarDecl {
                var: var_ref,
                type_node,
                default_value,
            } => {
                let var = tree.expr_pool.get(*var_ref);
                let var_name = match var {
                    Expr::Var { name } => name,
                    _ => unreachable!(),
                };
                if let Some(_) = self
                    .current_scope
                    .lookup_var(var_name.lexem(tree.source_code), true)
                {
                    return Err(Error::SemanticError {
                        msg: format!("var {:?} is already defined", var_name),
                        pos: tree.node_pos(NodeRef::ExprRef(*var_ref)),
                        error_code: ErrorCode::DuplicateVarDefinition,
                    });
                }
                let type_symbol_ref = self.visit_type(*type_node, tree)?;

                if let Some(expr) = default_value {
                    let default_type_ref = self.visit_expr(*expr, tree)?;
                    let default_type = self.semantic_metadata.types.get(default_type_ref);
                    let type_symbol = self.semantic_metadata.types.get(type_symbol_ref);
                    if !assinable(&self.semantic_metadata.types, type_symbol, default_type) {
                        return Err(Error::SemanticError {
                            msg: format!(
                                "default value have the type {:?} and is not assinable to {:?}",
                                default_type, type_symbol
                            ),
                            pos: tree.node_pos(NodeRef::ExprRef(*expr)),
                            error_code: ErrorCode::IncorrectType,
                        });
                    }
                }

                let var_symbol =
                    self.semantic_metadata
                        .vars
                        .alloc(crate::symbols::VarSymbol::Var {
                            name: var_name.lexem(tree.source_code).into(),
                            pass_mode: VarPassMode::Val,
                            type_symbol: type_symbol_ref,
                        });
                self.current_scope
                    .define_var(var_name.lexem(tree.source_code), var_symbol);
                self.semantic_metadata
                    .expr_type_map
                    .insert(*var_ref, type_symbol_ref);
                Ok(())
            }
        }
    }
    fn visit_callable(
        &mut self,
        node: &ExprRef,
        tree: &Tree,
        name: &Token,
        args: &Vec<ExprRef>,
    ) -> Result<Option<TypeSymbolRef>, Error> {
        let pos = tree.node_pos(NodeRef::ExprRef(*node));
        let callable_symbol_ref = self
            .current_scope
            .lookup_callable(name.lexem(tree.source_code), false)
            .ok_or(Error::SemanticError {
                msg: format!("could not find callable {}", name.lexem(tree.source_code)),
                pos,
                error_code: ErrorCode::UnkownCallable,
            })?;
        let arg_expr = args
            .iter()
            .map(|a| self.visit_expr(*a, tree))
            .collect::<Result<Vec<_>, Error>>()?;
        let callable_symbol = self.semantic_metadata.callables.get(callable_symbol_ref);
        let return_type = callable_symbol.return_type;
        let arg_count = arg_expr.len();
        match callable_symbol.param_input_mode {
            ParamInputMode::Seq => {
                if callable_symbol.params.len() != args.len() {
                    return Err(Error::SemanticError {
                        msg: format!(
                            "function {} expected {} arguments, but got {}",
                            name.lexem(tree.source_code),
                            callable_symbol.params.len(),
                            arg_count,
                        ),
                        pos,
                        error_code: ErrorCode::IncorrectNumberOfArguments,
                    });
                }
            }
            ParamInputMode::Repeat => {
                if arg_count % callable_symbol.params.len() != 0 {
                    return Err(Error::SemanticError {
                        msg: format!(
                            "function {} accepts {} arguements any amount of times, but number of provided arguments does not fit",
                            name.lexem(tree.source_code),
                            callable_symbol.params.len()
                        ),
                        pos,
                        error_code: ErrorCode::IncorrectNumberOfArguments,
                    });
                }
            }
        };
        let param_results: Errors = callable_symbol
            .params
            .iter()
            .cycle()
            .take(args.len())
            .zip(arg_expr)
            .map(|(p, expr_type)| {
                let expr_type = self.semantic_metadata.types.get(expr_type);
                let var_symbol = self.semantic_metadata.vars.get(*p);
                let (param_type, mode) = match var_symbol {
                    VarSymbol::Var {
                        type_symbol,
                        pass_mode,
                        ..
                    } => (self.semantic_metadata.types.get(*type_symbol), pass_mode),
                    _ => panic!("unreachable"),
                };
                if matches!(mode, VarPassMode::Ref)
                    && !TypeSymbol::eq(&self.semantic_metadata.types, param_type, expr_type)
                    || !assinable(&self.semantic_metadata.types, &param_type, &expr_type)
                {
                    return Err(Error::SemanticError {
                        msg: format!(
                            "parameter should be of type {:?}, got {:?}",
                            param_type, expr_type
                        ),
                        pos,
                        error_code: ErrorCode::IncorrectType,
                    });
                };
                Ok(())
            })
            .filter_map(Result::err)
            .collect::<Vec<Error>>()
            .into();
        match param_results.into() {
            Err(e) => return Err(e),
            Ok(()) => (),
        }
        self.semantic_metadata
            .callable_symbols
            .insert(*node, callable_symbol_ref);
        Ok(return_type)
    }
    fn visit_condition(&mut self, cond: &Condition, tree: &Tree) -> Result<(), Error> {
        let type_ref = self.visit_expr(cond.cond, tree)?;
        let type_symbol = self.semantic_metadata.types.get(type_ref);
        if !matches!(type_symbol, TypeSymbol::Boolean) {
            return Err(Error::SemanticError {
                msg: format!("condition should be boolean, got {:?}", type_symbol),
                pos: tree.node_pos(NodeRef::ExprRef(cond.cond)),
                error_code: ErrorCode::ConditionNotBoolean,
            });
        };
        self.visit_stmt(cond.expr, tree)?;
        Ok(())
    }
}
fn analyze_function(
    tree: &Tree,
    function_name: &str,
    stmt_node: StmtRef,
    in_assigned: bool,
) -> Result<(bool, bool), Error> {
    let pos = tree.node_pos(NodeRef::StmtRef(stmt_node));
    let stmt_node = tree.stmt_pool.get(stmt_node);
    match stmt_node {
        Stmt::Block {
            declarations: _,
            statements,
        } => Ok(analyze_function(
            tree,
            function_name,
            *statements,
            in_assigned,
        )?),
        Stmt::Exit(e) => {
            if let Some(_) = e {
                return Ok((true, false));
            }
            if !in_assigned {
                return Err(Error::SemanticError {
                    msg: format!("function exited without returning anything"),
                    pos,
                    error_code: ErrorCode::FunctionMayNotReturn,
                });
            }
            Ok((in_assigned, true))
        }
        Stmt::Assign { left, right: _ } => {
            let left_expr = tree.expr_pool.get(*left);
            match left_expr {
                Expr::Var { name } => Ok((
                    ["result", &function_name].contains(&name.lexem(tree.source_code))
                        || in_assigned,
                    true,
                )),
                Expr::Index { .. } => Ok((in_assigned, true)),
                _ => unreachable!(),
            }
        }
        Stmt::If {
            cond,
            elifs,
            else_statement,
        } => {
            let mut thens = vec![analyze_function(
                tree,
                function_name,
                cond.expr,
                in_assigned,
            )];
            thens.extend(elifs.iter().map(
                |Condition {
                     cond: _,
                     expr: expr_ref,
                 }| {
                    analyze_function(tree, function_name, *expr_ref, in_assigned)
                },
            ));
            match else_statement {
                Some(stmt) => {
                    thens.push(analyze_function(tree, function_name, *stmt, in_assigned));
                }
                None => thens.push(Ok((in_assigned, true))),
            };
            let thens = thens.into_iter().collect::<Result<Vec<_>, Error>>()?;
            let fall = thens.iter().any(|(_, fall)| *fall);
            let out = thens
                .iter()
                .fold(true, |v, (out, fall)| v && (!*fall | *out));
            Ok((out, fall))
        }
        Stmt::While { cond: _, body } => {
            analyze_function(tree, function_name, *body, in_assigned)?;
            Ok((in_assigned, true))
        }
        Stmt::For {
            var: _,
            init: _,
            end: _,
            body,
        } => {
            analyze_function(tree, function_name, *body, in_assigned)?;
            Ok((in_assigned, true))
        }
        Stmt::Compound(stmts) => {
            let mut assign = in_assigned;
            let mut fall = true;
            for stmt in stmts {
                if !fall {
                    break;
                }
                (assign, fall) = analyze_function(tree, function_name, *stmt, assign)?;
            }
            Ok((assign, fall))
        }
        _ => Ok((in_assigned, true)),
    }
}

fn assinable(
    types: &NodePool<TypeSymbolRef, TypeSymbol>,
    left: &TypeSymbol,
    right: &TypeSymbol,
) -> bool {
    if let TypeSymbol::Any = left {
        return true;
    }
    if TypeSymbol::eq(types, left, right) {
        return true;
    }
    matches!(
        (left, right),
        (TypeSymbol::Real, TypeSymbol::Integer)
            | (TypeSymbol::String, TypeSymbol::Char)
            | (TypeSymbol::Int64, TypeSymbol::Integer)
            | (TypeSymbol::Integer, TypeSymbol::Int64)
    )
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use regex::Regex;

    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    static MARKER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\{\s*#([a-zA-Z0-9_]+)\s*\}").expect("incorrect regex"));

    fn parse_markers(src: &str) -> HashMap<String, usize> {
        let mut markers = HashMap::new();
        MARKER_RE
            .captures_iter(src)
            .map(|m| (m.get_match(), m.get(1).unwrap()))
            .for_each(|(m, name)| {
                markers.insert(name.as_str().to_string(), m.start());
            });
        markers
    }

    macro_rules! test_fail {
        ($($name:ident -> [$err:path $(, $other_err:path)* $(,)?],)+) => {
            $(
                #[test]
                fn $name() {
                    let source_path = "test_cases\\semantic_analyzer\\".to_string() + &stringify!($name) + ".pas";
                    let source_code = std::fs::read_to_string(&source_path).unwrap_or_else(|r| panic!("file {source_path} does not exist: {r}"));
                    let lexer = Lexer::new(&source_code);
                    let tree = Parser::new(lexer).unwrap().parse().unwrap();
                    let result = SemanticAnalyzer::new().analyze(&tree);
                    assert!(result.is_err());
                    let err: Errors = result.unwrap_err().into();
                    let mut expected = Vec::new();
                    expected.push($err);
                    $(
                        expected.push($other_err);
                    )*
                    assert_eq!(expected.len(), err.len());
                    for (exp, err) in expected.iter().zip(err.iter()) {
                        match err {
                            Error::SemanticError { error_code, ..} => assert_eq!(error_code, exp),
                            _ => panic!("encountered non semantic error")
                        }
                    }
                }
            )+
        };
    }

    macro_rules! test_succ {
        ($($name:ident -> {
            $($first_marker:ident: $first_type:pat $(, $marker:ident: $type:pat)*$(,)?)?
        },)+) => {
            $(
                #[test]
                fn $name() {
                    let source_path = "test_cases\\semantic_analyzer\\".to_string() + &stringify!($name) + ".pas";
                    let source_code = std::fs::read_to_string(&source_path).expect(&format!("file {source_path} does not exist"));
                    let _markers = parse_markers(&source_code);
                    let lexer = Lexer::new(&source_code);
                    let tree = Parser::new(lexer).unwrap().parse().unwrap();
                    let result = SemanticAnalyzer::new().analyze(&tree);
                    assert!(result.is_ok(), "unexpected error: {:?}", result);
                    let _semantic_metadata = result.unwrap();
                    let missing: Vec<&Expr> = tree
                        .expr_pool
                        .ids()
                        .filter(|k| !_semantic_metadata.expr_type_map.contains_key(k))
                        .map(|id| tree.expr_pool.get(id))
                        .collect();
                    assert_eq!(missing, Vec::<&Expr>::new());
                    $(
                        let first_marker = stringify!($first_marker);
                        let marked_type = _semantic_metadata.get_expr_type_from_marker(_markers[first_marker], &tree);
                        assert!(matches!(marked_type, $first_type), "marker {} expected {}, got {:?}", stringify!($firat_marker), stringify!($first_type), marked_type);
                        $(
                            let marked_type = _semantic_metadata.get_expr_type_from_marker(_markers[stringify!($marker)], &tree);
                            assert!(matches!(marked_type, $type), "marker {} expected {}, got {:?}", stringify!($marker), stringify!($type), marked_type);
                        )*
                    )?
                }
            )+
        };
    }

    test_succ! {
        test_base -> {
            one: TypeSymbol::Integer,
            two: TypeSymbol::Real,
            three: TypeSymbol::Empty,
            paren: TypeSymbol::Integer,
        },
        test_array -> {
            range: TypeSymbol::Range{..},
            arr: TypeSymbol::Array{..},
            dyn_arr: TypeSymbol::DynamicArray(_)
        },
        test_function -> {
            func: TypeSymbol::Integer,
            res1: TypeSymbol::Integer,
            res2: TypeSymbol::Integer,
        },
        test_enum -> {

        },
        test_bin_op_integral -> {
            a1: TypeSymbol::Int64,
            a2: TypeSymbol::Int64,
            b1: TypeSymbol::Integer,
            p: TypeSymbol::Int64
        },
    }

    test_fail! {
        test_assign_fail -> [
            ErrorCode::AssignmentError,
            ErrorCode::AssignmentError,
            ErrorCode::AssignmentError,
        ],
        test_assign_to_const -> [ErrorCode::AssignmentError],
        test_break_continue_fail -> [
            ErrorCode::BreakOutsideLoop,
            ErrorCode::ContinueOutsideLoop,
        ],
        test_boolean_condition_fail -> [
            ErrorCode::ConditionNotBoolean,
            ErrorCode::ConditionNotBoolean,
        ],
        test_for_unkown_var_fail -> [
            ErrorCode::UnkownVariable,
            ErrorCode::ExpectedVar,
        ],
        test_for_limit_types_fail -> [
            ErrorCode::IncompatibleTypes,
            ErrorCode::IncompatibleTypes,
        ],
        test_var_unkown_fail -> [
            ErrorCode::UnkownVariable
        ],
        test_bin_op_fail -> [
            ErrorCode::UnsupportedBinaryOperation,
            ErrorCode::UnsupportedBinaryOperation,
            ErrorCode::UnsupportedBinaryOperation,
            ErrorCode::UnsupportedBinaryOperation,
            ErrorCode::UnsupportedBinaryOperation,
            ErrorCode::UnsupportedBinaryOperation,
            ErrorCode::UnsupportedBinaryOperation,
            ErrorCode::UnsupportedBinaryOperation,
        ],
        test_unary_op_fail -> [
            ErrorCode::UnsupportedUnaryOperator,
            ErrorCode::UnsupportedUnaryOperator,
        ],
        test_procedure_in_expr_fail -> [
            ErrorCode::IncorrectUseOfProcedure
        ],
        test_index_fail -> [
            ErrorCode::IncorrectIndexType,
            ErrorCode::IncorrectIndexType,
            ErrorCode::IncorrectIndexType,
            ErrorCode::IncorrectBaseType,
        ],
        test_type_fail -> [
            ErrorCode::UnkownType,
            ErrorCode::IncompatibleTypes,
            ErrorCode::RangeLimitsNotOrdinal,
        ],
        test_function_may_not_return_fail -> [
            ErrorCode::FunctionMayNotReturn,
            ErrorCode::FunctionMayNotReturn,
        ],
        test_already_defined_fail -> [
            ErrorCode::DuplicateTypeDefinition,
            ErrorCode::DuplicateVarDefinition,
        ],
        test_default_fail -> [ErrorCode::IncorrectType],
        test_call_fail -> [
            ErrorCode::UnkownCallable,
            ErrorCode::IncorrectNumberOfArguments,
            ErrorCode::IncorrectNumberOfArguments,
            ErrorCode::IncorrectType,
        ],
    }
}

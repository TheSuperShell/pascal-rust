use std::collections::HashMap;

use crate::{
    error::{Error, ErrorCode, Errors},
    parser::{Condition, Decl, Expr, ExprRef, NodeRef, Stmt, StmtRef, Tree, Type, TypeRef},
    symbols::{
        CallableSymbol, CallableSymbolRef, CallableType, ConstValue, ParamInputMode, ParamMode,
        SymbolTable, TypeSymbol, TypeSymbolRef, VarSymbol, VarSymbolRef,
    },
    tokens::{Token, TokenType},
    utils::NodePool,
};

#[derive(Debug, Clone)]
pub struct SemanticMetadata {
    pub expr_type_map: HashMap<ExprRef, TypeSymbolRef>,
    pub type_type_map: HashMap<TypeRef, TypeSymbolRef>,
    pub callable_symbols: HashMap<ExprRef, CallableSymbolRef>,
    pub var_symbols: HashMap<ExprRef, VarSymbolRef>,

    pub types: NodePool<TypeSymbolRef, TypeSymbol>,
    pub vars: NodePool<VarSymbolRef, VarSymbol>,
    pub callables: NodePool<CallableSymbolRef, CallableSymbol>,
}

impl SemanticMetadata {
    pub fn get_expr_type(&self, expr_ref: &ExprRef) -> Option<&TypeSymbol> {
        self.expr_type_map.get(expr_ref).map(|r| self.types.get(*r))
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
        let current_scope = SymbolTable::with_builtins(&mut types, &mut vars, &mut callables);
        let current_scope = SymbolTable::new(1, "global", Some(Box::new(current_scope)));
        Self {
            semantic_metadata: SemanticMetadata {
                types,
                vars,
                callables,
                type_type_map: HashMap::new(),
                expr_type_map: HashMap::new(),
                callable_symbols: HashMap::new(),
                var_symbols: HashMap::new(),
            },
            current_scope: Box::new(current_scope),
            loop_depth: 0,
        }
    }

    pub fn analyze(mut self, tree: &Tree) -> Result<SemanticMetadata, Error> {
        self.visit_stmt(tree.program, tree)?;
        let missing: Vec<&Expr> = tree
            .expr_pool
            .ids()
            .filter(|k| !self.semantic_metadata.expr_type_map.contains_key(k))
            .map(|id| tree.expr_pool.get(id))
            .collect();
        if !(tree.expr_pool.len() == self.semantic_metadata.expr_type_map.len()
            && missing.len() == 0)
        {
            println!(
                "WARNING: not all of the expressions were assigned a type:\n{:#?}",
                missing
            )
        }
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
                        let none_ref = self.semantic_metadata.types.alloc(TypeSymbol::Empty);
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
                let init_state_type = self.semantic_metadata.types.get(init_state_type_ref);
                let end_state_type = self.semantic_metadata.types.get(end_state_type_ref);
                let var_node = self.semantic_metadata.vars.get(
                    self.current_scope
                        .lookup_var(var.lexem(tree.source_code), false)
                        .ok_or(Error::SemanticError {
                            msg: format!("var `{}` is unkown", var.lexem(tree.source_code)),
                            pos: tree.node_pos(NodeRef::StmtRef(node)),
                            error_code: ErrorCode::UnkownVariable,
                        })?,
                );
                let var_type = match var_node {
                    VarSymbol::Var {
                        name: _,
                        type_symbol,
                    } => self.semantic_metadata.types.get(*type_symbol),
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("expected var, got {:?}", var_node),
                            pos: tree.node_pos(NodeRef::StmtRef(node)),
                            error_code: ErrorCode::ExpectedVar,
                        });
                    }
                };
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
            Expr::LiteralReal(_) => Ok(TypeSymbol::Real),
            Expr::LiteralString(_) => Ok(TypeSymbol::String),
            Expr::Var { name } => {
                let var_symbol_ref = self
                    .current_scope
                    .lookup_var(name.lexem(tree.source_code), false)
                    .ok_or(Error::SemanticError {
                        msg: format!("var {} is unkown", name.lexem(tree.source_code)),
                        pos,
                        error_code: ErrorCode::UnkownVariable,
                    })?;
                let var_symbol = self.semantic_metadata.vars.get(var_symbol_ref);
                self.semantic_metadata
                    .var_symbols
                    .insert(node, var_symbol_ref);
                let type_symbol = match var_symbol {
                    VarSymbol::Var {
                        name: _,
                        type_symbol,
                    } => {
                        self.semantic_metadata
                            .expr_type_map
                            .insert(node, *type_symbol);
                        return Ok(*type_symbol);
                    }
                    VarSymbol::Const { value } => match value {
                        ConstValue::Integer(_) => TypeSymbol::Integer,
                        ConstValue::Boolean(_) => TypeSymbol::Boolean,
                        ConstValue::Char(_) => TypeSymbol::Char,
                        ConstValue::String(_) => TypeSymbol::String,
                        ConstValue::Real(_) => TypeSymbol::Real,
                    },
                };
                Ok(type_symbol)
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
                                TypeSymbol::Real | TypeSymbol::Integer,
                                TypeSymbol::Real | TypeSymbol::Integer,
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
                            TypeSymbol::Real | TypeSymbol::Integer,
                            TypeSymbol::Real | TypeSymbol::Integer,
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
                            TypeSymbol::Integer | TypeSymbol::Real,
                            TypeSymbol::Integer | TypeSymbol::Real,
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
        let type_symbol_ref = self.semantic_metadata.types.alloc(type_symbol);
        self.semantic_metadata
            .expr_type_map
            .insert(node, type_symbol_ref);
        Ok(type_symbol_ref)
    }

    fn visit_type(&mut self, node: TypeRef, tree: &Tree) -> Result<TypeSymbolRef, Error> {
        let pos = tree.node_pos(NodeRef::TypeRef(node));
        let type_symbol = match tree.type_pool.get(node) {
            Type::Integer => Ok::<TypeSymbol, Error>(TypeSymbol::Integer),
            Type::Real => Ok(TypeSymbol::Real),
            Type::Boolean => Ok(TypeSymbol::Boolean),
            Type::Char => Ok(TypeSymbol::Char),
            Type::String => Ok(TypeSymbol::String),
            Type::Enum { items } => Ok(TypeSymbol::Enum(
                items
                    .iter()
                    .map(|t| t.lexem(tree.source_code).into())
                    .collect(),
            )),
            Type::Alias(v) => {
                let alias = self
                    .current_scope
                    .lookup_type(v.lexem(tree.source_code), false)
                    .ok_or(Error::SemanticError {
                        msg: format!("type {:?} is unknown", v),
                        pos,
                        error_code: ErrorCode::UnkownType,
                    })?;
                return Ok(alias);
            }
            Type::Array {
                index_type,
                element_type,
            } => {
                let index_type_ref = self.visit_type(*index_type, tree)?;
                let element_type = self.visit_type(*element_type, tree)?;
                Ok(TypeSymbol::Array {
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

                Ok(TypeSymbol::Range(start_val_type_ref))
            }
        }?;
        let type_symbol_ref = self.semantic_metadata.types.alloc(type_symbol);
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
                    Expr::LiteralBool(v) => ConstValue::Boolean(*v),
                    Expr::LiteralReal(v) => ConstValue::Real(*v),
                    Expr::LiteralString(v) => ConstValue::String(v.lexem(tree.source_code).into()),
                    Expr::LiteralChar(c) => ConstValue::Char(*c),
                    _ => unreachable!(),
                };
                let const_symbol = self
                    .semantic_metadata
                    .vars
                    .alloc(crate::symbols::VarSymbol::Const { value: const_type });
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
                let mut params_vec: Vec<(VarSymbolRef, ParamMode)> =
                    Vec::with_capacity(params.len());
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
                    let var_symbol = VarSymbol::Var {
                        name: var_name.lexem(tree.source_code).to_string(),
                        type_symbol: type_symbol_ref,
                    };
                    let var_symbol_ref = self.semantic_metadata.vars.alloc(var_symbol);
                    self.current_scope
                        .define_var(var_name.lexem(tree.source_code), var_symbol_ref);
                    let param_mode = match param.out {
                        true => ParamMode::Ref,
                        false => ParamMode::Var,
                    };
                    params_vec.push((var_symbol_ref, param_mode));
                }
                let return_type = match return_type {
                    Some(return_type_ref) => Some(self.visit_type(*return_type_ref, tree)?),
                    None => None,
                };
                let callable_symbol = CallableSymbol {
                    name: name.lexem(tree.source_code).into(),
                    params: params_vec.clone(),
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
                        type_symbol: return_type_ref,
                    });
                    self.current_scope.define_var("result", return_var);
                    let return_var = self.semantic_metadata.vars.alloc(VarSymbol::Var {
                        name: name.lexem(tree.source_code).into(),
                        type_symbol: return_type_ref,
                    });
                    self.current_scope
                        .define_var(name.lexem(tree.source_code), return_var);
                }
                self.visit_stmt(*block, tree)?;
                let enclosing_scope = self
                    .current_scope
                    .take_enclosing_scope()
                    .expect("there is always enclosing scope here");
                self.current_scope = enclosing_scope;
                Ok(())
            }
            Decl::TypeDecl { var, type_node } => {
                let var_expr = tree.expr_pool.get(*var);
                let var_name = match var_expr {
                    Expr::Var { name } => name,
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("expected var, got {:?}", var_expr),
                            pos: tree.node_pos(NodeRef::ExprRef(*var)),
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
                        pos: tree.node_pos(NodeRef::ExprRef(*var)),
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
                    if !TypeSymbol::eq(&self.semantic_metadata.types, &default_type, &type_symbol) {
                        return Err(Error::SemanticError {
                            msg: format!(
                                "default value should have the type {:?}, but it is {:?}",
                                type_symbol, default_type
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
            .map(|((p, _), expr_type)| {
                let expr_type = self.semantic_metadata.types.get(expr_type);
                let var_symbol = self.semantic_metadata.vars.get(*p);
                let param_type = match var_symbol {
                    VarSymbol::Var {
                        name: _,
                        type_symbol,
                    } => self.semantic_metadata.types.get(*type_symbol),
                    _ => panic!("unreachable"),
                };
                if !assinable(&self.semantic_metadata.types, &param_type, &expr_type) {
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
    match (left, right) {
        (TypeSymbol::Real, TypeSymbol::Integer) => true,
        (TypeSymbol::String, TypeSymbol::Char) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    macro_rules! test_fail {
        ($($name:ident -> [$err:path $(, $other_err:path)* $(,)?],)+) => {
            $(
                #[test]
                fn $name() {
                    let source_path = "test_cases\\semantic_analyzer\\".to_string() + &stringify!($name) + ".pas";
                    let source_code = std::fs::read_to_string(&source_path).expect(&format!("file {source_path} does not exist"));
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

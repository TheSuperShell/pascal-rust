use std::collections::HashMap;

use crate::{
    error::Error,
    parser::{Decl, Expr, ExprRef, Stmt, Tree, Type},
    symbols::{
        CallableSymbol, CallableSymbolRef, ConstValue, SymbolTable, TypeSymbol, TypeSymbolRef,
        VarSymbol, VarSymbolRef,
    },
    tokens::Token,
    utils::NodePool,
};

#[derive(Debug, Clone)]
pub struct SemanticMetadata {
    pub types: NodePool<TypeSymbolRef, TypeSymbol>,
    pub vars: NodePool<VarSymbolRef, VarSymbol>,
    pub callables: NodePool<CallableSymbolRef, CallableSymbol>,

    pub expr_type_map: HashMap<ExprRef, TypeSymbolRef>,
}

#[derive(Debug, Clone)]
pub struct SemanticAnalyzer {
    semantic_metadata: SemanticMetadata,
    current_scope: SymbolTable,
    loop_depth: usize,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            semantic_metadata: SemanticMetadata {
                types: NodePool::new(),
                vars: NodePool::new(),
                callables: NodePool::new(),
                expr_type_map: HashMap::new(),
            },
            current_scope: SymbolTable::new(0, "global", None),
            loop_depth: 0,
        }
    }

    pub fn analyze(mut self, tree: &Tree) -> Result<SemanticMetadata, Error> {
        let compund = tree.stmt_pool.get(tree.program.block.statements);
        if !matches!(compund, Stmt::Compound(_)) {
            return Err(Error::SemanticError {
                msg: "main block should contain statement".to_string(),
                error_code: None,
            });
        }
        tree.program
            .block
            .declarations
            .iter()
            .map(|d| self.visit_declaraction(d, tree))
            .collect::<Result<(), Error>>()?;
        self.visit_stmt(compund, tree)?;
        Ok(self.semantic_metadata)
    }

    fn visit_stmt(&mut self, node: &Stmt, tree: &Tree) -> Result<(), Error> {
        match node {
            Stmt::Assign { left, right } => {
                let left_expr = tree.expr_pool.get(*left);
                let left_type = self.visit_expr(left_expr, tree)?;
                let right_expr = tree.expr_pool.get(*right);
                let right_type = self.visit_expr(right_expr, tree)?;
                if !assinable(&left_type, &right_type) {
                    return Err(Error::SemanticError {
                        msg: "value not assignable".to_string(),
                        error_code: None,
                    });
                }
                Ok(())
            }
            Stmt::Break => {
                if self.loop_depth <= 0 {
                    return Err(Error::SemanticError {
                        msg: "break should be within loop".to_string(),
                        error_code: None,
                    });
                };
                Ok(())
            }
            Stmt::Continue => {
                if self.loop_depth <= 0 {
                    return Err(Error::SemanticError {
                        msg: "continue should be within loop".to_string(),
                        error_code: None,
                    });
                };
                Ok(())
            }
            Stmt::Call { call } => {
                todo!()
            }
            Stmt::Compound(stmts) => stmts
                .iter()
                .map(|s| tree.stmt_pool.get(*s))
                .map(|s| self.visit_stmt(s, tree))
                .collect::<Result<(), Error>>(),
            Stmt::Exit(v) => {
                if let Some(expr_ref) = v {
                    let expr = tree.expr_pool.get(*expr_ref);
                    self.visit_expr(expr, tree)?;
                }
                Ok(())
            }
            Stmt::NoOp => Ok(()),
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => todo!(),
            Stmt::While { cond, body } => todo!(),
            Stmt::For {
                var,
                init,
                end,
                body,
            } => todo!(),
        }
    }
    fn visit_expr(&mut self, node: &Expr, tree: &Tree) -> Result<TypeSymbol, Error> {
        match node {
            Expr::LiteralBool(_) => Ok(TypeSymbol::Boolean),
            Expr::LiteralChar(_) => Ok(TypeSymbol::Char),
            Expr::LiteralInteger(_) => Ok(TypeSymbol::Integer),
            Expr::LiteralReal(_) => Ok(TypeSymbol::Real),
            Expr::LiteralString(_) => Ok(TypeSymbol::String),
            Expr::Var { name } => {
                let var_symbol_ref =
                    self.current_scope
                        .lookup_var(name, false)
                        .ok_or(Error::SemanticError {
                            msg: "unkown var".to_string(),
                            error_code: None,
                        })?;
                let var_symbol = self.semantic_metadata.vars.get(var_symbol_ref);
                match var_symbol {
                    VarSymbol::Var {
                        name: _,
                        type_symbol,
                    } => Ok(self.semantic_metadata.types.get(*type_symbol).clone()),
                    VarSymbol::Const { name: _, value } => match value {
                        ConstValue::Integer(_) => Ok(TypeSymbol::Integer),
                        ConstValue::Boolean(_) => Ok(TypeSymbol::Boolean),
                        ConstValue::Char(_) => Ok(TypeSymbol::Char),
                        ConstValue::String(_) => Ok(TypeSymbol::String),
                        ConstValue::Real(_) => Ok(TypeSymbol::Real),
                    },
                }
            }
            Expr::BinOp { op, left, right } => todo!(),
            Expr::UnaryOp { op, expr: expr_ref } => {
                let expr = tree.expr_pool.get(*expr_ref);
                let expr_type = self.visit_expr(expr, tree)?;
                match (op, expr_type) {
                    (Token::Not, TypeSymbol::Boolean) => Ok(TypeSymbol::Boolean),
                    (Token::Minus | Token::Plus, TypeSymbol::Integer) => Ok(TypeSymbol::Integer),
                    (Token::Minus | Token::Plus, TypeSymbol::Real) => Ok(TypeSymbol::Real),
                    (_, _) => Err(Error::SemanticError {
                        msg: "unary operator is not applicable here".to_string(),
                        error_code: None,
                    }),
                }
            }
            Expr::Call { name, args } => {
                let callable_symbol_ref = self.current_scope.lookup_callable(name, false).ok_or(
                    Error::SemanticError {
                        msg: format!("could not find callable {name}"),
                        error_code: None,
                    },
                )?;
                let callable_symbol = self.semantic_metadata.callables.get(callable_symbol_ref);
                todo!()
            }
            Expr::Index {
                base,
                index_value,
                other_indicies,
            } => todo!(),
        }
    }
    fn visit_type(&mut self, node: &Type, tree: &Tree) -> Result<TypeSymbol, Error> {
        match node {
            Type::Integer => Ok(TypeSymbol::Integer),
            Type::Real => Ok(TypeSymbol::Real),
            Type::Boolean => Ok(TypeSymbol::Boolean),
            Type::Char => Ok(TypeSymbol::Char),
            Type::String => Ok(TypeSymbol::String),
            Type::Enum { items } => Ok(TypeSymbol::Enum(items.clone())),
            Type::Alias(v) => {
                let alias =
                    self.current_scope
                        .lookup_type(v, false)
                        .ok_or(Error::SemanticError {
                            msg: format!("unexpected type {:?}", v),
                            error_code: None,
                        })?;
                let type_symbol = self.semantic_metadata.types.get(alias);
                Ok(type_symbol.clone())
            }
            Type::Array {
                index_type,
                element_type,
            } => {
                let index_type = self.visit_type(tree.type_pool.get(*index_type), tree)?;
                let index_type = self.semantic_metadata.types.alloc(index_type);
                let element_type = self.visit_type(tree.type_pool.get(*element_type), tree)?;
                let element_type = self.semantic_metadata.types.alloc(element_type);
                Ok(TypeSymbol::Array {
                    index_type,
                    value_type: element_type,
                })
            }
            Type::DynamicArray { element_type } => {
                let element_type = self.visit_type(tree.type_pool.get(*element_type), tree)?;
                let element_type = self.semantic_metadata.types.alloc(element_type);
                Ok(TypeSymbol::DynamicArray(element_type))
            }
            Type::Range { start_val, end_val } => {
                let start_val = tree.expr_pool.get(*start_val);
                let start_val_type = self.visit_expr(start_val, tree)?;

                let end_val = tree.expr_pool.get(*end_val);
                let end_val_type = self.visit_expr(end_val, tree)?;

                if start_val_type != end_val_type {
                    return Err(Error::SemanticError {
                        msg: "range limits should be of the same type".to_string(),
                        error_code: None,
                    });
                }

                // let start_val_type_ref = self.semantic_metadata.types.alloc(start_val_type);
                Ok(start_val_type)
            }
        }
    }

    fn visit_declaraction(&mut self, decl: &Decl, tree: &Tree) -> Result<(), Error> {
        match decl {
            Decl::ConstDecl { var, literal } => {
                let var_expr = tree.expr_pool.get(*var);
                let var_name = match var_expr {
                    Expr::Var { name } => name,
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("expected variable, found {:?}", var_expr),
                            error_code: None,
                        });
                    }
                };
                let literal = tree.expr_pool.get(*literal);
                let const_type = match literal {
                    Expr::LiteralInteger(v) => ConstValue::Integer(*v),
                    Expr::LiteralBool(v) => ConstValue::Boolean(*v),
                    Expr::LiteralReal(v) => ConstValue::Real(*v),
                    Expr::LiteralString(v) => ConstValue::String(v.clone()),
                    Expr::LiteralChar(c) => ConstValue::Char(*c),
                    _ => {
                        return Err(Error::SemanticError {
                            msg: format!("expected literal for const, got {:?}", literal),
                            error_code: None,
                        });
                    }
                };
                let const_symbol =
                    self.semantic_metadata
                        .vars
                        .alloc(crate::symbols::VarSymbol::Const {
                            name: var_name.clone(),
                            value: const_type,
                        });
                self.current_scope.define_var(var_name, const_symbol);
                Ok(())
            }
            Decl::Function {
                name,
                block,
                params,
                return_type,
            } => {
                todo!()
            }
            Decl::Procedure {
                name,
                block,
                params,
            } => {
                todo!()
            }
            Decl::TypeDecl { var, type_node } => {
                let var = tree.expr_pool.get(*var);
                let var_name = match var {
                    Expr::Var { name } => name,
                    _ => {
                        return Err(Error::SemanticError {
                            msg: "expected var, got".to_string(),
                            error_code: None,
                        });
                    }
                };
                let type_node = tree.type_pool.get(*type_node);
                let type_symbol = self.visit_type(type_node, tree)?;
                let type_symbol_ref = self.semantic_metadata.types.alloc(type_symbol);
                self.current_scope.define_type(var_name, type_symbol_ref);
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
                    _ => {
                        return Err(Error::SemanticError {
                            msg: "expected var".to_string(),
                            error_code: None,
                        });
                    }
                };
                let type_node = tree.type_pool.get(*type_node);
                let type_symbol = self.visit_type(type_node, tree)?;

                if let Some(expr) = default_value {
                    let default_value = tree.expr_pool.get(*expr);
                    let default_type = self.visit_expr(default_value, tree)?;
                    if default_type != type_symbol {
                        return Err(Error::SemanticError {
                            msg: "default value should have the correct type".to_string(),
                            error_code: None,
                        });
                    }
                }

                let type_symbol_ref = self.semantic_metadata.types.alloc(type_symbol);
                let var_symbol =
                    self.semantic_metadata
                        .vars
                        .alloc(crate::symbols::VarSymbol::Var {
                            name: var_name.clone(),
                            type_symbol: type_symbol_ref,
                        });
                self.current_scope.define_var(var_name, var_symbol);
                self.semantic_metadata
                    .expr_type_map
                    .insert(*var_ref, type_symbol_ref);
                Ok(())
            }
        }
    }
}

fn assinable(left: &TypeSymbol, right: &TypeSymbol) -> bool {
    if left == right {
        return true;
    }
    match (left, right) {
        (TypeSymbol::Real, TypeSymbol::Integer) => true,
        (TypeSymbol::String, TypeSymbol::Char) => true,
        _ => false,
    }
}

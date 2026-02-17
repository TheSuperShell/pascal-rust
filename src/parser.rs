use std::fmt::Display;

use crate::{
    error::{Error, ErrorCode},
    lexer::Lexer,
    tokens::{Token, TokenType},
    utils::{NodePoolWithSpan, Pos, Span, define_ref},
};

define_ref!(ExprRef);
define_ref!(StmtRef);
define_ref!(TypeRef);

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Index {
        other_indicies: Vec<ExprRef>,
        base: ExprRef,
        index_value: ExprRef,
    },
    BinOp {
        op: TokenType,
        left: ExprRef,
        right: ExprRef,
    },
    UnaryOp {
        op: TokenType,
        expr: ExprRef,
    },
    Call {
        name: Token,
        args: Vec<ExprRef>,
    },
    Var {
        name: Token,
    },
    LiteralInteger(i32),
    LiteralReal(f32),
    LiteralBool(bool),
    LiteralChar(char),
    LiteralString(Token),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    For {
        var: Token,
        init: ExprRef,
        end: ExprRef,
        body: StmtRef,
    },
    If {
        elifs: Vec<Condition>,
        cond: Condition,
        else_statement: Option<StmtRef>,
    },
    Program {
        name: Token,
        block: StmtRef,
    },
    Block {
        declarations: Vec<Decl>,
        statements: StmtRef,
    },
    Assign {
        left: ExprRef,
        right: ExprRef,
    },
    While {
        cond: ExprRef,
        body: StmtRef,
    },
    Exit(Option<ExprRef>),
    Compound(Vec<StmtRef>),
    Call {
        call: ExprRef,
    },
    Break,
    Continue,
    NoOp,
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub cond: ExprRef,
    pub expr: StmtRef,
}

#[derive(Debug, Clone)]
pub enum Decl {
    VarDecl {
        default_value: Option<ExprRef>,
        var: ExprRef,
        type_node: TypeRef,
    },
    TypeDecl {
        var: TypeRef,
        type_node: TypeRef,
    },
    ConstDecl {
        var: ExprRef,
        literal: ExprRef,
    },
    Callable {
        params: Vec<Param>,
        name: Token,
        return_type: Option<TypeRef>,
        block: StmtRef,
    },
}

#[derive(Debug, Clone)]
pub struct Param {
    pub var: ExprRef,
    pub type_node: TypeRef,
    pub out: bool,
}

#[derive(Debug, Clone)]
pub enum Type {
    Integer,
    Boolean,
    String,
    Char,
    Real,
    Alias(Token),
    Array {
        index_type: TypeRef,
        element_type: TypeRef,
    },
    DynamicArray {
        element_type: TypeRef,
    },
    Range {
        start_val: ExprRef,
        end_val: ExprRef,
    },
    Enum {
        items: Vec<Token>,
    },
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    expr_pool: NodePoolWithSpan<ExprRef, Expr>,
    stmt_pool: NodePoolWithSpan<StmtRef, Stmt>,
    type_pool: NodePoolWithSpan<TypeRef, Type>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Result<Self, Error> {
        let token = lexer.next()?;
        Ok(Self {
            lexer,
            current_token: token,
            expr_pool: NodePoolWithSpan::new(),
            stmt_pool: NodePoolWithSpan::new(),
            type_pool: NodePoolWithSpan::new(),
        })
    }

    fn eat(&mut self, expected: TokenType) -> Result<Token, Error> {
        let token = self.current_token;
        if token.token_type() != &expected {
            return Err(Error::ParserError {
                msg: format!(
                    "expected token {:?}, got {:?}",
                    expected, self.current_token
                ),
                pos: token.pos(),
                error_code: ErrorCode::UnexpectedToken,
            });
        }
        self.current_token = self.lexer.next()?;
        Ok(token)
    }

    /// program:
    /// Program Id Semi block Dot
    fn program(&mut self) -> Result<StmtRef, Error> {
        let start_token = self.eat(TokenType::Program)?;
        let var = self.id_str()?;
        self.eat(TokenType::Semi)?;
        let block = self.block()?;
        let end_token = self.eat(TokenType::Dot)?;
        Ok(self.stmt_pool.alloc(
            Stmt::Program {
                name: var,
                block: block,
            },
            start_token.span().total(&end_token.span()),
        ))
    }

    /// id:
    /// ID
    fn id(&mut self) -> Result<ExprRef, Error> {
        if let TokenType::Id = &self.current_token.token_type() {
            let id = self.current_token;
            self.eat(TokenType::Id)?;
            return Ok(self.expr_pool.alloc(Expr::Var { name: id }, id.span()));
        }
        Err(Error::ParserError {
            msg: format!("expected id, got {:?}", self.current_token),
            pos: self.current_token.pos(),
            error_code: ErrorCode::UnexpectedToken,
        })
    }

    fn id_str(&mut self) -> Result<Token, Error> {
        if let TokenType::Id = &self.current_token.token_type() {
            let id = self.current_token;
            self.current_token = self.lexer.next()?;
            return Ok(id);
        }
        Err(Error::ParserError {
            msg: format!("expected id, got {:?}", self.current_token),
            pos: self.current_token.pos(),
            error_code: ErrorCode::UnexpectedToken,
        })
    }

    /// block:
    /// declarations compound_statement
    fn block(&mut self) -> Result<StmtRef, Error> {
        let (decls, span) = self.declarations()?;
        let compound = self.compound_statement()?;
        let comp_span = self.stmt_pool.span(compound);
        Ok(self.stmt_pool.alloc(
            Stmt::Block {
                declarations: decls,
                statements: compound,
            },
            span.total(&comp_span),
        ))
    }

    /// declarations:
    /// (
    /// Const (const_declaration Semi)+ |
    /// Type (type_declaration Semi)+ |
    /// Var (var_declataion Semi)+ |
    /// procedure_declaration |
    /// function_declaration
    /// )*
    fn declarations(&mut self) -> Result<(Vec<Decl>, Span), Error> {
        let start_span = self.current_token.span();
        let mut decls = Vec::new();
        while matches!(
            self.current_token.token_type(),
            TokenType::Const
                | TokenType::Type
                | TokenType::Var
                | TokenType::Function
                | TokenType::Procedure
        ) {
            match self.current_token.token_type() {
                TokenType::Const => {
                    self.eat(TokenType::Const)?;
                    decls.extend(self.const_declaration()?);
                    self.eat(TokenType::Semi)?;
                    while let TokenType::Id = self.current_token.token_type() {
                        decls.extend(self.const_declaration()?);
                        self.eat(TokenType::Semi)?;
                    }
                }
                TokenType::Type => {
                    self.eat(TokenType::Type)?;
                    decls.extend(self.type_declaration()?);
                    self.eat(TokenType::Semi)?;
                    while let TokenType::Id = self.current_token.token_type() {
                        decls.extend(self.type_declaration()?);
                        self.eat(TokenType::Semi)?;
                    }
                }
                TokenType::Var => {
                    self.eat(TokenType::Var)?;
                    decls.extend(self.var_declaration()?);
                    self.eat(TokenType::Semi)?;
                    while let TokenType::Id = self.current_token.token_type() {
                        decls.extend(self.var_declaration()?);
                        self.eat(TokenType::Semi)?;
                    }
                }
                TokenType::Procedure => decls.push(self.procedure_declaration()?),
                TokenType::Function => decls.push(self.function_declaration()?),
                _ => unreachable!(),
            }
        }
        Ok((decls, start_span))
    }

    /// function_declaration:
    /// Function id (LParen formal_parameter_list RParen)?
    /// Colon type_spec Semi block Semi
    fn function_declaration(&mut self) -> Result<Decl, Error> {
        self.eat(TokenType::Function)?;
        let func_name = self.id_str()?;
        let params = match self.current_token.token_type() {
            TokenType::LParen => {
                self.eat(TokenType::LParen)?;
                let params = self.formal_parameter_list()?;
                self.eat(TokenType::RParen)?;
                params
            }
            _ => Vec::with_capacity(0),
        };
        self.eat(TokenType::Colon)?;
        let return_type = self.type_spec()?;
        self.eat(TokenType::Semi)?;
        let block = self.block()?;
        self.eat(TokenType::Semi)?;
        Ok(Decl::Callable {
            name: func_name,
            block,
            params,
            return_type: Some(return_type),
        })
    }

    /// var_declaration:
    /// Var id (Comma id)* Colon type_spec (Equal literal)?
    fn var_declaration(&mut self) -> Result<Vec<Decl>, Error> {
        let mut vars = vec![self.id()?];
        while let TokenType::Comma = self.current_token.token_type() {
            self.eat(TokenType::Comma)?;
            vars.push(self.id()?);
        }
        self.eat(TokenType::Colon)?;
        let type_spec = self.type_spec()?;
        let default_value = match self.current_token.token_type() {
            TokenType::Equal => {
                self.eat(TokenType::Equal)?;
                Some(self.literal()?)
            }
            _ => None,
        };
        Ok(vars
            .iter()
            .map(|n| Decl::VarDecl {
                var: *n,
                type_node: type_spec,
                default_value,
            })
            .collect())
    }

    /// procedure_declaration:
    /// Procedure id (LParen formal_parameter_list RParen)? Semi block Semi
    fn procedure_declaration(&mut self) -> Result<Decl, Error> {
        self.eat(TokenType::Procedure)?;
        let proc_name = self.id_str()?;
        let params = match self.current_token.token_type() {
            TokenType::LParen => {
                self.eat(TokenType::LParen)?;
                let params = self.formal_parameter_list()?;
                self.eat(TokenType::RParen)?;
                params
            }
            _ => Vec::with_capacity(0),
        };
        let block = self.block()?;
        self.eat(TokenType::Semi)?;
        Ok(Decl::Callable {
            name: proc_name,
            block,
            params,
            return_type: None,
        })
    }

    /// formal_parameter_list:
    /// formal_parameters (Semi formal_parameter_list)?
    fn formal_parameter_list(&mut self) -> Result<Vec<Param>, Error> {
        let mut params = self.formal_parameters()?;
        if let TokenType::Semi = self.current_token.token_type() {
            self.eat(TokenType::Semi)?;
            params.extend(self.formal_parameter_list()?);
        };
        Ok(params)
    }

    /// formal_parameters:
    /// Out? id (Comma Out? id)* Colon type_spec
    fn formal_parameters(&mut self) -> Result<Vec<Param>, Error> {
        let out = match self.current_token.token_type() {
            TokenType::Out => {
                self.eat(TokenType::Out)?;
                true
            }
            _ => false,
        };
        let mut names = vec![(out, self.id()?)];
        while let TokenType::Comma = self.current_token.token_type() {
            self.eat(TokenType::Comma)?;
            let out = match self.current_token.token_type() {
                TokenType::Out => {
                    self.eat(TokenType::Out)?;
                    true
                }
                _ => false,
            };
            names.push((out, self.id()?));
        }
        self.eat(TokenType::Colon)?;
        let param_type = self.type_spec()?;
        Ok(names
            .iter()
            .map(|(o, n)| Param {
                var: *n,
                out: *o,
                type_node: param_type,
            })
            .collect())
    }

    /// const_declaration:
    /// Const id (Comma id)* Eq literal
    fn const_declaration(&mut self) -> Result<Vec<Decl>, Error> {
        let mut names = vec![self.id()?];
        while let TokenType::Comma = self.current_token.token_type() {
            self.eat(TokenType::Comma)?;
            names.push(self.id()?);
        }
        self.eat(TokenType::Equal)?;
        let literal = self.literal()?;
        Ok(names
            .iter()
            .map(|n| Decl::ConstDecl { var: *n, literal })
            .collect())
    }

    /// type_declaration:
    /// Type id (Comma id)* Equal type_spec
    fn type_declaration(&mut self) -> Result<Vec<Decl>, Error> {
        let mut names = vec![self.id_str()?];
        while let TokenType::Comma = self.current_token.token_type() {
            self.eat(TokenType::Comma)?;
            names.push(self.id_str()?);
        }
        self.eat(TokenType::Equal)?;
        let type_decl = self.type_spec()?;
        Ok(names
            .iter()
            .map(|&t| self.type_pool.alloc(Type::Alias(t), t.span()))
            .map(|t| Decl::TypeDecl {
                var: t,
                type_node: type_decl,
            })
            .collect())
    }

    /// type_spec:
    /// Integer | Real | Boolean | String | Char |
    /// enum_spec |
    /// array_spec |
    /// range_spec
    fn type_spec(&mut self) -> Result<TypeRef, Error> {
        let token = self.current_token.token_type();
        match token {
            TokenType::Integer => {
                let token = self.current_token;
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Integer, token.span()))
            }
            TokenType::Real => {
                let token = self.current_token;
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Real, token.span()))
            }
            TokenType::Boolean => {
                let token = self.current_token;
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Boolean, token.span()))
            }
            TokenType::String => {
                let token = self.current_token;
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::String, token.span()))
            }
            TokenType::Char => {
                let token = self.current_token;
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Char, token.span()))
            }
            TokenType::LParen => self.enum_spec(),
            TokenType::Array => self.array_spec(),
            _ => match self.lexer.peek() {
                Some('.') => self.range_spec(),
                _ => {
                    let token = self.current_token;
                    self.eat(TokenType::Id)?;
                    Ok(self.type_pool.alloc(Type::Alias(token), token.span()))
                }
            },
        }
    }

    /// enum_spec:
    /// LParan id (Comma id)* RParan
    fn enum_spec(&mut self) -> Result<TypeRef, Error> {
        let start_span = self.eat(TokenType::LParen)?.span();
        let mut items = vec![self.id_str()?];
        while let TokenType::Comma = self.current_token.token_type() {
            self.eat(TokenType::Comma)?;
            items.push(self.id_str()?);
        }
        let right_span = self.eat(TokenType::RParen)?.span();
        Ok(self
            .type_pool
            .alloc(Type::Enum { items }, start_span.total(&right_span)))
    }

    /// array_spec:
    /// Array (LBrack (id|range_spec) RBrack)? Of type_spec
    fn array_spec(&mut self) -> Result<TypeRef, Error> {
        let start_span = self.eat(TokenType::Array)?.span();
        if let TokenType::LBracket = self.current_token.token_type() {
            self.eat(TokenType::LBracket)?;
            let index_type = match self.lexer.peek() {
                Some('.') => self.range_spec()?,
                _ => {
                    let token = self.current_token;
                    self.current_token = self.lexer.next()?;
                    self.type_pool.alloc(Type::Alias(token), token.span())
                }
            };
            self.eat(TokenType::RBracket)?;
            self.eat(TokenType::Of)?;
            let element_type = self.type_spec()?;
            let element_span = self.type_pool.span(element_type);
            return Ok(self.type_pool.alloc(
                Type::Array {
                    index_type,
                    element_type,
                },
                start_span.total(&element_span),
            ));
        };
        self.eat(TokenType::Of)?;
        let element_type = self.type_spec()?;
        let element_span = self.type_pool.span(element_type);
        Ok(self.type_pool.alloc(
            Type::DynamicArray { element_type },
            start_span.total(&element_span),
        ))
    }

    /// range_spec:
    /// (id | literal) Dot Dot (id | literal)
    fn range_spec(&mut self) -> Result<TypeRef, Error> {
        let start = match self.current_token.token_type() {
            TokenType::Id => self.id()?,
            _ => self.literal()?,
        };
        let start_span = self.expr_pool.span(start);
        self.eat(TokenType::Dot)?;
        self.eat(TokenType::Dot)?;
        let end = match self.current_token.token_type() {
            TokenType::Id => self.id()?,
            _ => self.literal()?,
        };
        let end_span = self.expr_pool.span(end);
        Ok(self.type_pool.alloc(
            Type::Range {
                start_val: start,
                end_val: end,
            },
            start_span.total(&end_span),
        ))
    }

    /// compound_statement:
    /// Begin statement_list End
    fn compound_statement(&mut self) -> Result<StmtRef, Error> {
        let start_span = self.eat(TokenType::Begin)?.span();
        let statement_list = self.statement_list()?;
        let end_span = self.eat(TokenType::End)?.span();
        Ok(self
            .stmt_pool
            .alloc(Stmt::Compound(statement_list), start_span.total(&end_span)))
    }

    /// statement_list:
    /// statement (Semi statement)*
    fn statement_list(&mut self) -> Result<Vec<StmtRef>, Error> {
        let mut statements = vec![self.statement()?];
        while let TokenType::Semi = self.current_token.token_type() {
            self.eat(TokenType::Semi)?;
            statements.push(self.statement()?);
        }
        Ok(statements)
    }

    /// statement:
    /// Continue |
    /// Break |
    /// compound_statement |
    /// call_statement |
    /// assignment_statement |
    /// if_statement |
    /// while_statement |
    /// for_statement |
    /// exit_statement |
    /// NoOp
    fn statement(&mut self) -> Result<StmtRef, Error> {
        match self.current_token.token_type() {
            TokenType::Continue => {
                let span = self.eat(TokenType::Continue)?.span();
                Ok(self.stmt_pool.alloc(Stmt::Continue, span))
            }
            TokenType::Break => {
                let span = self.eat(TokenType::Break)?.span();
                Ok(self.stmt_pool.alloc(Stmt::Break, span))
            }
            TokenType::Begin => self.compound_statement(),
            TokenType::Id => match self.lexer.current_char() {
                Some('(') => self.call_statement(),
                _ => self.assignment_statement(),
            },
            TokenType::If => self.if_statement(),
            TokenType::While => self.while_statement(),
            TokenType::For => self.for_statement(),
            TokenType::Exit => self.exit_statement(),
            _ => Ok(self
                .stmt_pool
                .alloc(Stmt::NoOp, Span::zero(self.current_token.span().start()))),
        }
    }

    /// exit_statement:
    /// Exit (LParan exprt RParan)?
    fn exit_statement(&mut self) -> Result<StmtRef, Error> {
        let mut span = self.eat(TokenType::Exit)?.span();
        let mut expr = None;
        if let TokenType::LParen = self.current_token.token_type() {
            self.eat(TokenType::LParen)?;
            expr = Some(self.expr()?);
            span = span.total(&self.eat(TokenType::RParen)?.span());
        };
        Ok(self.stmt_pool.alloc(Stmt::Exit(expr), span))
    }

    /// for_statement:
    /// For id Assign expr To expr Do statement
    fn for_statement(&mut self) -> Result<StmtRef, Error> {
        let start_span = self.eat(TokenType::For)?.span();
        let var = self.id_str()?;
        self.eat(TokenType::Assign)?;
        let init_state = self.expr()?;
        self.eat(TokenType::To)?;
        let end_state = self.expr()?;
        self.eat(TokenType::Do)?;
        let expr = self.statement()?;
        let end_span = self.stmt_pool.span(expr);
        Ok(self.stmt_pool.alloc(
            Stmt::For {
                var,
                init: init_state,
                end: end_state,
                body: expr,
            },
            start_span.total(&end_span),
        ))
    }

    /// while_statement:
    /// While expr Do statement
    fn while_statement(&mut self) -> Result<StmtRef, Error> {
        let span = self.eat(TokenType::While)?.span();
        let cond = self.expr()?;
        self.eat(TokenType::Do)?;
        let body = self.statement()?;
        let span = span.total(&self.stmt_pool.span(body));
        Ok(self.stmt_pool.alloc(Stmt::While { cond, body }, span))
    }

    /// if_statement:
    /// If condition
    /// (Else If condition)*
    /// (Else statement)?
    fn if_statement(&mut self) -> Result<StmtRef, Error> {
        let mut span = self.eat(TokenType::If)?.span();
        let (main_cond, main_span) = self.condition()?;
        span = span.total(&main_span);
        let mut other_conditions = Vec::new();
        let mut last_conditition = None;
        while let TokenType::Else = self.current_token.token_type() {
            self.eat(TokenType::Else)?;
            match self.current_token.token_type() {
                TokenType::If => {
                    self.eat(TokenType::If)?;
                    let (cond, new_span) = self.condition()?;
                    span = span.total(&new_span);
                    other_conditions.push(cond);
                }
                _ => {
                    let stmt = self.statement()?;
                    span = span.total(&self.stmt_pool.span(stmt));
                    last_conditition = Some(stmt);
                    break;
                }
            }
        }
        Ok(self.stmt_pool.alloc(
            Stmt::If {
                cond: main_cond,
                elifs: other_conditions,
                else_statement: last_conditition,
            },
            span,
        ))
    }

    /// condition:
    /// expr Then statement
    fn condition(&mut self) -> Result<(Condition, Span), Error> {
        let cond = self.expr()?;
        let span = self.expr_pool.span(cond);
        self.eat(TokenType::Then)?;
        let expr = self.statement()?;
        let span = span.total(&self.stmt_pool.span(expr));
        Ok((Condition { cond, expr }, span))
    }

    /// assignment_statement:
    /// id (LBracket expr RBracket)? Assign expr
    fn assignment_statement(&mut self) -> Result<StmtRef, Error> {
        let var = match self.lexer.current_char() {
            Some('[') => self.index_of_statement()?,
            _ => self.id()?,
        };
        let span = self.expr_pool.span(var);
        self.eat(TokenType::Assign)?;
        let expr = self.expr()?;
        let span = span.total(&self.expr_pool.span(expr));
        Ok(self.stmt_pool.alloc(
            Stmt::Assign {
                left: var,
                right: expr,
            },
            span,
        ))
    }

    /// expr:
    /// bool_expr (OR bool_expr)*
    fn expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.bool_expr()?;
        while let TokenType::Or = self.current_token.token_type() {
            self.eat(TokenType::Or)?;
            let right = self.bool_expr()?;
            let span = self.expr_pool.span(node).total(&self.expr_pool.span(right));
            node = self.expr_pool.alloc(
                Expr::BinOp {
                    op: TokenType::Or,
                    left: node,
                    right,
                },
                span,
            );
        }
        Ok(node)
    }

    /// bool_expr:
    /// compare_expr (AND compare_expr)*
    fn bool_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.compare_expr()?;
        while let TokenType::And = self.current_token.token_type() {
            self.eat(TokenType::And)?;
            let right = self.compare_expr()?;
            let span = self.expr_pool.span(node).total(&self.expr_pool.span(right));
            node = self.expr_pool.alloc(
                Expr::BinOp {
                    op: TokenType::And,
                    left: node,
                    right,
                },
                span,
            );
        }
        Ok(node)
    }

    /// compare_expr:
    /// add_expr (compare_token add_expr)*
    fn compare_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.add_expr()?;
        while self.current_token.token_type().is_compare_operator() {
            let token = *self.current_token.token_type();
            self.current_token = self.lexer.next()?;
            let right = self.add_expr()?;
            let span = self.expr_pool.span(node).total(&self.expr_pool.span(right));
            node = self.expr_pool.alloc(
                Expr::BinOp {
                    op: token,
                    left: node,
                    right,
                },
                span,
            );
        }
        Ok(node)
    }

    /// add_expr
    /// mult_expr ((Minus | Plus) mult_expr)*
    fn add_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.mult_expr()?;
        while matches!(
            self.current_token.token_type(),
            TokenType::Plus | TokenType::Minus
        ) {
            let token = *self.current_token.token_type();
            self.current_token = self.lexer.next()?;
            let right = self.mult_expr()?;
            let span = self.expr_pool.span(node).total(&self.expr_pool.span(right));
            node = self.expr_pool.alloc(
                Expr::BinOp {
                    op: token,
                    left: node,
                    right,
                },
                span,
            );
        }
        Ok(node)
    }

    /// mult_expr:
    /// factor ((Mult | Div | RealDiv) factor)*
    fn mult_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.factor()?;
        while matches!(
            self.current_token.token_type(),
            TokenType::Mul | TokenType::RealDiv | TokenType::IntegerDiv
        ) {
            let token = *self.current_token.token_type();
            self.current_token = self.lexer.next()?;
            let right = self.factor()?;
            let span = self.expr_pool.span(node).total(&self.expr_pool.span(right));
            node = self.expr_pool.alloc(
                Expr::BinOp {
                    op: token,
                    left: node,
                    right,
                },
                span,
            );
        }
        Ok(node)
    }

    /// factor:
    /// (Plus | Minus) factor |
    /// Not compare_statement |
    /// literal |
    /// OpenParan expr CloseParan |
    /// call_expr |
    /// index_of_statement |
    /// variable
    fn factor(&mut self) -> Result<ExprRef, Error> {
        match self.current_token.token_type() {
            TokenType::Plus | TokenType::Minus => {
                let token = self.current_token;
                self.current_token = self.lexer.next()?;
                let factor = self.factor()?;
                let span = token.span().total(&self.expr_pool.span(factor));
                return Ok(self.expr_pool.alloc(
                    Expr::UnaryOp {
                        op: *token.token_type(),
                        expr: factor,
                    },
                    span,
                ));
            }
            TokenType::Not => {
                let span = self.eat(TokenType::Not)?.span();
                let expr = self.compare_expr()?;
                let span = span.total(&self.expr_pool.span(expr));
                return Ok(self.expr_pool.alloc(
                    Expr::UnaryOp {
                        op: TokenType::Not,
                        expr,
                    },
                    span,
                ));
            }
            TokenType::IntegerConst(_)
            | TokenType::RealConst(_)
            | TokenType::BooleanConst(_)
            | TokenType::StringConst
            | TokenType::CharConst(_) => self.literal(),
            TokenType::LParen => {
                self.eat(TokenType::LParen)?;
                let expr = self.expr();
                self.eat(TokenType::RParen)?;
                return expr;
            }
            TokenType::Id => match self.lexer.current_char() {
                Some('(') => self.call_expr(),
                Some('[') => self.index_of_statement(),
                _ => self.id(),
            },
            _ => Err(Error::ParserError {
                msg: format!("unexpected factor {:?}", self.current_token),
                pos: self.current_token.pos(),
                error_code: ErrorCode::UnexpectedToken,
            }),
        }
    }

    /// call_expr:
    /// Id LParan expr (Comma expr)* RParan
    fn call_expr(&mut self) -> Result<ExprRef, Error> {
        let proc_name = self.id_str()?;
        self.eat(TokenType::LParen)?;
        let mut params = Vec::new();
        if self.current_token.token_type() != &TokenType::RParen {
            params.push(self.expr()?);
        }
        while let TokenType::Comma = self.current_token.token_type() {
            self.eat(TokenType::Comma)?;
            params.push(self.expr()?);
        }
        let span = proc_name.span().total(&self.eat(TokenType::RParen)?.span());
        Ok(self.expr_pool.alloc(
            Expr::Call {
                name: proc_name,
                args: params,
            },
            span,
        ))
    }

    /// call_statement:
    /// Id LParan expr (Comma expr)* RParan
    fn call_statement(&mut self) -> Result<StmtRef, Error> {
        let call = self.call_expr()?;
        let span = self.expr_pool.span(call);
        Ok(self.stmt_pool.alloc(Stmt::Call { call }, span))
    }

    /// index_of_statement:
    /// id LBracket expr (Comma expr)* RBracket
    fn index_of_statement(&mut self) -> Result<ExprRef, Error> {
        let var_node = self.id()?;
        let span = self.expr_pool.span(var_node);
        self.eat(TokenType::LBracket)?;
        let expr = self.expr()?;
        let mut other_indicies = Vec::new();
        while let TokenType::Comma = *self.current_token.token_type() {
            self.eat(TokenType::Comma)?;
            other_indicies.push(self.expr()?);
        }
        let span = span.total(&self.eat(TokenType::RBracket)?.span());
        Ok(self.expr_pool.alloc(
            Expr::Index {
                base: var_node,
                index_value: expr,
                other_indicies,
            },
            span,
        ))
    }

    fn literal(&mut self) -> Result<ExprRef, Error> {
        let token = self.current_token;
        self.current_token = self.lexer.next()?;
        match *token.token_type() {
            TokenType::IntegerConst(v) => {
                Ok(self.expr_pool.alloc(Expr::LiteralInteger(v), token.span()))
            }
            TokenType::RealConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralReal(v), token.span())),
            TokenType::StringConst => Ok(self
                .expr_pool
                .alloc(Expr::LiteralString(token), token.span())),
            TokenType::CharConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralChar(v), token.span())),
            TokenType::BooleanConst(v) => {
                Ok(self.expr_pool.alloc(Expr::LiteralBool(v), token.span()))
            }
            _ => Err(Error::ParserError {
                msg: format!("unkown literal {:?}", token),
                pos: token.pos(),
                error_code: ErrorCode::UnkownLiteral,
            }),
        }
    }

    pub fn parse(mut self) -> Result<Tree<'a>, Error> {
        let program = self.program()?;
        Ok(Tree {
            source_code: self.lexer.source_code(),
            program,
            expr_pool: self.expr_pool,
            stmt_pool: self.stmt_pool,
            type_pool: self.type_pool,
        })
    }
}

#[derive(Debug, Clone)]
pub enum NodeRef {
    ExprRef(ExprRef),
    StmtRef(StmtRef),
    TypeRef(TypeRef),
}
#[derive(Debug, Clone)]
pub struct Tree<'a> {
    pub source_code: &'a str,
    pub program: StmtRef,
    pub expr_pool: NodePoolWithSpan<ExprRef, Expr>,
    pub stmt_pool: NodePoolWithSpan<StmtRef, Stmt>,
    pub type_pool: NodePoolWithSpan<TypeRef, Type>,
}

impl<'a> Tree<'a> {
    pub fn node_pos(&self, node_ref: NodeRef) -> Pos {
        match node_ref {
            NodeRef::ExprRef(r) => self.expr_pool.span(r).pos(self.source_code),
            NodeRef::StmtRef(r) => self.stmt_pool.span(r).pos(self.source_code),
            NodeRef::TypeRef(r) => self.type_pool.span(r).pos(self.source_code),
        }
    }

    fn visit_declaraction(&self, decl: &Decl, level: usize) -> String {
        let indent = " ".repeat(2 * level);
        match decl {
            Decl::ConstDecl { var, literal } => {
                format!(
                    "{indent}Const\n{}\n{}",
                    self.visit_expr(*var, level + 1),
                    self.visit_expr(*literal, level + 2)
                )
            }
            Decl::TypeDecl { var, type_node } => {
                format!(
                    "{indent}Type\n{}\n{}",
                    self.visit_type(*var, level + 1),
                    self.visit_type(*type_node, level + 2)
                )
            }
            Decl::VarDecl {
                var,
                type_node,
                default_value,
            } => {
                let mut result = format!(
                    "{indent}Var\n{}\n{}",
                    self.visit_expr(*var, level + 1),
                    self.visit_type(*type_node, level + 2)
                );
                if let Some(v) = default_value {
                    result.push_str("\n");
                    result.push_str(&format!(
                        "{indent}  Default\n{}",
                        self.visit_expr(*v, level + 2)
                    ));
                };
                result
            }
            Decl::Callable {
                name,
                block,
                params,
                return_type,
            } => {
                let mut result = format!("{indent}Callable({})", name.lexem(self.source_code));
                if let Some(r) = return_type {
                    result.push_str(&format!("\n{}", self.visit_type(*r, level + 1)));
                }
                let params_str = params
                    .iter()
                    .map(|p| {
                        format!(
                            "{}{}\n{}",
                            self.visit_expr(p.var, level + 1),
                            match p.out {
                                true => " Out",
                                false => "",
                            },
                            self.visit_type(p.type_node, level + 2)
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n");
                if !params_str.is_empty() {
                    result.push_str("\n");
                    result.push_str(&params_str);
                }
                result.push_str(&self.visit_stmt(*block, level));
                result
            }
        }
    }
    fn visit_type(&self, id: TypeRef, level: usize) -> String {
        let indent = " ".repeat(2 * level);
        match self.type_pool.get(id) {
            Type::Integer => format!("{indent}Type(Integer)"),
            Type::Real => format!("{indent}Type(Real)"),
            Type::Boolean => format!("{indent}Type(Boolean)"),
            Type::Char => format!("{indent}Type(Char)"),
            Type::String => format!("{indent}Type(String)"),
            Type::Range { start_val, end_val } => {
                format!(
                    "{indent}Type(Range)\n{}\n{}",
                    self.visit_expr(*start_val, level + 1),
                    self.visit_expr(*end_val, level + 1)
                )
            }
            Type::Alias(v) => format!("{indent}TypeAlias({})", v.lexem(self.source_code)),
            Type::Enum { items } => format!(
                "{indent}Type(Enum)\n{}",
                items
                    .iter()
                    .map(|i| { format!("{indent}  {}", i.lexem(self.source_code)) })
                    .collect::<Vec<String>>()
                    .join("\n")
            ),
            Type::Array {
                index_type,
                element_type,
            } => {
                format!(
                    "{indent}Type(Array)\n{}\n{}",
                    self.visit_type(*index_type, level + 1),
                    self.visit_type(*element_type, level + 1)
                )
            }
            Type::DynamicArray { element_type } => {
                format!(
                    "{indent}Type(DynamicArray)\n{}",
                    self.visit_type(*element_type, level + 1)
                )
            }
        }
    }
    fn visit_stmt(&self, id: StmtRef, level: usize) -> String {
        let indent = " ".repeat(2 * level);
        match self.stmt_pool.get(id) {
            Stmt::Program { name, block } => {
                format!(
                    "Program {}\n{}",
                    name.lexem(self.source_code),
                    self.visit_stmt(*block, 0)
                )
            }
            Stmt::Block {
                declarations,
                statements,
            } => {
                let mut result = declarations
                    .iter()
                    .map(|d| self.visit_declaraction(d, level))
                    .collect::<Vec<String>>()
                    .join("\n");
                let compound: String = self.visit_stmt(*statements, level);
                if !compound.is_empty() {
                    result.push_str("\n");
                    result.push_str(&compound);
                }
                result
            }
            Stmt::Assign { left, right } => {
                let left_str = self.visit_expr(*left, level + 1);
                let right_str = self.visit_expr(*right, level + 1);
                format!("{indent}Assign\n{left_str}\n{right_str}")
            }
            Stmt::Compound(stmts) => {
                format!("{indent}Begin\n")
                    + &stmts
                        .iter()
                        .map(|id| self.visit_stmt(*id, level + 1))
                        .collect::<Vec<String>>()
                        .join("\n")
            }
            Stmt::Break => format!("{indent}Break"),
            Stmt::Continue => format!("{indent}Continue"),
            Stmt::Exit(v) => {
                let stmt = v.map(|v| self.visit_expr(v, level + 1));
                match stmt {
                    Some(v) => format!("{indent}Exit\n{v}"),
                    None => format!("{indent}Exit"),
                }
            }
            Stmt::For {
                var,
                init,
                end,
                body,
            } => {
                let init_str = self.visit_expr(*init, level + 1);
                let end_str = self.visit_expr(*end, level + 1);
                let body_str = self.visit_stmt(*body, level + 1);
                format!(
                    "{indent}For({})\n{init_str}\n{indent}  To\n{end_str}\n{indent}Do\n{body_str}",
                    var.lexem(self.source_code)
                )
            }
            Stmt::If {
                cond,
                elifs,
                else_statement,
            } => {
                let mut cond_str = format!(
                    "{indent}If\n{}\n{indent}Then\n{}",
                    self.visit_expr(cond.cond, level + 1),
                    self.visit_stmt(cond.expr, level + 1)
                );
                let elifs_str = &elifs
                    .iter()
                    .map(|c| {
                        format!(
                            "{indent}ElseIf\n{}\n{indent}Then\n{}",
                            self.visit_expr(c.cond, level + 1),
                            self.visit_stmt(c.expr, level + 1)
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n");
                if !elifs_str.is_empty() {
                    cond_str.push_str("\n");
                    cond_str.push_str(elifs_str);
                }
                let else_str = else_statement
                    .map(|v| format!("{indent}Else\n{}", self.visit_stmt(v, level + 1)));
                if let Some(v) = else_str {
                    cond_str.push_str("\n");
                    cond_str.push_str(&v);
                }
                cond_str
            }
            Stmt::While { cond, body } => format!(
                "{indent}While\n{}\n{indent}Do\n{}",
                self.visit_expr(*cond, level + 1),
                self.visit_stmt(*body, level + 1)
            ),
            Stmt::NoOp => format!("{indent}NoOp"),
            Stmt::Call { call } => self.visit_expr(*call, level),
        }
    }
    fn visit_expr(&self, id: ExprRef, level: usize) -> String {
        let indent = " ".repeat(2 * level);
        match self.expr_pool.get(id) {
            Expr::BinOp { op, left, right } => {
                let left_str = self.visit_expr(*left, level + 1);
                let right_str = self.visit_expr(*right, level + 1);
                format!("{indent}BinOp\n{left_str}\n{indent}  {:?}\n{right_str}", op)
            }
            Expr::LiteralInteger(v) => format!("{indent}LitInt({v})"),
            Expr::LiteralBool(v) => format!("{indent}LitBool({v})"),
            Expr::LiteralChar(v) => format!("{indent}LitChar('{v}')"),
            Expr::LiteralReal(v) => format!("{indent}LitReal({v})"),
            Expr::LiteralString(v) => {
                format!("{indent}LitString(\"{}\")", v.lexem(self.source_code))
            }
            Expr::Var { name } => format!("{indent}Var({})", name.lexem(self.source_code)),
            Expr::Call { name, args } => {
                let mut result = format!("{indent}Call({})", name.lexem(self.source_code));
                let param_str = args
                    .iter()
                    .map(|p| self.visit_expr(*p, level + 1))
                    .collect::<Vec<String>>()
                    .join("\n");
                if !param_str.is_empty() {
                    result.push_str("\n");
                    result.push_str(&param_str);
                }
                result
            }
            Expr::Index {
                base,
                index_value,
                other_indicies,
            } => {
                let mut base_str = format!("{indent}Index({})\n", self.visit_expr(*base, 0));
                let mut indicies = vec![self.visit_expr(*index_value, level + 1)];
                indicies.extend(
                    other_indicies
                        .iter()
                        .map(|v| self.visit_expr(*v, level + 1)),
                );
                base_str.push_str(&indicies.join("\n"));
                base_str
            }
            Expr::UnaryOp { op, expr } => format!(
                "{indent}UnaryOp\n{indent}  {:?}\n{}",
                op,
                self.visit_expr(*expr, level + 1)
            ),
        }
    }
}

impl<'a> Display for Tree<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.visit_stmt(self.program, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_err {
        ($($name:ident($source:literal, $err_code:path, $row:literal, $col:literal),)*) => {
            $(
                #[test]
                fn $name() {
                    let lexer = Lexer::new($source);
                    let parser = Parser::new(lexer);
                    assert!(parser.is_ok());
                    let parser = parser.unwrap();
                    let tree = parser.parse();
                    assert!(tree.is_err());
                    let err = tree.unwrap_err();
                    println!("{:?}", err);
                    assert!(matches!(err, Error::ParserError { error_code: $err_code, pos: Pos { row: $row, col: $col }, .. }));
                }
            )*
        };
    }

    macro_rules! test_succ_file {
        ($($name:ident,)*) => {
            $(
                #[test]
                fn $name() {
                    let p = "test_cases\\parser\\".to_string() + stringify!($name) + ".pas";
                    let source_code = std::fs::read_to_string(p).unwrap();
                    let lexer = Lexer::new(&source_code);
                    let parser = Parser::new(lexer);
                    assert!(parser.is_ok());
                    let parser = parser.unwrap();
                    let tree = parser.parse();
                    println!("{:?}", tree);
                    assert!(tree.is_ok());
                    let tree = tree.unwrap();
                    let result = std::fs::read_to_string("test_cases\\parser\\".to_string() + stringify!($name)).unwrap().replace("\r", "");
                    assert_eq!(
                        &format!("{tree}"),
                        &result,
                    )
                }

            )*
        };
    }

    test_succ_file! {
        test_empty_program,
        test_decls,
        test_callable_decls,
        test_binary,
        test_if_stmt,
        test_while_loop,
        test_for_loop,
        test_string,
        test_array_decl,
        test_enum,
        test_range,
    }

    test_err! {
        test_unexpected_id(".", ErrorCode::UnexpectedToken, 1, 2),
        test_expected_id_got("PROGRAM 3 BEGIN END.", ErrorCode::UnexpectedToken, 1, 9),
        test_unexpected_factor("PROGRAM n; BEGIN a := 10 + IF; END.", ErrorCode::UnexpectedToken, 1, 28),
        test_unkown_literal("PROGRAM n; var a: integer = for; BEGIN END.", ErrorCode::UnkownLiteral, 1, 29),
    }
}

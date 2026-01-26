use std::fmt::Display;

use crate::{
    error::Error,
    lexer::Lexer,
    tokens::Token,
    utils::{NodePool, define_ref},
};

define_ref!(ExprRef);
define_ref!(StmtRef);
define_ref!(TypeRef);

#[derive(Debug, Clone)]
pub enum Expr {
    Var {
        name: String,
    },
    BinOp {
        op: Token,
        left: ExprRef,
        right: ExprRef,
    },
    LiteralInteger(i64),
    LiteralReal(f64),
    LiteralBool(bool),
    LiteralChar(char),
    LiteralString(String),
    UnaryOp {
        op: Token,
        expr: ExprRef,
    },
    Call {
        name: String,
        args: Vec<ExprRef>,
    },
    Index {
        base: ExprRef,
        index_value: ExprRef,
        other_indicies: Vec<ExprRef>,
    },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Break,
    Continue,
    Assign {
        left: ExprRef,
        right: ExprRef,
    },
    NoOp,
    If {
        cond: Condition,
        elifs: Vec<Condition>,
        else_statement: Option<StmtRef>,
    },
    While {
        cond: ExprRef,
        body: StmtRef,
    },
    For {
        var: String,
        init: ExprRef,
        end: ExprRef,
        body: StmtRef,
    },
    Exit(Option<ExprRef>),
    Compound(Vec<StmtRef>),
    Call {
        call: ExprRef,
    },
}

#[derive(Debug, Clone)]
pub struct Condition {
    cond: ExprRef,
    expr: StmtRef,
}

#[derive(Debug, Clone)]
pub struct Program {
    name: String,
    block: Block,
}

#[derive(Debug, Clone)]
pub struct Block {
    declarations: Vec<Decl>,
    statements: StmtRef,
}

#[derive(Debug, Clone)]
pub enum Decl {
    VarDecl {
        var: ExprRef,
        type_node: TypeRef,
        default_value: Option<ExprRef>,
    },
    TypeDecl {
        var: ExprRef,
        type_node: TypeRef,
    },
    ConstDecl {
        var: ExprRef,
        literal: ExprRef,
    },
    Function {
        name: String,
        block: Block,
        params: Vec<Param>,
        return_type: TypeRef,
    },
    Procedure {
        name: String,
        block: Block,
        params: Vec<Param>,
    },
}

#[derive(Debug, Clone)]
pub struct Param {
    var: ExprRef,
    out: bool,
    type_node: TypeRef,
}

#[derive(Debug, Clone)]
pub enum Type {
    Integer,
    Boolean,
    String,
    Char,
    Real,
    Alias(String),
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
        items: Vec<String>,
    },
}

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    expr_pool: NodePool<ExprRef, Expr>,
    stmt_pool: NodePool<StmtRef, Stmt>,
    type_pool: NodePool<TypeRef, Type>,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Result<Self, Error> {
        let token = lexer.next()?;
        Ok(Self {
            lexer,
            current_token: token,
            expr_pool: NodePool::new(),
            stmt_pool: NodePool::new(),
            type_pool: NodePool::new(),
        })
    }

    fn eat(&mut self, expected: Token) -> Result<Token, Error> {
        let token = self.current_token.clone();
        if token != expected {
            return Err(Error::ParserError {
                msg: format!(
                    "expected token {:?}, got {:?}",
                    expected, self.current_token
                ),
                error_code: None,
            });
        }
        self.current_token = self.lexer.next()?;
        Ok(token)
    }

    /// program:
    /// Program Id Semi block Dot
    fn program(&mut self) -> Result<Program, Error> {
        self.eat(Token::Program)?;
        let var = self.id_str()?;
        self.eat(Token::Semi)?;
        let block = self.block()?;
        self.eat(Token::Dot)?;
        Ok(Program {
            name: var,
            block: block,
        })
    }

    /// id:
    /// ID
    fn id(&mut self) -> Result<ExprRef, Error> {
        if let Token::Id(id) = &self.current_token {
            let id = id.clone();
            self.eat(Token::Id(id.clone()))?;
            return Ok(self.expr_pool.alloc(Expr::Var { name: id }));
        }
        Err(Error::ParserError {
            msg: format!("expected id, got {:?}", self.current_token),
            error_code: None,
        })
    }

    fn id_str(&mut self) -> Result<String, Error> {
        if let Token::Id(id) = &self.current_token {
            let id = id.clone();
            self.current_token = self.lexer.next()?;
            return Ok(id);
        }
        Err(Error::ParserError {
            msg: format!("expected id, got {:?}", self.current_token),
            error_code: None,
        })
    }

    /// block:
    /// declarations compound_statement
    fn block(&mut self) -> Result<Block, Error> {
        let nodes = self.declarations()?;
        let compound = self.compound_statement()?;
        Ok(Block {
            declarations: nodes,
            statements: compound,
        })
    }

    /// declarations:
    /// (
    /// Const (const_declaration Semi)+ |
    /// Type (type_declaration Semi)+ |
    /// Var (var_declataion Semi)+ |
    /// procedure_declaration |
    /// function_declaration
    /// )*
    fn declarations(&mut self) -> Result<Vec<Decl>, Error> {
        let mut decls = Vec::new();
        while matches!(
            self.current_token,
            Token::Const | Token::Type | Token::Var | Token::Function | Token::Procedure
        ) {
            match self.current_token {
                Token::Const => {
                    self.eat(Token::Const)?;
                    decls.extend(self.const_declaration()?);
                    self.eat(Token::Semi)?;
                    while let Token::Id(_) = self.current_token {
                        decls.extend(self.const_declaration()?);
                        self.eat(Token::Semi)?;
                    }
                }
                Token::Type => {
                    self.eat(Token::Type)?;
                    decls.extend(self.type_declaration()?);
                    self.eat(Token::Semi)?;
                    while let Token::Id(_) = self.current_token {
                        decls.extend(self.type_declaration()?);
                        self.eat(Token::Semi)?;
                    }
                }
                Token::Var => {
                    self.eat(Token::Var)?;
                    decls.extend(self.var_declaration()?);
                    self.eat(Token::Semi)?;
                    while let Token::Id(_) = self.current_token {
                        decls.extend(self.var_declaration()?);
                        self.eat(Token::Semi)?;
                    }
                }
                Token::Procedure => decls.push(self.procedure_declaration()?),
                Token::Function => decls.push(self.function_declaration()?),
                _ => unreachable!(),
            }
        }
        Ok(decls)
    }

    /// function_declaration:
    /// Function id (LParen formal_parameter_list RParen)?
    /// Colon type_spec Semi block Semi
    fn function_declaration(&mut self) -> Result<Decl, Error> {
        self.eat(Token::Function)?;
        let func_name = self.id_str()?;
        let params = match self.current_token {
            Token::LParen => {
                self.eat(Token::LParen)?;
                let params = self.formal_parameter_list()?;
                self.eat(Token::RParen)?;
                params
            }
            _ => Vec::with_capacity(0),
        };
        self.eat(Token::Colon)?;
        let return_type = self.type_spec()?;
        self.eat(Token::Semi)?;
        let block = self.block()?;
        self.eat(Token::Semi)?;
        Ok(Decl::Function {
            name: func_name,
            block,
            params,
            return_type,
        })
    }

    /// var_declaration:
    /// Var id (Comma id)* Colon type_spec (Equal literal)?
    fn var_declaration(&mut self) -> Result<Vec<Decl>, Error> {
        let mut vars = vec![self.id()?];
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            vars.push(self.id()?);
        }
        self.eat(Token::Colon)?;
        let type_spec = self.type_spec()?;
        let default_value = match self.current_token {
            Token::Equal => {
                self.eat(Token::Equal)?;
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
        self.eat(Token::Procedure)?;
        let proc_name = self.id_str()?;
        let params = match self.current_token {
            Token::LParen => {
                self.eat(Token::LParen)?;
                let params = self.formal_parameter_list()?;
                self.eat(Token::RParen)?;
                params
            }
            _ => Vec::with_capacity(0),
        };
        let block = self.block()?;
        self.eat(Token::Semi)?;
        Ok(Decl::Procedure {
            name: proc_name,
            block,
            params,
        })
    }

    /// formal_parameter_list:
    /// formal_parameters (Semi formal_parameter_list)?
    fn formal_parameter_list(&mut self) -> Result<Vec<Param>, Error> {
        let mut params = self.formal_parameters()?;
        if let Token::Semi = self.current_token {
            self.eat(Token::Semi)?;
            params.extend(self.formal_parameter_list()?);
        };
        Ok(params)
    }

    /// formal_parameters:
    /// Out? id (Comma Out? id)* Colon type_spec
    fn formal_parameters(&mut self) -> Result<Vec<Param>, Error> {
        let out = match self.current_token {
            Token::Out => {
                self.eat(Token::Out)?;
                true
            }
            _ => false,
        };
        let mut names = vec![(out, self.id()?)];
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            let out = match self.current_token {
                Token::Out => {
                    self.eat(Token::Out)?;
                    true
                }
                _ => false,
            };
            names.push((out, self.id()?));
        }
        self.eat(Token::Colon)?;
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
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            names.push(self.id()?);
        }
        self.eat(Token::Equal)?;
        let literal = self.literal()?;
        Ok(names
            .iter()
            .map(|n| Decl::ConstDecl { var: *n, literal })
            .collect())
    }

    /// type_declaration:
    /// Type id (Comma id)* Equal type_spec
    fn type_declaration(&mut self) -> Result<Vec<Decl>, Error> {
        let mut names = vec![self.id()?];
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            names.push(self.id()?);
        }
        self.eat(Token::Equal)?;
        let type_decl = self.type_spec()?;

        Ok(names
            .iter()
            .map(|n| Decl::TypeDecl {
                var: *n,
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
        let token = self.current_token.clone();
        match token {
            Token::Id(v) => {
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Alias(v)))
            }
            Token::Integer => {
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Integer))
            }
            Token::Real => {
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Real))
            }
            Token::Boolean => {
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Boolean))
            }
            Token::String => {
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::String))
            }
            Token::Char => {
                self.current_token = self.lexer.next()?;
                Ok(self.type_pool.alloc(Type::Char))
            }
            Token::LParen => self.enum_spec(),
            Token::Array => self.array_spec(),
            _ => self.range_spec(),
        }
    }

    /// enum_spec:
    /// LParan id (Comma id)* RParan
    fn enum_spec(&mut self) -> Result<TypeRef, Error> {
        self.eat(Token::LParen)?;
        let mut items = vec![self.id_str()?];
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            items.push(self.id_str()?);
        }
        self.eat(Token::RParen)?;
        Ok(self.type_pool.alloc(Type::Enum { items }))
    }

    /// array_spec:
    /// Array (LBrack range_spec RBrack)? Of type_spec
    fn array_spec(&mut self) -> Result<TypeRef, Error> {
        self.eat(Token::Array)?;
        if let Token::LBracket = self.current_token {
            self.eat(Token::LBracket)?;
            let index_type = self.range_spec()?;
            self.eat(Token::RBracket)?;
            self.eat(Token::Of)?;
            let element_type = self.type_spec()?;
            return Ok(self.type_pool.alloc(Type::Array {
                index_type,
                element_type,
            }));
        };
        self.eat(Token::Of)?;
        let element_type = self.type_spec()?;
        Ok(self.type_pool.alloc(Type::DynamicArray { element_type }))
    }

    /// range_spec:
    /// (id | literal) Dot Dot (id | literal)
    fn range_spec(&mut self) -> Result<TypeRef, Error> {
        let start = match self.current_token {
            Token::Id(_) => self.id()?,
            _ => self.literal()?,
        };
        self.eat(Token::Dot)?;
        self.eat(Token::Dot)?;
        let end = match self.current_token {
            Token::Id(_) => self.id()?,
            _ => self.literal()?,
        };
        Ok(self.type_pool.alloc(Type::Range {
            start_val: start,
            end_val: end,
        }))
    }

    /// compound_statement:
    /// Begin statement_list End
    fn compound_statement(&mut self) -> Result<StmtRef, Error> {
        self.eat(Token::Begin)?;
        let statement_list = self.statement_list()?;
        self.eat(Token::End)?;
        Ok(self.stmt_pool.alloc(Stmt::Compound(statement_list)))
    }

    /// statement_list:
    /// statement (Semi statement)*
    fn statement_list(&mut self) -> Result<Vec<StmtRef>, Error> {
        let mut statements = vec![self.statement()?];
        while let Token::Semi = self.current_token {
            self.eat(Token::Semi)?;
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
        match self.current_token {
            Token::Continue => Ok(self.stmt_pool.alloc(Stmt::Continue)),
            Token::Break => Ok(self.stmt_pool.alloc(Stmt::Break)),
            Token::Begin => self.compound_statement(),
            Token::Id(_) => match self.lexer.current_char() {
                Some('(') => self.call_statement(),
                _ => self.assignment_statement(),
            },
            Token::If => self.if_statement(),
            Token::While => self.while_statement(),
            Token::For => self.for_statement(),
            Token::Exit => self.exit_statement(),
            _ => Ok(self.stmt_pool.alloc(Stmt::NoOp)),
        }
    }

    /// exit_statement:
    /// Exit (LParan exprt RParan)?
    fn exit_statement(&mut self) -> Result<StmtRef, Error> {
        self.eat(Token::Exit)?;
        let mut expr = None;
        if let Token::LParen = self.current_token {
            self.eat(Token::LParen)?;
            expr = Some(self.expr()?);
            self.eat(Token::RParen)?;
        };
        Ok(self.stmt_pool.alloc(Stmt::Exit(expr)))
    }

    /// for_statement:
    /// For id Assign expr To expr Do statement
    fn for_statement(&mut self) -> Result<StmtRef, Error> {
        self.eat(Token::For)?;
        let var = self.id_str()?;
        self.eat(Token::Assign)?;
        let init_state = self.expr()?;
        self.eat(Token::To)?;
        let end_state = self.expr()?;
        self.eat(Token::Do)?;
        let expr = self.statement()?;
        Ok(self.stmt_pool.alloc(Stmt::For {
            var,
            init: init_state,
            end: end_state,
            body: expr,
        }))
    }

    /// while_statement:
    /// While expr Do statement
    fn while_statement(&mut self) -> Result<StmtRef, Error> {
        self.eat(Token::While)?;
        let cond = self.expr()?;
        self.eat(Token::Do)?;
        let body = self.statement()?;
        Ok(self.stmt_pool.alloc(Stmt::While { cond, body }))
    }

    /// if_statement:
    /// If condition
    /// (Else If condition)*
    /// (Else statement)?
    fn if_statement(&mut self) -> Result<StmtRef, Error> {
        self.eat(Token::If)?;
        let main_cond = self.condition()?;
        let mut other_conditions = Vec::new();
        let mut last_conditition = None;
        while let Token::Else = self.current_token {
            self.eat(Token::Else)?;
            match self.current_token {
                Token::If => {
                    self.eat(Token::If)?;
                    other_conditions.push(self.condition()?);
                }
                _ => {
                    last_conditition = Some(self.statement()?);
                    break;
                }
            }
        }
        Ok(self.stmt_pool.alloc(Stmt::If {
            cond: main_cond,
            elifs: other_conditions,
            else_statement: last_conditition,
        }))
    }

    /// condition:
    /// expr Then statement
    fn condition(&mut self) -> Result<Condition, Error> {
        let cond = self.expr()?;
        self.eat(Token::Then)?;
        let expr = self.statement()?;
        Ok(Condition { cond, expr })
    }

    /// assignment_statement:
    /// id Assign expr
    fn assignment_statement(&mut self) -> Result<StmtRef, Error> {
        let var = self.id()?;
        self.eat(Token::Assign)?;
        let expr = self.expr()?;
        Ok(self.stmt_pool.alloc(Stmt::Assign {
            left: var,
            right: expr,
        }))
    }

    /// expr:
    /// bool_expr (OR bool_expr)*
    fn expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.bool_expr()?;
        while let Token::Or = self.current_token {
            self.eat(Token::Or)?;
            let right = self.bool_expr()?;
            node = self.expr_pool.alloc(Expr::BinOp {
                op: Token::Or,
                left: node,
                right,
            });
        }
        Ok(node)
    }

    /// bool_expr:
    /// compare_expr (AND compare_expr)*
    fn bool_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.compare_expr()?;
        while let Token::And = self.current_token {
            self.eat(Token::And)?;
            let right = self.compare_expr()?;
            node = self.expr_pool.alloc(Expr::BinOp {
                op: Token::And,
                left: node,
                right,
            });
        }
        Ok(node)
    }

    /// compare_expr:
    /// add_expr (compare_token add_expr)*
    fn compare_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.add_expr()?;
        while self.current_token.is_compare_operator() {
            let token = self.current_token.clone();
            self.current_token = self.lexer.next()?;
            let right = self.add_expr()?;
            node = self.expr_pool.alloc(Expr::BinOp {
                op: token,
                left: node,
                right,
            });
        }
        Ok(node)
    }

    /// add_expr
    /// mult_expr ((Minus | Plus) mult_expr)*
    fn add_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.mult_expr()?;
        while matches!(self.current_token, Token::Plus | Token::Minus) {
            let token = self.current_token.clone();
            self.current_token = self.lexer.next()?;
            let right = self.mult_expr()?;
            node = self.expr_pool.alloc(Expr::BinOp {
                op: token,
                left: node,
                right,
            });
        }
        Ok(node)
    }

    /// mult_expr:
    /// factor ((Mult | Div | RealDiv) factor)*
    fn mult_expr(&mut self) -> Result<ExprRef, Error> {
        let mut node = self.factor()?;
        while matches!(
            self.current_token,
            Token::Mul | Token::RealDiv | Token::IntegerDiv
        ) {
            let token = self.current_token.clone();
            self.current_token = self.lexer.next()?;
            let right = self.factor()?;
            node = self.expr_pool.alloc(Expr::BinOp {
                op: token,
                left: node,
                right,
            });
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
        match self.current_token {
            Token::Plus | Token::Minus => {
                let token = self.current_token.clone();
                self.current_token = self.lexer.next()?;
                let factor = self.factor()?;
                return Ok(self.expr_pool.alloc(Expr::UnaryOp {
                    op: token,
                    expr: factor,
                }));
            }
            Token::Not => {
                self.eat(Token::Not)?;
                let expr = self.compare_expr()?;
                return Ok(self.expr_pool.alloc(Expr::UnaryOp {
                    op: Token::Not,
                    expr,
                }));
            }
            Token::IntegerConst(_)
            | Token::RealConst(_)
            | Token::BooleanConst(_)
            | Token::StringConst(_)
            | Token::CharConst(_) => self.literal(),
            Token::LParen => {
                self.eat(Token::LParen)?;
                let expr = self.expr();
                self.eat(Token::RParen)?;
                return expr;
            }
            Token::Id(_) => match self.lexer.current_char() {
                Some('(') => self.call_expr(),
                Some('[') => self.index_of_statement(),
                _ => self.id(),
            },
            _ => Err(Error::ParserError {
                msg: "unexpected factor".to_string(),
                error_code: None,
            }),
        }
    }

    /// call_expr:
    /// Id LParan expr (Comma expr)* RParan
    fn call_expr(&mut self) -> Result<ExprRef, Error> {
        let proc_name = self.id_str()?;
        self.eat(Token::LParen)?;
        let mut params = Vec::new();
        if self.current_token != Token::RParen {
            params.push(self.expr()?);
        }
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            params.push(self.expr()?);
        }
        self.eat(Token::RParen)?;
        Ok(self.expr_pool.alloc(Expr::Call {
            name: proc_name,
            args: params,
        }))
    }

    /// call_statement:
    /// Id LParan expr (Comma expr)* RParan
    fn call_statement(&mut self) -> Result<StmtRef, Error> {
        let call = self.call_expr()?;
        Ok(self.stmt_pool.alloc(Stmt::Call { call }))
    }

    /// index_of_statement:
    /// If LBracket expr (Comma expr)* RBracket
    fn index_of_statement(&mut self) -> Result<ExprRef, Error> {
        let var_node = self.id()?;
        self.eat(Token::LBracket)?;
        let expr = self.expr()?;
        let mut other_indicies = Vec::new();
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            other_indicies.push(self.expr()?);
        }
        self.eat(Token::RBracket)?;
        Ok(self.expr_pool.alloc(Expr::Index {
            base: var_node,
            index_value: expr,
            other_indicies,
        }))
    }

    fn literal(&mut self) -> Result<ExprRef, Error> {
        let token = self.current_token.clone();
        self.current_token = self.lexer.next()?;
        match token {
            Token::IntegerConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralInteger(v))),
            Token::RealConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralReal(v))),
            Token::StringConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralString(v))),
            Token::CharConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralChar(v))),
            Token::BooleanConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralBool(v))),
            _ => Err(Error::ParserError {
                msg: format!("unkown literal {:?}", token),
                error_code: None,
            }),
        }
    }

    pub fn parse(&mut self) -> Result<Tree, Error> {
        let program = self.program()?;
        Ok(Tree {
            program,
            expr_pool: self.expr_pool.clone(),
            stmt_pool: self.stmt_pool.clone(),
            type_pool: self.type_pool.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Tree {
    pub program: Program,
    pub expr_pool: NodePool<ExprRef, Expr>,
    pub stmt_pool: NodePool<StmtRef, Stmt>,
    pub type_pool: NodePool<TypeRef, Type>,
}

impl Tree {
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
                    self.visit_expr(*var, level + 1),
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
            Decl::Procedure {
                name,
                block,
                params,
            } => {
                let mut result = format!("{indent}Procedure({name})");
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
                let decls_str = block
                    .declarations
                    .iter()
                    .map(|d| self.visit_declaraction(d, level))
                    .collect::<Vec<String>>()
                    .join("\n");
                if !decls_str.is_empty() {
                    result.push_str("\n");
                    result.push_str(&decls_str);
                }
                result.push_str("\n");
                result.push_str(&self.visit_stmt(block.statements, level));
                result
            }
            Decl::Function {
                name,
                block,
                params,
                return_type,
            } => {
                let mut result = format!(
                    "{indent}Function({name})\n{}",
                    self.visit_type(*return_type, level + 1)
                );
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
                let decls_str = block
                    .declarations
                    .iter()
                    .map(|d| self.visit_declaraction(d, level))
                    .collect::<Vec<String>>()
                    .join("\n");
                if !decls_str.is_empty() {
                    result.push_str("\n");
                    result.push_str(&decls_str);
                }
                result.push_str("\n");
                result.push_str(&self.visit_stmt(block.statements, level));
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
            Type::Alias(v) => format!("{indent}TypeAlias({v})"),
            Type::Enum { items } => format!(
                "{indent}Type(Enum)\n{}",
                items
                    .iter()
                    .map(|i| { format!("{indent}  {i}") })
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
                    "{indent}For({var})\n{init_str}\n{indent}  To\n{end_str}\n{indent}Do\n{body_str}"
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
            Expr::LiteralString(v) => format!("{indent}LitString(\"{v}\")"),
            Expr::Var { name } => format!("{indent}Var({name})"),
            Expr::Call { name, args } => {
                let mut result = format!("{indent}Call({name})");
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

impl Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut result = format!("Program {}", self.program.name);
        let decls = self
            .program
            .block
            .declarations
            .iter()
            .map(|d| self.visit_declaraction(d, 0))
            .collect::<Vec<String>>()
            .join("\n");
        if !decls.is_empty() {
            result.push_str("\n");
            result.push_str(&decls);
        };
        let compound: String = self.visit_stmt(self.program.block.statements, 0);
        if !compound.is_empty() {
            result.push_str("\n");
            result.push_str(&compound);
        }
        write!(f, "{result}")
    }
}

use crate::{error::Error, lexer::Lexer, tokens::Token};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

#[derive(Debug, Clone)]
pub struct NodePool<T>(Vec<T>);

impl<T> NodePool<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn alloc(&mut self, node: T) -> NodeId {
        let id = NodeId(self.0.len() as u32);
        self.0.push(node);
        id
    }

    pub fn get(&self, id: NodeId) -> &T {
        &self.0[id.0 as usize]
    }

    pub fn get_mut(&mut self, id: NodeId) -> &mut T {
        &mut self.0[id.0 as usize]
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Var {
        name: String,
    },
    BinOp {
        op: Token,
        left: NodeId,
        right: NodeId,
    },
    LiteralInteger(i64),
    LiteralReal(f64),
    LiteralBool(bool),
    LiteralChar(char),
    LiteralString(String),
    UnaryOp {
        op: Token,
        expr: NodeId,
    },
    Call {
        name: String,
        args: Vec<NodeId>,
    },
    Index {
        base: NodeId,
        index_value: NodeId,
    },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Break,
    Continue,
    Assign {
        left: NodeId,
        right: NodeId,
    },
    NoOp,
    If {
        cond: NodeId,
        elifs: Vec<NodeId>,
        else_statement: NodeId,
    },
    While {
        cond: NodeId,
        body: NodeId,
    },
    For {
        var: String,
        init: NodeId,
        end: NodeId,
        body: NodeId,
    },
    Exit(Option<NodeId>),
    Compound(Vec<NodeId>),
    Call {
        call: NodeId,
    },
}

#[derive(Debug, Clone)]
pub struct Program {
    name: NodeId,
    block: Block,
}

#[derive(Debug, Clone)]
pub struct Block {
    declarations: Vec<Decl>,
    statements: NodeId,
}

#[derive(Debug, Clone)]
pub enum Decl {
    VarDecl {
        var: NodeId,
        type_node: NodeId,
        default_value: Option<NodeId>,
    },
    TypeDecl {
        var: NodeId,
        type_node: NodeId,
    },
    ConstDecl {
        var: NodeId,
        literal: NodeId,
    },
    Function {
        name: String,
        block: NodeId,
        params: Vec<Param>,
        return_type: NodeId,
    },
    Procedure {
        name: String,
        block: NodeId,
        params: Vec<Param>,
    },
}

#[derive(Debug, Clone)]
pub struct Param {
    var: NodeId,
    type_node: NodeId,
}

#[derive(Debug, Clone)]
pub enum Type {
    Integer,
    Boolean,
    String,
    Char,
    Real,
    Array {
        index_type: NodeId,
        element_type: NodeId,
    },
    DynamicArray {
        element_type: NodeId,
    },
    Range {
        start_val: NodeId,
        end_val: NodeId,
    },
    Enum {
        items: Vec<String>,
    },
}

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    expr_pool: NodePool<Expr>,
    stmt_pool: NodePool<Stmt>,
    type_pool: NodePool<Type>,
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
        let var = self.id()?;
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
    fn id(&mut self) -> Result<NodeId, Error> {
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
        Ok(Vec::new())
    }

    /// compound_statement:
    /// Begin statement_list End
    fn compound_statement(&mut self) -> Result<NodeId, Error> {
        self.eat(Token::Begin)?;
        let statement_list = self.statement_list()?;
        self.eat(Token::End)?;
        Ok(self.stmt_pool.alloc(Stmt::Compound(statement_list)))
    }

    /// statement_list:
    /// statement (Semi statement)*
    fn statement_list(&mut self) -> Result<Vec<NodeId>, Error> {
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
    fn statement(&mut self) -> Result<NodeId, Error> {
        match self.current_token {
            Token::Id(_) => self.assignment_statement(),
            _ => Ok(self.stmt_pool.alloc(Stmt::NoOp)),
        }
    }

    /// assignment_statement:
    /// id Assign expr
    fn assignment_statement(&mut self) -> Result<NodeId, Error> {
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
    fn expr(&mut self) -> Result<NodeId, Error> {
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
    fn bool_expr(&mut self) -> Result<NodeId, Error> {
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
    fn compare_expr(&mut self) -> Result<NodeId, Error> {
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
    fn add_expr(&mut self) -> Result<NodeId, Error> {
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
    fn mult_expr(&mut self) -> Result<NodeId, Error> {
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
    /// call_statement |
    /// index_of_statement |
    /// variable
    fn factor(&mut self) -> Result<NodeId, Error> {
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
            _ => self.id(),
        }
    }

    fn literal(&mut self) -> Result<NodeId, Error> {
        let token = self.current_token.clone();
        self.current_token = self.lexer.next()?;
        match token {
            Token::IntegerConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralInteger(v))),
            Token::RealConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralReal(v))),
            Token::StringConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralString(v))),
            Token::CharConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralChar(v))),
            Token::BooleanConst(v) => Ok(self.expr_pool.alloc(Expr::LiteralBool(v))),
            _ => Err(Error::ParserError {
                msg: "unkown literal".to_string(),
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
    pub expr_pool: NodePool<Expr>,
    pub stmt_pool: NodePool<Stmt>,
    pub type_pool: NodePool<Type>,
}

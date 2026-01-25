use std::{fmt::Display, process::Command};

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
        other_indicies: Vec<NodeId>,
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
        cond: Condition,
        elifs: Vec<Condition>,
        else_statement: Option<NodeId>,
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
pub struct Condition {
    cond: NodeId,
    expr: NodeId,
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
            Token::Continue => Ok(self.stmt_pool.alloc(Stmt::Continue)),
            Token::Break => Ok(self.stmt_pool.alloc(Stmt::Break)),
            Token::Begin => self.compound_statement(),
            Token::Id(_) => match self.lexer.peek() {
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
    fn exit_statement(&mut self) -> Result<NodeId, Error> {
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
    fn for_statement(&mut self) -> Result<NodeId, Error> {
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
    fn while_statement(&mut self) -> Result<NodeId, Error> {
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
    fn if_statement(&mut self) -> Result<NodeId, Error> {
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
    /// call_expr |
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
    fn call_expr(&mut self) -> Result<NodeId, Error> {
        let proc_name = self.id_str()?;
        self.eat(Token::LParen)?;
        let mut params = Vec::new();
        if self.current_token != Token::LParen {
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
    fn call_statement(&mut self) -> Result<NodeId, Error> {
        let call = self.call_expr()?;
        Ok(self.stmt_pool.alloc(Stmt::Call { call }))
    }

    /// index_of_statement:
    /// If LBracket expr (Comma expr)* RBracket
    fn index_of_statement(&mut self) -> Result<NodeId, Error> {
        let var_node = self.id()?;
        self.eat(Token::LBracket)?;
        let expr = self.expr()?;
        let mut other_indicies = Vec::new();
        while let Token::Comma = self.current_token {
            self.eat(Token::Comma)?;
            other_indicies.push(self.expr()?);
        }
        self.eat(Token::LBracket)?;
        Ok(self.expr_pool.alloc(Expr::Index {
            base: var_node,
            index_value: expr,
            other_indicies,
        }))
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

impl Tree {
    fn visit_stmt(&self, id: NodeId, level: usize) -> String {
        let indent = " ".repeat(2 * level);
        match self.stmt_pool.get(id) {
            Stmt::Assign { left, right } => {
                let left_str = self.visit_expr(*left, level + 1);
                let right_str = self.visit_expr(*right, level + 1);
                format!("{indent}Assign\n{left_str}\n{right_str}")
            }
            Stmt::Compound(stmts) => stmts
                .iter()
                .map(|id| self.visit_stmt(*id, level + 1))
                .collect::<Vec<String>>()
                .join(""),
            Stmt::NoOp => indent,
            _ => "stmt".to_string(),
        }
    }
    fn visit_expr(&self, id: NodeId, level: usize) -> String {
        let indent = " ".repeat(2 * level);
        match self.expr_pool.get(id) {
            Expr::BinOp { op, left, right } => {
                let left_str = self.visit_expr(*left, level + 1);
                let right_str = self.visit_expr(*right, level + 1);
                format!("{indent}BinOp\n{left_str}\n{indent}  {:?}\n{right_str}", op)
            }
            Expr::LiteralInteger(v) => format!("{indent}Lit({v})"),
            Expr::LiteralBool(v) => format!("{indent}Lit({v})"),
            Expr::LiteralChar(v) => format!("{indent}Lit({v})"),
            Expr::LiteralReal(v) => format!("{indent}Lit({v})"),
            Expr::LiteralString(v) => format!("{indent}Lit({v})"),
            Expr::Var { name } => format!("{indent}Var({name})"),
            Expr::Call { name, args } => {
                let param_str = args
                    .iter()
                    .map(|p| self.visit_expr(*p, level + 1))
                    .collect::<Vec<String>>()
                    .join("\n");
                format!("{indent}Call({name})\n{param_str}")
            }
            _ => "expr".to_string(),
        }
    }
}

impl Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.expr_pool.get(self.program.name) {
            Expr::Var { name } => name,
            _ => panic!("should be var"),
        };
        let compund: String = match self.stmt_pool.get(self.program.block.statements) {
            Stmt::Compound(stmts) => stmts,
            _ => panic!("should be compund"),
        }
        .iter()
        .map(|stmt| self.visit_stmt(*stmt, 1))
        .collect::<Vec<String>>()
        .join("\n");
        write!(f, "Program {name}\n{compund}")
    }
}

use crate::ast;
use crate::runtime;
use sqlparser::ast as sqlast;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub type Ident = ast::Ident;

#[derive(Clone, Eq, PartialEq)]
pub struct SchemaInstance {
    pub schema: SchemaRef,
    pub id: Option<usize>,
}

impl fmt::Debug for SchemaInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(f.debug_struct("FnExpr")
            .field("id", &self.id)
            .finish_non_exhaustive()?)
    }
}

impl SchemaInstance {
    pub fn global(schema: SchemaRef) -> SchemaInstance {
        SchemaInstance { schema, id: None }
    }

    pub fn instance(schema: SchemaRef, id: usize) -> SchemaInstance {
        SchemaInstance {
            schema,
            id: Some(id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtomicType {
    Null,
    Bool,
    Number,
    String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnType {
    pub args: Vec<TypedName>,
    pub ret: Box<Type>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Unknown,
    Atom(AtomicType),
    Struct(BTreeMap<String, Type>),
    List(Box<Type>),
    Exclude {
        inner: Box<Type>,
        excluded: Vec<Ident>,
    },
    Decl(DeclRef),
    Fn(FnType),
}

pub type Value = runtime::Value;

pub type Params = BTreeMap<ast::Path, TypedExpr>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SQLExpr {
    pub params: Params,
    pub expr: sqlast::Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SQLQuery {
    pub params: Params,
    pub query: sqlast::Query,
}

#[derive(Clone, Eq, PartialEq)]
pub struct FnExpr {
    pub inner_schema: Rc<RefCell<Schema>>,
    pub body: Box<Expr>,
}

impl fmt::Debug for FnExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(f.debug_struct("FnExpr")
            .field("body", &self.body)
            .finish_non_exhaustive()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    SQLQuery(SQLQuery),
    SQLExpr(SQLExpr),
    Decl(DeclRef),
    Fn(FnExpr),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedNameAndSQLExpr {
    pub name: String,
    pub type_: Type,
    pub expr: SQLExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSQLExpr {
    pub type_: Type,
    pub expr: SQLExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedExpr {
    pub type_: Type,
    pub expr: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaEntry {
    Schema(ast::Path),
    Type(Type),
    Expr(TypedExpr),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decl {
    pub public: bool,
    pub extern_: bool,
    pub name: String,
    pub value: SchemaEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedNameAndExpr {
    pub name: String,
    pub type_: Type,
    pub expr: Expr,
}

pub type SchemaRef = Rc<RefCell<Schema>>;
pub type DeclRef = Rc<RefCell<Decl>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedName {
    pub name: String,
    pub type_: Type,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedSchema {
    pub args: Option<Vec<BTreeMap<String, TypedNameAndExpr>>>,
    pub schema: SchemaRef,
}

// XXX We should implement a cheaper Eq / PartialEq over Schema, because it's
// currently used to check if two types are equal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Schema {
    pub folder: Option<String>,
    pub parent_scope: Option<Rc<RefCell<Schema>>>,
    pub next_placeholder: usize,
    pub externs: BTreeMap<String, Type>,
    pub decls: BTreeMap<String, Rc<RefCell<Decl>>>,
    pub imports: BTreeMap<ast::Path, Rc<RefCell<ImportedSchema>>>,
}

impl Schema {
    pub fn new(folder: Option<String>) -> Rc<RefCell<Schema>> {
        Rc::new(RefCell::new(Schema {
            folder,
            parent_scope: None,
            next_placeholder: 1,
            externs: BTreeMap::new(),
            decls: BTreeMap::new(),
            imports: BTreeMap::new(),
        }))
    }

    pub fn new_global_schema() -> Rc<RefCell<Schema>> {
        Rc::new(RefCell::new(Schema {
            folder: None,
            parent_scope: None,
            next_placeholder: 1,
            externs: BTreeMap::new(),
            imports: BTreeMap::new(),
            decls: BTreeMap::from([
                (
                    "number".to_string(),
                    Rc::new(RefCell::new(Decl {
                        public: true,
                        extern_: false,
                        name: "number".to_string(),
                        value: SchemaEntry::Type(Type::Atom(AtomicType::Number)),
                    })),
                ),
                (
                    "string".to_string(),
                    Rc::new(RefCell::new(Decl {
                        public: true,
                        extern_: false,
                        name: "string".to_string(),
                        value: SchemaEntry::Type(Type::Atom(AtomicType::String)),
                    })),
                ),
                (
                    "bool".to_string(),
                    Rc::new(RefCell::new(Decl {
                        public: true,
                        extern_: false,
                        name: "string".to_string(),
                        value: SchemaEntry::Type(Type::Atom(AtomicType::Bool)),
                    })),
                ),
                (
                    "null".to_string(),
                    Rc::new(RefCell::new(Decl {
                        public: true,
                        extern_: false,
                        name: "string".to_string(),
                        value: SchemaEntry::Type(Type::Atom(AtomicType::Null)),
                    })),
                ),
            ]),
        }))
    }
}

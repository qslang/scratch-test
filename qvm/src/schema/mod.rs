use crate::ast;
use crate::runtime;
use crate::types::{AtomicType, Field, FnType, Type};
use sqlparser::ast as sqlast;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

pub type Ident = ast::Ident;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MFnType {
    pub args: Vec<MField>,
    pub ret: Ref<MType>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MField {
    pub name: String,
    pub type_: Ref<MType>,
    pub nullable: bool,
}

impl MField {
    pub fn new_nullable(name: String, type_: Ref<MType>) -> MField {
        MField {
            name,
            type_,
            nullable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MType {
    Atom(AtomicType),
    Record(Vec<MField>),
    List(Ref<MType>),
    Fn(MFnType),
    Name(String),

    Unknown,
    Ref(Ref<MType>),
}

impl MType {
    pub fn new_unknown() -> Ref<MType> {
        mkref(MType::Ref(mkref(MType::Unknown)))
    }

    pub fn is_known(&self) -> bool {
        match self {
            MType::Unknown => false,
            MType::Ref(r) => r.borrow().is_known(),
            _ => true,
        }
    }

    pub fn to_runtime_type(&self) -> runtime::error::Result<Type> {
        match self {
            MType::Atom(a) => Ok(Type::Atom(a.clone())),
            MType::Record(fields) => Ok(Type::Record(
                fields
                    .iter()
                    .map(|f| {
                        Ok(Field {
                            name: f.name.clone(),
                            type_: f.type_.borrow().to_runtime_type()?,
                            nullable: f.nullable,
                        })
                    })
                    .collect::<runtime::error::Result<Vec<_>>>()?,
            )),
            MType::List(inner) => Ok(Type::List(Box::new(inner.borrow().to_runtime_type()?))),
            MType::Fn(MFnType { args, ret }) => Ok(Type::Fn(FnType {
                args: args
                    .iter()
                    .map(|a| {
                        Ok(Field {
                            name: a.name.clone(),
                            type_: a.type_.borrow().to_runtime_type()?,
                            nullable: a.nullable,
                        })
                    })
                    .collect::<runtime::error::Result<Vec<_>>>()?,
                ret: Box::new(ret.borrow().to_runtime_type()?),
            })),
            MType::Ref(r) => Ok(r.borrow().to_runtime_type()?),
            _ => runtime::error::fail!("Unresolved type cannot exist at runtime: {:?}", self),
        }
    }
}

pub type Ref<T> = Rc<RefCell<T>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SType {
    pub variables: BTreeSet<String>,
    pub body: Ref<MType>,
}

impl SType {
    pub fn new_mono(body: Ref<MType>) -> Ref<SType> {
        mkref(SType {
            variables: BTreeSet::new(),
            body,
        })
    }
}

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

pub type Value = crate::types::Value;

pub type Params<Ty> = BTreeMap<ast::Ident, TypedExpr<Ty>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SQLExpr<Ty> {
    pub params: Params<Ty>,
    pub expr: sqlast::Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SQLQuery<Ty> {
    pub params: Params<Ty>,
    pub query: sqlast::Query,
}

#[derive(Clone, Eq, PartialEq)]
pub struct FnExpr<Ty> {
    pub inner_schema: Ref<Schema>,
    pub body: Rc<Expr<Ty>>,
}

impl<Ty: fmt::Debug> fmt::Debug for FnExpr<Ty> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(f.debug_struct("FnExpr")
            .field("body", &self.body)
            .finish_non_exhaustive()?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnCallExpr<Ty> {
    pub func: Rc<TypedExpr<Ty>>,
    pub args: Vec<TypedExpr<Ty>>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SchemaEntryExpr {
    pub debug_name: String,
    pub entry: SchemaEntry,
}

impl fmt::Debug for SchemaEntryExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SchemaEntryExpr")
            .field("debug_name", &self.debug_name)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr<Ty> {
    SQLQuery(Rc<SQLQuery<Ty>>),
    SQLExpr(Rc<SQLExpr<Ty>>),
    SchemaEntry(SchemaEntryExpr),
    Fn(FnExpr<Ty>),
    FnCall(FnCallExpr<Ty>),
    NativeFn(String),
    Unknown,
}

impl Expr<MType> {
    pub fn to_runtime_type(&self) -> runtime::error::Result<Expr<Type>> {
        match self {
            Expr::SQLQuery(q) => {
                let SQLQuery { params, query } = q.as_ref();
                Ok(Expr::SQLQuery(Rc::new(SQLQuery {
                    params: params
                        .iter()
                        .map(|(name, param)| Ok((name.clone(), param.to_runtime_type()?)))
                        .collect::<runtime::error::Result<_>>()?,
                    query: query.clone(),
                })))
            }
            Expr::SQLExpr(e) => {
                let SQLExpr { params, expr } = e.as_ref();
                Ok(Expr::SQLExpr(Rc::new(SQLExpr {
                    params: params
                        .iter()
                        .map(|(name, param)| Ok((name.clone(), param.to_runtime_type()?)))
                        .collect::<runtime::error::Result<_>>()?,
                    expr: expr.clone(),
                })))
            }
            Expr::Fn(FnExpr { inner_schema, body }) => Ok(Expr::Fn(FnExpr {
                inner_schema: inner_schema.clone(),
                body: Rc::new(body.to_runtime_type()?),
            })),
            Expr::FnCall(FnCallExpr { func, args }) => Ok(Expr::FnCall(FnCallExpr {
                func: Rc::new(func.to_runtime_type()?),
                args: args
                    .iter()
                    .map(|a| Ok(a.to_runtime_type()?))
                    .collect::<runtime::error::Result<_>>()?,
            })),
            Expr::SchemaEntry(e) => Ok(Expr::SchemaEntry(e.clone())),
            Expr::NativeFn(f) => Ok(Expr::NativeFn(f.clone())),
            Expr::Unknown => Ok(Expr::Unknown),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedNameAndSQLExpr<Ty> {
    pub name: String,
    pub type_: Ref<Ty>,
    pub expr: Rc<SQLExpr<Ty>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSQLExpr<Ty> {
    pub type_: Ref<Ty>,
    pub expr: Rc<SQLExpr<Ty>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedExpr<Ty> {
    pub type_: Ref<Ty>,
    pub expr: Rc<Expr<Ty>>,
}

impl TypedExpr<MType> {
    pub fn to_runtime_type(&self) -> runtime::error::Result<TypedExpr<Type>> {
        Ok(TypedExpr::<Type> {
            type_: mkref(self.type_.borrow().to_runtime_type()?),
            expr: Rc::new(self.expr.to_runtime_type()?),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STypedExpr {
    pub type_: Ref<SType>,
    pub expr: Rc<Expr<MType>>,
}

impl STypedExpr {
    pub fn to_runtime_type(&self) -> runtime::error::Result<TypedExpr<Type>> {
        Ok(TypedExpr::<Type> {
            type_: mkref(self.type_.borrow().body.borrow().to_runtime_type()?),
            expr: Rc::new(self.expr.to_runtime_type()?),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaEntry {
    Schema(ast::Path),
    Type(Ref<MType>),
    Expr(Ref<STypedExpr>),
}

pub fn mkref<T>(t: T) -> Ref<T> {
    Rc::new(RefCell::new(t))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decl {
    pub public: bool,
    pub extern_: bool,
    pub name: String,
    pub value: SchemaEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedNameAndExpr<Ty> {
    pub name: String,
    pub type_: Ref<Ty>,
    pub expr: Rc<Expr<Ty>>,
}

pub type SchemaRef = Ref<Schema>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedName<Ty> {
    pub name: String,
    pub type_: Ty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedSchema {
    pub args: Option<Vec<BTreeMap<String, TypedNameAndExpr<MType>>>>,
    pub schema: SchemaRef,
}

// XXX We should implement a cheaper Eq / PartialEq over Schema, because it's
// currently used to check if two types are equal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Schema {
    pub folder: Option<String>,
    pub parent_scope: Option<Ref<Schema>>,
    pub next_placeholder: usize,
    pub externs: BTreeMap<String, Ref<MType>>,
    pub decls: BTreeMap<String, Decl>,
    pub imports: BTreeMap<ast::Path, Ref<ImportedSchema>>,
}

impl Schema {
    pub fn new(folder: Option<String>) -> Ref<Schema> {
        mkref(Schema {
            folder,
            parent_scope: None,
            next_placeholder: 1,
            externs: BTreeMap::new(),
            decls: BTreeMap::new(),
            imports: BTreeMap::new(),
        })
    }

    pub fn new_global_schema() -> Ref<Schema> {
        let ret = Schema::new(None);
        ret.borrow_mut().decls = BTreeMap::from([
            (
                "number".to_string(),
                Decl {
                    public: true,
                    extern_: false,
                    name: "number".to_string(),
                    value: SchemaEntry::Type(mkref(MType::Atom(AtomicType::Float64))),
                },
            ),
            (
                "string".to_string(),
                Decl {
                    public: true,
                    extern_: false,
                    name: "string".to_string(),
                    value: SchemaEntry::Type(mkref(MType::Atom(AtomicType::Utf8))),
                },
            ),
            (
                "bool".to_string(),
                Decl {
                    public: true,
                    extern_: false,
                    name: "string".to_string(),
                    value: SchemaEntry::Type(mkref(MType::Atom(AtomicType::Boolean))),
                },
            ),
            (
                "null".to_string(),
                Decl {
                    public: true,
                    extern_: false,
                    name: "string".to_string(),
                    value: SchemaEntry::Type(mkref(MType::Atom(AtomicType::Null))),
                },
            ),
            (
                "load_json".to_string(),
                Decl {
                    public: true,
                    extern_: false,
                    name: "load_json".to_string(),
                    value: SchemaEntry::Expr(mkref(STypedExpr {
                        type_: mkref(SType {
                            variables: BTreeSet::from(["R".to_string()]),
                            body: mkref(MType::Fn(MFnType {
                                args: vec![MField {
                                    name: "file".to_string(),
                                    type_: mkref(MType::Atom(AtomicType::Utf8)),
                                    nullable: false,
                                }],
                                ret: mkref(MType::List(mkref(MType::Name("R".to_string())))),
                            })),
                        }),
                        expr: Rc::new(Expr::NativeFn("load_json".to_string())),
                    })),
                },
            ),
        ]);

        ret
    }
}

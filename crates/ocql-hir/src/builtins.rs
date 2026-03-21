use crate::namespace::{ModuleNamespaces, PredicateInfo};
use crate::types::Type;
use crate::def::{DefId, DefKind, FileId, LocalDefId};
use crate::DefInfo;
use ocql_common::Span;
use ocql_ql_ast::ty::PrimitiveType;

/// Sentinel FileId for built-in definitions.
pub const BUILTIN_FILE: FileId = FileId(u32::MAX);

/// Allocator for builtin DefIds.
struct BuiltinAlloc {
    next: u32,
    defs: Vec<DefInfo>,
}

impl BuiltinAlloc {
    fn new() -> Self {
        Self {
            next: 0,
            defs: Vec::new(),
        }
    }

    fn alloc(&mut self, kind: DefKind, name: &str) -> DefId {
        let id = DefId {
            file: BUILTIN_FILE,
            local: LocalDefId(self.next),
        };
        self.next += 1;
        self.defs.push(DefInfo {
            id,
            kind,
            name: name.to_string(),
            span: Span::dummy(),
        });
        id
    }
}

/// Built-in type and predicate definitions.
pub struct Builtins {
    /// Namespace containing all built-in predicates and types.
    pub namespaces: ModuleNamespaces,
    /// Definitions for built-in entities.
    pub defs: Vec<DefInfo>,
}

impl Builtins {
    pub fn new() -> Self {
        let mut alloc = BuiltinAlloc::new();
        let mut ns = ModuleNamespaces::default();

        // Built-in string member predicates (result-returning)
        register_member_pred(&mut alloc, &mut ns, "string", "length", 0, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "string", "charAt", 1, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "indexOf", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "string", "substring", 2, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "toUpperCase", 0, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "toLowerCase", 0, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "trim", 0, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "replaceAll", 2, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "regexpMatch", 1, None); // predicate (boolean)
        register_member_pred(&mut alloc, &mut ns, "string", "regexpFind", 3, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "regexpReplaceAll", 2, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "regexpCapture", 2, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "splitAt", 1, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "splitAt", 2, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "prefix", 1, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "suffix", 1, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "matches", 1, None);
        register_member_pred(&mut alloc, &mut ns, "string", "toInt", 0, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "string", "toFloat", 0, Some(Type::Primitive(PrimitiveType::Float)));
        register_member_pred(&mut alloc, &mut ns, "string", "toString", 0, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "string", "isLowercase", 0, None);
        register_member_pred(&mut alloc, &mut ns, "string", "isUppercase", 0, None);

        // Built-in int member predicates
        register_member_pred(&mut alloc, &mut ns, "int", "toString", 0, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "int", "abs", 0, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "minimum", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "maximum", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "bitAnd", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "bitOr", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "bitXor", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "bitNot", 0, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "bitShiftLeft", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "bitShiftRight", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "bitShiftRightSigned", 1, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "int", "toFloat", 0, Some(Type::Primitive(PrimitiveType::Float)));

        // Built-in float member predicates
        register_member_pred(&mut alloc, &mut ns, "float", "toString", 0, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "float", "abs", 0, Some(Type::Primitive(PrimitiveType::Float)));
        register_member_pred(&mut alloc, &mut ns, "float", "floor", 0, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "float", "ceil", 0, Some(Type::Primitive(PrimitiveType::Int)));
        register_member_pred(&mut alloc, &mut ns, "float", "sqrt", 0, Some(Type::Primitive(PrimitiveType::Float)));
        register_member_pred(&mut alloc, &mut ns, "float", "log", 0, Some(Type::Primitive(PrimitiveType::Float)));
        register_member_pred(&mut alloc, &mut ns, "float", "minimum", 1, Some(Type::Primitive(PrimitiveType::Float)));
        register_member_pred(&mut alloc, &mut ns, "float", "maximum", 1, Some(Type::Primitive(PrimitiveType::Float)));

        // Built-in boolean member predicates
        register_member_pred(&mut alloc, &mut ns, "boolean", "toString", 0, Some(Type::Primitive(PrimitiveType::String)));
        register_member_pred(&mut alloc, &mut ns, "boolean", "booleanNot", 0, Some(Type::Primitive(PrimitiveType::Boolean)));
        register_member_pred(&mut alloc, &mut ns, "boolean", "booleanAnd", 1, Some(Type::Primitive(PrimitiveType::Boolean)));
        register_member_pred(&mut alloc, &mut ns, "boolean", "booleanOr", 1, Some(Type::Primitive(PrimitiveType::Boolean)));
        register_member_pred(&mut alloc, &mut ns, "boolean", "booleanXor", 1, Some(Type::Primitive(PrimitiveType::Boolean)));

        // Built-in global predicates
        register_global_pred(&mut alloc, &mut ns, "underlyingElement", 1, Some(Type::Error));
        register_global_pred(&mut alloc, &mut ns, "mkElement", 1, Some(Type::Error));
        register_global_pred(&mut alloc, &mut ns, "unreachable", 0, None);
        register_global_pred(&mut alloc, &mut ns, "none", 0, None);
        register_global_pred(&mut alloc, &mut ns, "any", 0, None);
        // toUrl: built-in URL construction predicate
        register_global_pred(&mut alloc, &mut ns, "toUrl", 5, Some(Type::Primitive(PrimitiveType::String)));
        register_global_pred(&mut alloc, &mut ns, "toUrl", 6, Some(Type::Primitive(PrimitiveType::String)));
        register_global_pred(&mut alloc, &mut ns, "unresolveElement", 1, Some(Type::Error));

        Self {
            namespaces: ns,
            defs: alloc.defs,
        }
    }
}

fn register_global_pred(
    alloc: &mut BuiltinAlloc,
    ns: &mut ModuleNamespaces,
    pred_name: &str,
    arity: usize,
    result_type: Option<Type>,
) {
    let id = alloc.alloc(DefKind::Predicate, pred_name);
    ns.predicates.insert(
        (pred_name.to_string(), arity),
        PredicateInfo {
            def_id: id,
            result_type,
            arity,
        },
    );
}

fn register_member_pred(
    alloc: &mut BuiltinAlloc,
    ns: &mut ModuleNamespaces,
    type_name: &str,
    pred_name: &str,
    arity: usize,
    result_type: Option<Type>,
) {
    let id = alloc.alloc(DefKind::MemberPredicate, &format!("{type_name}::{pred_name}"));
    let key = format!("{type_name}::{pred_name}");
    ns.predicates.insert(
        (key, arity),
        PredicateInfo {
            def_id: id,
            result_type,
            arity,
        },
    );
}

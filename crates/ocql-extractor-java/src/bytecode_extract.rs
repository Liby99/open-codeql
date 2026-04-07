//! Extracts relational facts from parsed Java class files into the database.
//!
//! This module mirrors what the source-level extractor (`extract.rs`) does for
//! `.java` files, but operates on parsed `.class` bytecode instead.  It populates
//! the same tables — `classes_or_interfaces`, `methods`, `constrs`, `fields`,
//! `params`, `modifiers`, `hasModifier`, `packages`, `declaresMember`,
//! `isInterface`, `extendsReftype`, `implInterface`, and `compiler_generated` —
//! so that QL queries work identically regardless of whether the input was source
//! or bytecode.

use std::collections::HashMap;

use ocql_database::{EntityId, Value};
use ocql_extractor_common::FactEmitter;

use crate::bytecode::{
    self, ClassFile, ACC_ABSTRACT, ACC_ANNOTATION, ACC_ENUM, ACC_FINAL, ACC_INTERFACE, ACC_NATIVE,
    ACC_PRIVATE, ACC_PROTECTED, ACC_PUBLIC, ACC_STATIC, ACC_STRICT, ACC_SYNCHRONIZED,
    ACC_SYNTHETIC, ACC_TRANSIENT, ACC_VOLATILE,
};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Extract facts from a parsed `ClassFile` into the database.
///
/// Returns the entity ID of the class/interface that was created (or looked up
/// from `type_cache` if it was already referenced as a supertype elsewhere).
///
/// # Caches
///
/// * `type_cache`     — maps fully-qualified internal names (e.g. `"java/lang/Object"`)
///                      to the entity ID already allocated for them.
/// * `modifier_cache` — maps modifier name strings (`"public"`, `"static"`, …) to
///                      the entity ID of the row in `modifiers`.
/// * `package_cache`  — maps dotted package names (`"java.lang"`) to entity IDs in
///                      the `packages` table.
pub fn extract_classfile(
    emitter: &mut FactEmitter<'_>,
    cf: &ClassFile,
    file_id: EntityId,
    type_cache: &mut HashMap<String, EntityId>,
    modifier_cache: &mut HashMap<String, EntityId>,
    package_cache: &mut HashMap<String, EntityId>,
) -> EntityId {
    // -- package --------------------------------------------------------
    let (pkg_name, simple_name) = split_class_name(&cf.this_class);
    let pkg_id = get_or_create_package(emitter, package_cache, &pkg_name);

    // -- class / interface ----------------------------------------------
    let class_id = get_or_create_type(emitter, type_cache, &cf.this_class, &simple_name, pkg_id);

    // Mark as interface if applicable
    if cf.access_flags & ACC_INTERFACE != 0 {
        emitter.emit("isInterface", vec![Value::Entity(class_id)]);
    }

    // Mark as enum type if applicable
    if cf.access_flags & ACC_ENUM != 0 {
        emitter.emit("isEnumType", vec![Value::Entity(class_id)]);
    }

    // Mark as annotation type if applicable
    if cf.access_flags & ACC_ANNOTATION != 0 {
        emitter.emit("isAnnotType", vec![Value::Entity(class_id)]);
    }

    // Modifiers on the class itself
    emit_access_modifiers(emitter, modifier_cache, class_id, cf.access_flags);

    // Synthetic
    if cf.access_flags & ACC_SYNTHETIC != 0 {
        emitter.emit(
            "compiler_generated",
            vec![Value::Entity(class_id), Value::Int(1)],
        );
    }

    // -- superclass -----------------------------------------------------
    if let Some(ref super_name) = cf.super_class {
        let (super_pkg, super_simple) = split_class_name(super_name);
        let super_pkg_id = get_or_create_package(emitter, package_cache, &super_pkg);
        let super_id =
            get_or_create_type(emitter, type_cache, super_name, &super_simple, super_pkg_id);
        emitter.emit(
            "extendsReftype",
            vec![Value::Entity(class_id), Value::Entity(super_id)],
        );
    }

    // -- interfaces -----------------------------------------------------
    let is_interface = cf.access_flags & ACC_INTERFACE != 0;
    for iface_name in &cf.interfaces {
        let (iface_pkg, iface_simple) = split_class_name(iface_name);
        let iface_pkg_id = get_or_create_package(emitter, package_cache, &iface_pkg);
        let iface_id =
            get_or_create_type(emitter, type_cache, iface_name, &iface_simple, iface_pkg_id);
        // Interfaces extend other interfaces (extendsReftype);
        // classes implement interfaces (implInterface).
        let table = if is_interface {
            "extendsReftype"
        } else {
            "implInterface"
        };
        emitter.emit(
            table,
            vec![Value::Entity(class_id), Value::Entity(iface_id)],
        );
    }

    // -- fields ---------------------------------------------------------
    for field in &cf.fields {
        let field_id = emitter.alloc();
        let field_type = descriptor_to_type_name(&field.descriptor);

        let name_val = emitter.string(&field.name);
        let type_val = emitter.string(&field_type);
        emitter.emit(
            "fields",
            vec![
                Value::Entity(field_id),
                name_val,
                type_val,
                Value::Entity(class_id),
            ],
        );

        emitter.emit(
            "declaresMember",
            vec![Value::Entity(class_id), Value::Entity(field_id)],
        );

        emit_access_modifiers(emitter, modifier_cache, field_id, field.access_flags);

        if field.access_flags & ACC_SYNTHETIC != 0 {
            emitter.emit(
                "compiler_generated",
                vec![Value::Entity(field_id), Value::Int(1)],
            );
        }
    }

    // -- methods & constructors -----------------------------------------
    for method in &cf.methods {
        // Skip class initializers — they are not user-visible
        if method.name == "<clinit>" {
            continue;
        }

        let is_constructor = method.name == "<init>";

        let param_types = parse_parameter_types(&method.descriptor);
        let return_type = parse_return_type(&method.descriptor);

        if is_constructor {
            // Constructor
            let constr_id = emitter.alloc();
            let sig = format!(
                "{}({})",
                simple_name,
                param_types
                    .iter()
                    .map(|t| descriptor_to_type_name(t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            let name_val = emitter.string(&simple_name);
            let sig_val = emitter.string(&sig);
            let type_val = emitter.string(&descriptor_to_type_name(&return_type));
            emitter.emit(
                "constrs",
                vec![
                    Value::Entity(constr_id),
                    name_val,
                    sig_val,
                    type_val,
                    Value::Entity(class_id),
                    Value::Entity(constr_id), // sourceid = self
                ],
            );

            emitter.emit(
                "declaresMember",
                vec![Value::Entity(class_id), Value::Entity(constr_id)],
            );

            emit_access_modifiers(emitter, modifier_cache, constr_id, method.access_flags);
            emit_parameters(emitter, modifier_cache, constr_id, &param_types);

            if method.access_flags & ACC_SYNTHETIC != 0 {
                emitter.emit(
                    "compiler_generated",
                    vec![Value::Entity(constr_id), Value::Int(1)],
                );
            }
        } else {
            // Regular method
            let method_id = emitter.alloc();
            let sig = format!(
                "{}({})",
                method.name,
                param_types
                    .iter()
                    .map(|t| descriptor_to_type_name(t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            let name_val = emitter.string(&method.name);
            let sig_val = emitter.string(&sig);
            let type_val = emitter.string(&descriptor_to_type_name(&return_type));
            emitter.emit(
                "methods",
                vec![
                    Value::Entity(method_id),
                    name_val,
                    sig_val,
                    type_val,
                    Value::Entity(class_id),
                    Value::Entity(method_id), // sourceid = self
                ],
            );

            emitter.emit(
                "declaresMember",
                vec![Value::Entity(class_id), Value::Entity(method_id)],
            );

            emit_access_modifiers(emitter, modifier_cache, method_id, method.access_flags);
            emit_parameters(emitter, modifier_cache, method_id, &param_types);

            if method.access_flags & ACC_SYNTHETIC != 0 {
                emitter.emit(
                    "compiler_generated",
                    vec![Value::Entity(method_id), Value::Int(1)],
                );
            }
        }
    }

    class_id
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Split a JVM internal class name like `"java/lang/String"` into
/// `("java.lang", "String")`.  A class in the default (unnamed) package
/// such as `"Foo"` yields `("", "Foo")`.
fn split_class_name(internal_name: &str) -> (String, String) {
    if let Some(pos) = internal_name.rfind('/') {
        let pkg = internal_name[..pos].replace('/', ".");
        let mut simple = internal_name[pos + 1..].to_string();
        // For inner classes like "OuterClass$InnerClass", use just "InnerClass"
        // to match CodeQL's naming convention.
        if let Some(dollar_pos) = simple.rfind('$') {
            let inner = &simple[dollar_pos + 1..];
            // Only strip if the inner part is a named class (not anonymous like "$1")
            if !inner.is_empty() && !inner.chars().next().unwrap_or('0').is_ascii_digit() {
                simple = inner.to_string();
            }
        }
        (pkg, simple)
    } else {
        (String::new(), internal_name.to_string())
    }
}

/// Get or create a `packages` row for the given dotted package name.
fn get_or_create_package(
    emitter: &mut FactEmitter<'_>,
    cache: &mut HashMap<String, EntityId>,
    pkg_name: &str,
) -> EntityId {
    if let Some(&id) = cache.get(pkg_name) {
        return id;
    }
    let id = emitter.alloc();
    let name_val = emitter.string(pkg_name);
    emitter.emit("packages", vec![Value::Entity(id), name_val]);
    cache.insert(pkg_name.to_string(), id);
    id
}

/// Get or create a `classes_or_interfaces` row for the given fully-qualified
/// internal class name.  If the type was already seen (via `type_cache`) the
/// existing entity ID is returned without emitting a duplicate row.
fn get_or_create_type(
    emitter: &mut FactEmitter<'_>,
    cache: &mut HashMap<String, EntityId>,
    internal_name: &str,
    simple_name: &str,
    pkg_id: EntityId,
) -> EntityId {
    if let Some(&id) = cache.get(internal_name) {
        return id;
    }
    let id = emitter.alloc();
    let name_val = emitter.string(simple_name);
    emitter.emit(
        "classes_or_interfaces",
        vec![
            Value::Entity(id),
            name_val,
            Value::Entity(pkg_id),
            Value::Entity(id), // sourceid = self
        ],
    );
    cache.insert(internal_name.to_string(), id);
    id
}

/// Emit `params` rows for each parameter type descriptor in `param_types`.
/// Each parameter gets a fresh entity ID, a 0-based index, and its type name.
fn emit_parameters(
    emitter: &mut FactEmitter<'_>,
    _modifier_cache: &mut HashMap<String, EntityId>,
    callable_id: EntityId,
    param_types: &[String],
) {
    for (index, desc) in param_types.iter().enumerate() {
        let param_id = emitter.alloc();
        let type_name = descriptor_to_type_name(desc);
        let type_val = emitter.string(&type_name);
        emitter.emit(
            "params",
            vec![
                Value::Entity(param_id),
                type_val,
                Value::Int(index as i64),
                Value::Entity(callable_id),
                Value::Entity(param_id), // sourceid = self
            ],
        );
    }
}

/// Emit `modifiers` / `hasModifier` rows for the given JVM access-flag bitmask.
/// Modifier entities are deduplicated via `modifier_cache`.
fn emit_access_modifiers(
    emitter: &mut FactEmitter<'_>,
    cache: &mut HashMap<String, EntityId>,
    element_id: EntityId,
    flags: u16,
) {
    static FLAG_TABLE: &[(u16, &str)] = &[
        (ACC_PUBLIC, "public"),
        (ACC_PRIVATE, "private"),
        (ACC_PROTECTED, "protected"),
        (ACC_STATIC, "static"),
        (ACC_FINAL, "final"),
        (ACC_ABSTRACT, "abstract"),
        (ACC_NATIVE, "native"),
        (ACC_SYNCHRONIZED, "synchronized"),
        (ACC_STRICT, "strictfp"),
        (ACC_VOLATILE, "volatile"),
        (ACC_TRANSIENT, "transient"),
    ];

    for &(flag, name) in FLAG_TABLE {
        if flags & flag != 0 {
            let mod_id = get_or_create_modifier(emitter, cache, name);
            emitter.emit(
                "hasModifier",
                vec![Value::Entity(element_id), Value::Entity(mod_id)],
            );
        }
    }
}

/// Get or create a `modifiers` row for the named modifier.
fn get_or_create_modifier(
    emitter: &mut FactEmitter<'_>,
    cache: &mut HashMap<String, EntityId>,
    name: &str,
) -> EntityId {
    if let Some(&id) = cache.get(name) {
        return id;
    }
    let id = emitter.alloc();
    let name_val = emitter.string(name);
    emitter.emit("modifiers", vec![Value::Entity(id), name_val]);
    cache.insert(name.to_string(), id);
    id
}

// ---------------------------------------------------------------------------
// JVM descriptor parsing
// ---------------------------------------------------------------------------

/// Convert a single JVM field/type descriptor to a human-readable type name.
///
/// Examples:
///   `"I"`                     → `"int"`
///   `"Ljava/lang/String;"`    → `"String"`
///   `"[I"`                    → `"int[]"`
///   `"[[Ljava/lang/Object;"` → `"Object[][]"`
///   `"V"`                     → `"void"`
fn descriptor_to_type_name(desc: &str) -> String {
    let mut array_depth = 0usize;
    let mut chars = desc.chars().peekable();

    // Count leading `[` for array dimensions
    while chars.peek() == Some(&'[') {
        array_depth += 1;
        chars.next();
    }

    let base = match chars.next() {
        Some('B') => "byte".to_string(),
        Some('C') => "char".to_string(),
        Some('D') => "double".to_string(),
        Some('F') => "float".to_string(),
        Some('I') => "int".to_string(),
        Some('J') => "long".to_string(),
        Some('S') => "short".to_string(),
        Some('Z') => "boolean".to_string(),
        Some('V') => "void".to_string(),
        Some('L') => {
            // Object type — collect until ';'
            let rest: String = chars.collect();
            let class_path = rest.trim_end_matches(';');
            // Use the simple name (last segment after '/')
            if let Some(pos) = class_path.rfind('/') {
                class_path[pos + 1..].to_string()
            } else {
                class_path.to_string()
            }
        }
        _ => "unknown".to_string(),
    };

    if array_depth == 0 {
        base
    } else {
        format!("{}{}", base, "[]".repeat(array_depth))
    }
}

/// Parse the parameter type descriptors from a JVM method descriptor.
///
/// A method descriptor has the form `(param_types)return_type`.
/// Returns a `Vec` of the raw descriptor strings for each parameter.
///
/// Example: `"(ILjava/lang/String;[D)V"` → `["I", "Ljava/lang/String;", "[D"]`
fn parse_parameter_types(descriptor: &str) -> Vec<String> {
    let mut result = Vec::new();

    // Find content between '(' and ')'
    let inner = match (descriptor.find('('), descriptor.find(')')) {
        (Some(open), Some(close)) if close > open + 1 => &descriptor[open + 1..close],
        _ => return result,
    };

    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        // Skip array dimensions
        while i < bytes.len() && bytes[i] == b'[' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        match bytes[i] {
            b'L' => {
                // Object type — scan until ';'
                if let Some(semi) = inner[i..].find(';') {
                    i += semi + 1;
                } else {
                    // Malformed — consume rest
                    i = bytes.len();
                }
            }
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' => {
                i += 1;
            }
            _ => {
                // Unknown — skip one byte to avoid infinite loop
                i += 1;
            }
        }
        result.push(inner[start..i].to_string());
    }

    result
}

/// Parse the return-type descriptor from a JVM method descriptor.
///
/// Example: `"(I)Ljava/lang/String;"` → `"Ljava/lang/String;"`
fn parse_return_type(descriptor: &str) -> String {
    match descriptor.find(')') {
        Some(pos) => descriptor[pos + 1..].to_string(),
        None => "V".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_class_name() {
        let (pkg, simple) = split_class_name("java/lang/String");
        assert_eq!(pkg, "java.lang");
        assert_eq!(simple, "String");

        let (pkg, simple) = split_class_name("Foo");
        assert_eq!(pkg, "");
        assert_eq!(simple, "Foo");

        let (pkg, simple) = split_class_name("com/example/nested/Inner");
        assert_eq!(pkg, "com.example.nested");
        assert_eq!(simple, "Inner");
    }

    #[test]
    fn test_descriptor_to_type_name() {
        assert_eq!(descriptor_to_type_name("I"), "int");
        assert_eq!(descriptor_to_type_name("Z"), "boolean");
        assert_eq!(descriptor_to_type_name("V"), "void");
        assert_eq!(descriptor_to_type_name("D"), "double");
        assert_eq!(descriptor_to_type_name("Ljava/lang/String;"), "String");
        assert_eq!(descriptor_to_type_name("Ljava/util/List;"), "List");
        assert_eq!(descriptor_to_type_name("[I"), "int[]");
        assert_eq!(descriptor_to_type_name("[[D"), "double[][]");
        assert_eq!(descriptor_to_type_name("[Ljava/lang/Object;"), "Object[]");
        assert_eq!(
            descriptor_to_type_name("[[Ljava/lang/String;"),
            "String[][]"
        );
        assert_eq!(descriptor_to_type_name("B"), "byte");
        assert_eq!(descriptor_to_type_name("C"), "char");
        assert_eq!(descriptor_to_type_name("F"), "float");
        assert_eq!(descriptor_to_type_name("J"), "long");
        assert_eq!(descriptor_to_type_name("S"), "short");
    }

    #[test]
    fn test_parse_parameter_types() {
        assert_eq!(parse_parameter_types("()V"), Vec::<String>::new());

        assert_eq!(parse_parameter_types("(I)V"), vec!["I"]);

        assert_eq!(
            parse_parameter_types("(ILjava/lang/String;[D)V"),
            vec!["I", "Ljava/lang/String;", "[D"]
        );

        assert_eq!(parse_parameter_types("(ZJ)I"), vec!["Z", "J"]);

        assert_eq!(
            parse_parameter_types("([[Ljava/lang/Object;I)V"),
            vec!["[[Ljava/lang/Object;", "I"]
        );

        // No params, object return
        assert_eq!(
            parse_parameter_types("()Ljava/lang/String;"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_parse_return_type() {
        assert_eq!(parse_return_type("()V"), "V");
        assert_eq!(parse_return_type("(I)Ljava/lang/String;"), "Ljava/lang/String;");
        assert_eq!(parse_return_type("(II)[I"), "[I");
    }
}

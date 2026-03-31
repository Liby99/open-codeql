//! Comprehensive integration tests for the C/C++ extractor.
//!
//! Each test file exercises a different set of C/C++ language features
//! and validates that the extractor populates the database correctly.

use ocql_database::Database;
use ocql_extractor_common::Extractor;
use ocql_extractor_cpp::{CppExtractor, cpp_schema};

/// Helper: extract a fixture file and return the database.
fn extract(filename: &str) -> Database {
    let path = format!("tests/fixtures/{}", filename);
    let source = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = if filename.ends_with(".c") {
        CppExtractor::c()
    } else {
        CppExtractor::cpp()
    };
    let result = extractor.extract_source(&mut db, filename, &source);
    assert!(result.success, "Extraction of {} failed: {:?}", filename, result.error);
    db
}

/// Helper: get all values in a column as strings.
fn column_strings(db: &Database, table: &str, col_idx: usize) -> Vec<String> {
    db.scan(table).unwrap()
        .map(|t| {
            match &t[col_idx] {
                ocql_database::Value::String(s) => db.strings.resolve(*s).to_string(),
                ocql_database::Value::Int(i) => i.to_string(),
                ocql_database::Value::Entity(e) => format!("@{}", e.0),
                other => format!("{:?}", other),
            }
        })
        .collect()
}

fn function_names(db: &Database) -> Vec<String> { column_strings(db, "functions", 1) }
fn type_names(db: &Database) -> Vec<String> { column_strings(db, "usertypes", 1) }
fn field_names(db: &Database) -> Vec<String> { column_strings(db, "fields", 2) }
fn var_names(db: &Database) -> Vec<String> { column_strings(db, "variables", 1) }
fn include_paths(db: &Database) -> Vec<String> { column_strings(db, "includes", 2) }
fn base_class_names(db: &Database) -> Vec<String> { column_strings(db, "derivations", 2) }
fn param_names(db: &Database) -> Vec<String> { column_strings(db, "params", 2) }

fn table_count(db: &Database, table: &str) -> usize {
    db.relation(table).map_or(0, |r| r.len())
}

// ============================================================
// simple.cpp — basic C++ (already tested in unit tests, quick check)
// ============================================================

#[test]
fn simple_cpp_functions() {
    let db = extract("simple.cpp");
    let names = function_names(&db);
    assert!(names.contains(&"factorial".into()));
    assert!(names.contains(&"main".into()));
    assert!(names.contains(&"~Animal".into()));
}

#[test]
fn simple_cpp_types() {
    let db = extract("simple.cpp");
    let names = type_names(&db);
    assert!(names.contains(&"Point".into()));
    assert!(names.contains(&"Animal".into()));
    assert!(names.contains(&"Dog".into()));
}

// ============================================================
// templates.cpp — class/function templates, specialization
// ============================================================

#[test]
fn templates_classes() {
    let db = extract("templates.cpp");
    let names = type_names(&db);
    eprintln!("[templates] Types: {:?}", names);
    assert!(names.contains(&"Stack".into()), "Should extract template class Stack");
    assert!(names.contains(&"Pair".into()), "Should extract template class Pair");
    assert!(names.contains(&"FixedArray".into()), "Should extract FixedArray");
}

#[test]
fn templates_functions() {
    let db = extract("templates.cpp");
    let names = function_names(&db);
    eprintln!("[templates] Functions: {:?}", names);
    assert!(names.contains(&"max_of".into()), "Should extract function template max_of");
    assert!(names.contains(&"test_templates".into()));
}

#[test]
fn templates_member_functions() {
    let db = extract("templates.cpp");
    let names = function_names(&db);
    // Stack's member functions
    assert!(names.contains(&"push".into()), "Should extract Stack::push");
    assert!(names.contains(&"pop".into()), "Should extract Stack::pop");
    assert!(names.contains(&"empty".into()), "Should extract Stack::empty");
}

#[test]
fn templates_fields() {
    let db = extract("templates.cpp");
    let names = field_names(&db);
    eprintln!("[templates] Fields: {:?}", names);
    assert!(names.contains(&"data_".into()), "Should extract Stack::data_");
    assert!(names.contains(&"first_".into()), "Should extract Pair::first_");
    assert!(names.contains(&"second_".into()), "Should extract Pair::second_");
}

// ============================================================
// namespaces.cpp — nested namespaces, anonymous namespaces
// ============================================================

#[test]
fn namespaces_functions() {
    let db = extract("namespaces.cpp");
    let names = function_names(&db);
    eprintln!("[namespaces] Functions: {:?}", names);
    assert!(names.contains(&"square".into()), "Should find math::square");
    assert!(names.contains(&"cube".into()), "Should find math::cube");
    assert!(names.contains(&"triangle_area".into()), "Should find geometry::triangle_area");
    assert!(names.contains(&"dot".into()), "Should find linear_algebra::dot");
    assert!(names.contains(&"cross".into()), "Should find linear_algebra::cross");
    assert!(names.contains(&"increment".into()), "Should find anonymous namespace::increment");
    assert!(names.contains(&"main".into()));
}

#[test]
fn namespaces_types() {
    let db = extract("namespaces.cpp");
    let names = type_names(&db);
    eprintln!("[namespaces] Types: {:?}", names);
    assert!(names.contains(&"Circle".into()), "Should find geometry::Circle");
    assert!(names.contains(&"Rectangle".into()), "Should find geometry::Rectangle");
    assert!(names.contains(&"Vec3".into()), "Should find linear_algebra::Vec3");
}

#[test]
fn namespaces_variables() {
    let db = extract("namespaces.cpp");
    let names = var_names(&db);
    eprintln!("[namespaces] Variables: {:?}", names);
    assert!(names.contains(&"pi".into()), "Should find math::pi");
    assert!(names.contains(&"internal_counter".into()), "Should find anonymous ns counter");
}

#[test]
fn namespaces_fields() {
    let db = extract("namespaces.cpp");
    let names = field_names(&db);
    assert!(names.contains(&"radius".into()), "Circle should have radius field");
    assert!(names.contains(&"width".into()), "Rectangle should have width field");
    assert!(names.contains(&"height".into()), "Rectangle should have height field");
    assert!(names.contains(&"x".into()), "Vec3 should have x field");
}

// ============================================================
// inheritance.cpp — single, multiple, virtual, diamond, nested
// ============================================================

#[test]
fn inheritance_types() {
    let db = extract("inheritance.cpp");
    let names = type_names(&db);
    eprintln!("[inheritance] Types: {:?}", names);
    assert!(names.contains(&"Shape".into()));
    assert!(names.contains(&"Drawable".into()));
    assert!(names.contains(&"Serializable".into()));
    assert!(names.contains(&"Circle".into()));
    assert!(names.contains(&"Rectangle".into()));
    assert!(names.contains(&"Base".into()));
    assert!(names.contains(&"Left".into()));
    assert!(names.contains(&"Right".into()));
    assert!(names.contains(&"Diamond".into()));
    assert!(names.contains(&"Container".into()));
    assert!(names.contains(&"Iterator".into()), "Nested class Iterator should be extracted");
}

#[test]
fn inheritance_base_classes() {
    let db = extract("inheritance.cpp");
    let bases = base_class_names(&db);
    eprintln!("[inheritance] Base classes: {:?}", bases);
    // Circle : Shape, Drawable, Serializable
    assert!(bases.contains(&"Shape".into()));
    assert!(bases.contains(&"Drawable".into()));
    assert!(bases.contains(&"Serializable".into()));
    // Diamond : Left, Right
    assert!(bases.contains(&"Left".into()));
    assert!(bases.contains(&"Right".into()));
    // Left/Right : virtual Base
    assert!(bases.contains(&"Base".into()));
}

#[test]
fn inheritance_virtual_functions() {
    let db = extract("inheritance.cpp");
    let names = function_names(&db);
    assert!(names.contains(&"area".into()), "Should extract virtual area()");
    assert!(names.contains(&"draw".into()), "Should extract virtual draw()");
    assert!(names.contains(&"serialize".into()), "Should extract serialize()");
    assert!(names.contains(&"~Shape".into()), "Should extract virtual destructor");
}

// ============================================================
// enums_unions.cpp — enums, scoped enums, unions, bitfields
// ============================================================

#[test]
fn enums_unions_types() {
    let db = extract("enums_unions.cpp");
    let names = type_names(&db);
    eprintln!("[enums_unions] Types: {:?}", names);
    assert!(names.contains(&"Color".into()), "Should extract C-style enum");
    assert!(names.contains(&"Direction".into()), "Should extract scoped enum");
    assert!(names.contains(&"Permissions".into()), "Should extract flags enum");
    assert!(names.contains(&"Value".into()), "Should extract union");
    assert!(names.contains(&"TaggedValue".into()), "Should extract tagged union");
    assert!(names.contains(&"PackedFlags".into()), "Should extract bitfields struct");
}

#[test]
fn enums_unions_functions() {
    let db = extract("enums_unions.cpp");
    let names = function_names(&db);
    assert!(names.contains(&"direction_name".into()));
    assert!(names.contains(&"blend".into()));
    assert!(names.contains(&"main".into()));
}

#[test]
fn enums_unions_fields() {
    let db = extract("enums_unions.cpp");
    let names = field_names(&db);
    eprintln!("[enums_unions] Fields: {:?}", names);
    // Value union fields
    assert!(names.contains(&"i".into()));
    assert!(names.contains(&"f".into()));
    assert!(names.contains(&"d".into()));
    // PackedFlags bitfields
    assert!(names.contains(&"readable".into()));
    assert!(names.contains(&"writable".into()));
    assert!(names.contains(&"executable".into()));
}

// ============================================================
// operator_overloading.cpp — operator overloads, friend functions
// ============================================================

#[test]
fn operators_type() {
    let db = extract("operator_overloading.cpp");
    let names = type_names(&db);
    assert!(names.contains(&"Complex".into()));
}

#[test]
fn operators_functions() {
    let db = extract("operator_overloading.cpp");
    let names = function_names(&db);
    eprintln!("[operators] Functions: {:?}", names);
    assert!(names.contains(&"main".into()));
    assert!(names.contains(&"real".into()), "Should extract Complex::real()");
    assert!(names.contains(&"imag".into()), "Should extract Complex::imag()");
    // Operator overloads should be extracted with their operator names
    assert!(names.iter().any(|n| n.contains("operator")),
        "Should extract at least one operator overload, got: {:?}", names);
}

#[test]
fn operators_fields() {
    let db = extract("operator_overloading.cpp");
    let names = field_names(&db);
    assert!(names.contains(&"real_".into()));
    assert!(names.contains(&"imag_".into()));
}

// ============================================================
// modern_cpp.cpp — lambdas, auto, constexpr, smart pointers
// ============================================================

#[test]
fn modern_cpp_functions() {
    let db = extract("modern_cpp.cpp");
    let names = function_names(&db);
    eprintln!("[modern_cpp] Functions: {:?}", names);
    assert!(names.contains(&"factorial".into()), "Should extract constexpr function");
    assert!(names.contains(&"apply".into()), "Should extract std::function param");
    assert!(names.contains(&"make_pair".into()), "Should extract auto return type");
    assert!(names.contains(&"divide".into()), "Should extract trailing return type");
    assert!(names.contains(&"main".into()));
}

#[test]
fn modern_cpp_types() {
    let db = extract("modern_cpp.cpp");
    let names = type_names(&db);
    eprintln!("[modern_cpp] Types: {:?}", names);
    assert!(names.contains(&"Buffer".into()), "Should extract Buffer class");
}

#[test]
fn modern_cpp_variables() {
    let db = extract("modern_cpp.cpp");
    let names = var_names(&db);
    eprintln!("[modern_cpp] Variables: {:?}", names);
    assert!(names.contains(&"FACT_10".into()), "Should extract constexpr variable");
}

#[test]
fn modern_cpp_includes() {
    let db = extract("modern_cpp.cpp");
    let paths = include_paths(&db);
    eprintln!("[modern_cpp] Includes: {:?}", paths);
    assert!(paths.iter().any(|p| p.contains("memory")));
    assert!(paths.iter().any(|p| p.contains("vector")));
    assert!(paths.iter().any(|p| p.contains("algorithm")));
    assert!(paths.iter().any(|p| p.contains("functional")));
}

// ============================================================
// data_structures.c — linked list, hash table, function pointers
// ============================================================

#[test]
fn data_structures_c_functions() {
    let db = extract("data_structures.c");
    let names = function_names(&db);
    eprintln!("[data_structures.c] Functions: {:?}", names);
    assert!(names.contains(&"list_create".into()));
    assert!(names.contains(&"list_push".into()));
    assert!(names.contains(&"list_pop".into()));
    assert!(names.contains(&"list_destroy".into()));
    assert!(names.contains(&"djb2_hash".into()));
    assert!(names.contains(&"table_create".into()));
    assert!(names.contains(&"table_put".into()));
    assert!(names.contains(&"table_get".into()));
    assert!(names.contains(&"table_destroy".into()));
    assert!(names.contains(&"op_add".into()));
    assert!(names.contains(&"op_sub".into()));
    assert!(names.contains(&"op_mul".into()));
    assert!(names.contains(&"op_div".into()));
    assert!(names.contains(&"main".into()));
}

#[test]
fn data_structures_c_types() {
    let db = extract("data_structures.c");
    let names = type_names(&db);
    eprintln!("[data_structures.c] Types: {:?}", names);
    assert!(names.contains(&"Node".into()));
    assert!(names.contains(&"LinkedList".into()));
    assert!(names.contains(&"HashEntry".into()));
    assert!(names.contains(&"HashTable".into()));
}

#[test]
fn data_structures_c_fields() {
    let db = extract("data_structures.c");
    let names = field_names(&db);
    eprintln!("[data_structures.c] Fields: {:?}", names);
    assert!(names.contains(&"data".into()), "Node should have data field");
    assert!(names.contains(&"next".into()), "Node should have next field");
    assert!(names.contains(&"head".into()), "LinkedList should have head field");
    assert!(names.contains(&"size".into()), "LinkedList should have size field");
    assert!(names.contains(&"key".into()), "HashEntry should have key field");
    assert!(names.contains(&"value".into()), "HashEntry should have value field");
}

#[test]
fn data_structures_c_params() {
    let db = extract("data_structures.c");
    let names = param_names(&db);
    eprintln!("[data_structures.c] Params: {:?}", names);
    assert!(names.contains(&"list".into()));
    assert!(names.contains(&"value".into()) || names.contains(&"value".into()));
    assert!(names.contains(&"key".into()));
}

// ============================================================
// header_only.h — header-only library, inline functions, templates
// ============================================================

#[test]
fn header_only_functions() {
    let db = extract("header_only.h");
    let names = function_names(&db);
    eprintln!("[header_only.h] Functions: {:?}", names);
    assert!(names.contains(&"clamp".into()));
    assert!(names.contains(&"lerp".into()));
    assert!(names.contains(&"min_val".into()));
    assert!(names.contains(&"max_val".into()));
}

#[test]
fn header_only_types() {
    let db = extract("header_only.h");
    let names = type_names(&db);
    eprintln!("[header_only.h] Types: {:?}", names);
    assert!(names.contains(&"Point2D".into()));
    assert!(names.contains(&"AABB".into()));
}

#[test]
fn header_only_fields() {
    let db = extract("header_only.h");
    let names = field_names(&db);
    assert!(names.contains(&"x".into()));
    assert!(names.contains(&"y".into()));
    assert!(names.contains(&"min".into()));
    assert!(names.contains(&"max".into()));
}

#[test]
fn header_only_includes() {
    let db = extract("header_only.h");
    let paths = include_paths(&db);
    assert!(paths.iter().any(|p| p.contains("cstddef")));
}

// ============================================================
// extern_c.cpp — extern "C" blocks, static functions, C++ wrapper
// ============================================================

#[test]
fn extern_c_functions() {
    let db = extract("extern_c.cpp");
    let names = function_names(&db);
    eprintln!("[extern_c] Functions: {:?}", names);
    assert!(names.contains(&"cbuffer_create".into()), "Should extract extern C functions");
    assert!(names.contains(&"cbuffer_destroy".into()));
    assert!(names.contains(&"cbuffer_append".into()));
    assert!(names.contains(&"validate_input".into()), "Should extract static helper");
    assert!(names.contains(&"main".into()));
}

#[test]
fn extern_c_types() {
    let db = extract("extern_c.cpp");
    let names = type_names(&db);
    eprintln!("[extern_c] Types: {:?}", names);
    assert!(names.contains(&"CBuffer".into()), "Should extract struct in extern C");
    assert!(names.contains(&"BufferWrapper".into()), "Should extract C++ wrapper class");
}

#[test]
fn extern_c_fields() {
    let db = extract("extern_c.cpp");
    let names = field_names(&db);
    // CBuffer fields
    assert!(names.contains(&"data".into()));
    assert!(names.contains(&"size".into()));
    assert!(names.contains(&"capacity".into()));
    // BufferWrapper field
    assert!(names.contains(&"buf_".into()));
}

// ============================================================
// edge_cases.cpp — forward decls, empty class, many params, etc.
// ============================================================

#[test]
fn edge_cases_functions() {
    let db = extract("edge_cases.cpp");
    let names = function_names(&db);
    eprintln!("[edge_cases] Functions: {:?}", names);
    assert!(names.contains(&"do_nothing".into()));
    assert!(names.contains(&"get_null".into()));
    assert!(names.contains(&"get_ref".into()));
    assert!(names.contains(&"many_params".into()));
    assert!(names.contains(&"register_callback".into()));
    assert!(names.contains(&"main".into()));
}

#[test]
fn edge_cases_types() {
    let db = extract("edge_cases.cpp");
    let names = type_names(&db);
    eprintln!("[edge_cases] Types: {:?}", names);
    assert!(names.contains(&"Empty".into()), "Should extract empty class");
    assert!(names.contains(&"OnlyCtors".into()));
    assert!(names.contains(&"MathUtils".into()));
    assert!(names.contains(&"Widget".into()));
}

#[test]
fn edge_cases_variables() {
    let db = extract("edge_cases.cpp");
    let names = var_names(&db);
    eprintln!("[edge_cases] Variables: {:?}", names);
    // Multiple vars on one line
    assert!(names.contains(&"a".into()));
    assert!(names.contains(&"b".into()));
    assert!(names.contains(&"c".into()));
    assert!(names.contains(&"MAGIC_NUMBER".into()));
    assert!(names.contains(&"static_counter".into()));
}

#[test]
fn edge_cases_many_params() {
    let db = extract("edge_cases.cpp");
    let params: Vec<_> = db.scan("params").unwrap().collect();
    // Find the many_params function's parameters
    let param_names: Vec<String> = params.iter()
        .map(|t| db.strings.resolve(t[2].as_string().unwrap()).to_string())
        .collect();
    eprintln!("[edge_cases] All params: {:?}", param_names);
    // many_params has 8 parameters (a through h)
    for ch in &["a", "b", "c", "d", "e", "f", "g", "h"] {
        assert!(param_names.contains(&ch.to_string()),
            "many_params should have param '{}', got {:?}", ch, param_names);
    }
}

#[test]
fn edge_cases_nested_namespace() {
    let db = extract("edge_cases.cpp");
    let names = function_names(&db);
    assert!(names.contains(&"deep_func".into()), "Should extract deeply nested function");
    let vars = var_names(&db);
    assert!(vars.contains(&"deep_var".into()), "Should extract deeply nested variable");
}

// ============================================================
// sqlite_like.c — realistic C: structs, callbacks, error handling
// ============================================================

#[test]
fn sqlite_like_functions() {
    let db = extract("sqlite_like.c");
    let names = function_names(&db);
    eprintln!("[sqlite_like.c] Functions: {:?}", names);
    assert!(names.contains(&"db_open".into()));
    assert!(names.contains(&"db_close".into()));
    assert!(names.contains(&"db_errmsg".into()));
    assert!(names.contains(&"db_create_table".into()));
    assert!(names.contains(&"db_insert".into()));
    assert!(names.contains(&"db_exec".into()));
    assert!(names.contains(&"print_row".into()));
    assert!(names.contains(&"main".into()));
}

#[test]
fn sqlite_like_types() {
    let db = extract("sqlite_like.c");
    let names = type_names(&db);
    eprintln!("[sqlite_like.c] Types: {:?}", names);
    assert!(names.contains(&"Column".into()));
    assert!(names.contains(&"Table".into()));
    assert!(names.contains(&"Database".into()));
    assert!(names.contains(&"ColumnType".into()), "Should extract C enum");
}

#[test]
fn sqlite_like_fields() {
    let db = extract("sqlite_like.c");
    let names = field_names(&db);
    eprintln!("[sqlite_like.c] Fields: {:?}", names);
    // Column fields
    assert!(names.contains(&"name".into()));
    assert!(names.contains(&"type".into()));
    assert!(names.contains(&"not_null".into()));
    assert!(names.contains(&"primary_key".into()));
    // Table fields
    assert!(names.contains(&"ncols".into()));
    assert!(names.contains(&"nrows".into()));
    assert!(names.contains(&"capacity".into()));
    // Database fields
    assert!(names.contains(&"path".into()));
    assert!(names.contains(&"ntables".into()));
    assert!(names.contains(&"is_open".into()));
}

#[test]
fn sqlite_like_params() {
    let db = extract("sqlite_like.c");
    let names = param_names(&db);
    eprintln!("[sqlite_like.c] Params: {:?}", names);
    assert!(names.contains(&"db".into()));
    assert!(names.contains(&"path".into()));
    assert!(names.contains(&"table_name".into()));
    assert!(names.contains(&"cb".into()));
    assert!(names.contains(&"user_data".into()));
}

// ============================================================
// New tables: fun_decls, fun_def, mangled_name, manglednames,
//             containerparent, numlines
// ============================================================

#[test]
fn simple_cpp_fun_decls_and_manglednames() {
    let db = extract("simple.cpp");
    let func_count = table_count(&db, "functions");
    let fun_decl_count = table_count(&db, "fun_decls");
    let fun_def_count = table_count(&db, "fun_def");
    let mangled_count = table_count(&db, "mangled_name");
    let manglednames_count = table_count(&db, "manglednames");

    eprintln!("[simple.cpp] functions={}, fun_decls={}, fun_def={}, mangled_name={}, manglednames={}",
        func_count, fun_decl_count, fun_def_count, mangled_count, manglednames_count);

    // Every function should have a fun_decl, fun_def, and mangled_name
    assert_eq!(fun_decl_count, func_count, "Each function should have a fun_decl");
    assert_eq!(fun_def_count, func_count, "Each function definition should have fun_def");
    assert_eq!(mangled_count, func_count, "Each function should have a mangled_name entry");
    assert_eq!(manglednames_count, func_count, "Each function should have a manglednames entry");
}

#[test]
fn simple_cpp_containerparent_and_numlines() {
    // Use a path with a directory component so containerparent gets emitted
    let path = "tests/fixtures/simple.cpp";
    let source = std::fs::read(path).unwrap();
    let schema = cpp_schema();
    let mut db = Database::from_schema(schema);
    let extractor = CppExtractor::cpp();
    let result = extractor.extract_source(&mut db, path, &source);
    assert!(result.success);

    let containerparent_count = table_count(&db, "containerparent");
    let numlines_count = table_count(&db, "numlines");

    eprintln!("[simple.cpp] containerparent={}, numlines={}", containerparent_count, numlines_count);

    // Should have at least one containerparent (folder -> file)
    assert!(containerparent_count >= 1, "Should have containerparent entries");
    // Should have numlines for the file
    assert!(numlines_count >= 1, "Should have numlines for the file");
}

// ============================================================
// Cross-cutting: location and count sanity checks
// ============================================================

#[test]
fn all_fixtures_have_locations() {
    let fixtures = [
        "simple.cpp", "simple.c", "templates.cpp", "namespaces.cpp",
        "inheritance.cpp", "enums_unions.cpp", "operator_overloading.cpp",
        "modern_cpp.cpp", "data_structures.c", "header_only.h",
        "extern_c.cpp", "edge_cases.cpp", "sqlite_like.c",
    ];
    for file in &fixtures {
        let db = extract(file);
        let loc_count = table_count(&db, "locations_default");
        let func_count = table_count(&db, "functions");
        let elem_loc_count = table_count(&db, "element_location");

        eprintln!("{}: {} locations, {} functions, {} element_locations",
            file, loc_count, func_count, elem_loc_count);

        assert!(loc_count > 0, "{} should have locations", file);
        assert!(func_count > 0, "{} should have functions", file);
        assert!(elem_loc_count > 0, "{} should have element_location entries", file);

        // Every location should have valid line numbers
        for loc in db.scan("locations_default").unwrap() {
            let line = loc[2].as_int().unwrap();
            assert!(line > 0, "{}: line number should be positive, got {}", file, line);
        }
    }
}

#[test]
fn extraction_counts_summary() {
    let fixtures = [
        "simple.cpp", "simple.c", "templates.cpp", "namespaces.cpp",
        "inheritance.cpp", "enums_unions.cpp", "operator_overloading.cpp",
        "modern_cpp.cpp", "data_structures.c", "header_only.h",
        "extern_c.cpp", "edge_cases.cpp", "sqlite_like.c",
    ];
    eprintln!("\n{:<30} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5}",
        "File", "Funcs", "Types", "Fields", "Vars", "Params", "Incl", "Locs");
    eprintln!("{}", "-".repeat(95));

    for file in &fixtures {
        let db = extract(file);
        eprintln!("{:<30} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5}",
            file,
            table_count(&db, "functions"),
            table_count(&db, "usertypes"),
            table_count(&db, "fields"),
            table_count(&db, "variables"),
            table_count(&db, "params"),
            table_count(&db, "includes"),
            table_count(&db, "locations_default"),
        );
    }
}

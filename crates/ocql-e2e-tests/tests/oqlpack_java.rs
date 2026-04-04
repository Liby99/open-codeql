//! End-to-end tests: compile our own Java oqlpack, extract Java code,
//! and evaluate queries using our translated QL library.
//!
//! This validates that the oqlpack/java/ library we translated from
//! vendor/codeql/java/ actually works with our pipeline.
//!
//! Run with: cargo test -p ocql-e2e-tests --test oqlpack_java -- --nocapture

use std::path::Path;
use std::collections::HashSet;

use ocql_database::{Database, Value};
use ocql_extractor_common::Extractor;
use ocql_extractor_java::{JavaExtractor, java_schema};

/// Shared compilation context: HIR for the oqlpack library.
struct OqlpackContext {
    hir: ocql_hir::HirDatabase,
}

impl OqlpackContext {
    fn compile_or_skip() -> Option<Self> {
        let workspace = Self::workspace();
        let oqlpack_lib = workspace.join("oqlpack/java/lib");
        if !oqlpack_lib.exists() {
            eprintln!("SKIP: oqlpack/java/lib not found");
            return None;
        }
        let hir = ocql_hir::analyze_project(&oqlpack_lib);
        eprintln!("  oqlpack: {} files, {} clean", hir.files.len(), hir.clean_file_count());
        Some(Self { hir })
    }

    fn workspace() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()  // crates/
            .parent().unwrap()  // workspace root
    }

    /// Run a query against the given Java fixture file.
    /// Returns the select_result rows as Vec<Vec<String>>.
    ///
    /// NOTE: The select_result format is (from_entity, select_expr1, select_expr2, ...).
    /// Column 0 is always the from-variable entity. Actual select expressions start at column 1.
    fn run_query(&self, query_source: &str, fixture: &str) -> QueryResult {
        let workspace = Self::workspace();

        // Parse query
        let query_ast = ocql_ql_parser::parse_source_file(query_source)
            .expect("query should parse");

        // MIR lowering: library + query
        let mut all_asts: Vec<&ocql_ql_ast::module::SourceFile> = self.hir.files.values()
            .map(|f| &f.ast)
            .collect();
        all_asts.push(&query_ast);
        let mir = ocql_mir::lower_source_files(&all_asts).expect("MIR lowering failed");

        // Engine emission
        let program = ocql_mir::emit_program_with_strings(&mir);

        // Extract fixture
        let java_path = workspace.join(format!("crates/ocql-e2e-tests/tests/fixtures/{}", fixture));
        let source = std::fs::read(&java_path)
            .unwrap_or_else(|e| panic!("Cannot read {}: {}", fixture, e));
        let schema = java_schema();
        let mut db = Database::from_schema(schema);
        let extractor = JavaExtractor::new();
        let result = extractor.extract_source(&mut db, java_path.to_str().unwrap(), &source);
        assert!(result.success, "Java extraction of {} failed: {:?}", fixture, result.error);

        // Evaluate
        let mut program2 = program;
        program2.resolve_strings(&mut db);
        ocql_engine::evaluate(&program2, &mut db)
            .unwrap_or_else(|e| panic!("Evaluation failed: {}", e));

        // Collect select_result rows
        let select_names: Vec<String> = db.relation_names()
            .filter(|n| n.starts_with("select_result"))
            .map(|n| n.to_string())
            .collect();

        let mut rows: Vec<Vec<String>> = Vec::new();
        for select_name in &select_names {
            if let Some(iter) = db.scan(select_name) {
                for row in iter {
                    let vals: Vec<String> = row.iter().map(|v| match v {
                        Value::String(s) => db.strings.resolve(*s).to_string(),
                        Value::Int(i) => i.to_string(),
                        Value::Entity(e) => format!("#{}", e.0),
                        _ => "?".to_string(),
                    }).collect();
                    rows.push(vals);
                }
            }
        }

        QueryResult { rows, db }
    }
}

struct QueryResult {
    rows: Vec<Vec<String>>,
    db: Database,
}

impl QueryResult {
    /// Get a sorted list of values from a specific column.
    fn column_sorted(&self, col: usize) -> Vec<String> {
        let mut vals: Vec<String> = self.rows.iter()
            .filter_map(|r| r.get(col).cloned())
            .collect();
        vals.sort();
        vals
    }

    /// Get a set of values from a specific column.
    fn column_set(&self, col: usize) -> HashSet<String> {
        self.rows.iter()
            .filter_map(|r| r.get(col).cloned())
            .collect()
    }

    /// Get the unique values from a specific column (sorted).
    fn column_unique_sorted(&self, col: usize) -> Vec<String> {
        let set = self.column_set(col);
        let mut vals: Vec<String> = set.into_iter().collect();
        vals.sort();
        vals
    }

    fn count(&self) -> usize {
        self.rows.len()
    }

    fn relation_count(&self, name: &str) -> usize {
        self.db.scan(name).map(|i| i.count()).unwrap_or(0)
    }
}

fn run_in_big_stack(f: impl FnOnce() + Send + 'static) {
    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let handle = builder.spawn(f).unwrap();
    handle.join().unwrap();
}

// =============================================================================
// Tests against Simple.java
// =============================================================================

/// Query: find all public methods and their declaring type.
/// Columns: [0]=entity, [1]=m.getName(), [2]=m.getDeclaringType().getName()
#[test]
fn oqlpack_java_public_methods() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
where m.isPublic()
select m.getName(), m.getDeclaringType().getName()
"#, "Simple.java");

        eprintln!("  public methods: {:?}", result.rows);
        assert_eq!(result.count(), 4, "Simple.java has 4 public methods");
        // col 1 = m.getName()
        let names = result.column_sorted(1);
        assert_eq!(names, vec!["add", "getValue", "helper", "main"]);
        // col 2 = m.getDeclaringType().getName() — all in "Simple"
        for row in &result.rows {
            assert_eq!(row[2], "Simple", "All methods declared in Simple");
        }
    });
}

/// Query: find static methods.
/// Columns: [0]=entity, [1]=m.getName()
#[test]
fn oqlpack_java_static_methods() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
where m.isStatic()
select m.getName()
"#, "Simple.java");

        eprintln!("  static methods: {:?}", result.rows);
        let names = result.column_sorted(1);
        assert_eq!(names, vec!["helper", "main"], "helper and main are static");
    });
}

/// Query: find constructors.
/// Columns: [0]=entity, [1]=c.getName(), [2]=c.getDeclaringType().getName()
#[test]
fn oqlpack_java_constructors() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Constructor c
select c.getName(), c.getDeclaringType().getName()
"#, "Simple.java");

        eprintln!("  constructors: {:?}", result.rows);
        assert_eq!(result.count(), 1, "Simple.java has 1 constructor");
        assert_eq!(result.rows[0][1], "Simple");
        assert_eq!(result.rows[0][2], "Simple");
    });
}

/// Query: find fields and their declaring type.
/// Columns: [0]=entity, [1]=f.getName(), [2]=f.getDeclaringType().getName()
#[test]
fn oqlpack_java_fields() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Field f
select f.getName(), f.getDeclaringType().getName()
"#, "Simple.java");

        eprintln!("  fields: {:?}", result.rows);
        assert_eq!(result.count(), 1, "Simple.java has 1 field");
        assert_eq!(result.rows[0][1], "value");
        assert_eq!(result.rows[0][2], "Simple");
    });
}

/// Query: find private fields.
/// Columns: [0]=entity, [1]=f.getName()
#[test]
fn oqlpack_java_private_fields() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Field f
where f.isPrivate()
select f.getName()
"#, "Simple.java");

        eprintln!("  private fields: {:?}", result.rows);
        assert_eq!(result.count(), 1);
        assert_eq!(result.rows[0][1], "value");
    });
}

/// Query: find callable signature strings.
/// Columns: [0]=entity, [1]=m.getName(), [2]=m.getSignature()
#[test]
fn oqlpack_java_callable_signatures() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
select m.getName(), m.getSignature()
"#, "Simple.java");

        eprintln!("  signatures: {:?}", result.rows);
        assert_eq!(result.count(), 4);
        // col 2 = signature
        let sigs = result.column_set(2);
        assert!(sigs.contains("getValue()"), "Should have getValue() sig");
        assert!(sigs.contains("add(int)"), "Should have add(int) sig");
        assert!(sigs.contains("helper(int, int)"), "Should have helper(int, int) sig");
    });
}

/// Query: find return statements.
/// Columns: [0]=entity, [1]=r.getEnclosingCallable().getName()
#[test]
fn oqlpack_java_return_stmts() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from ReturnStmt r
select r.getEnclosingCallable().getName()
"#, "Simple.java");

        eprintln!("  return stmts: {:?}", result.rows);
        // col 1 = callable name
        let names = result.column_set(1);
        assert!(names.contains("getValue"), "getValue has return");
        assert!(names.contains("add"), "add has return");
        assert!(names.contains("helper"), "helper has return");
    });
}

/// Query: check Element#hasName.
/// Columns: [0]=entity, [1]=m.getName(), [2]=declaring type name
#[test]
fn oqlpack_java_element_has_name() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
where m.hasName("main")
select m.getName(), m.getDeclaringType().getName()
"#, "Simple.java");

        eprintln!("  main method: {:?}", result.rows);
        assert_eq!(result.count(), 1);
        assert_eq!(result.rows[0][1], "main");
        assert_eq!(result.rows[0][2], "Simple");
    });
}

// =============================================================================
// Tests against Shapes.java (richer fixture)
// =============================================================================

/// Query: find all classes. Class#char should include all concrete/abstract classes.
/// Columns: [0]=entity, [1]=c.getName()
#[test]
fn oqlpack_java_shapes_classes() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Class c
select c.getName()
"#, "Shapes.java");

        eprintln!("  classes: {:?}", result.rows);
        // col 1 = class name
        let names = result.column_set(1);
        eprintln!("  unique class names: {:?}", names);
        assert!(names.contains("Shape"), "Should find Shape class");
        assert!(names.contains("Circle"), "Should find Circle class");
        assert!(names.contains("Rectangle"), "Should find Rectangle class");
        assert!(names.contains("Canvas"), "Should find Canvas class");
    });
}

/// Query: find interfaces.
/// Columns: [0]=entity, [1]=i.getName()
#[test]
fn oqlpack_java_shapes_interfaces() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Interface i
select i.getName()
"#, "Shapes.java");

        eprintln!("  interfaces: {:?}", result.rows);
        // col 1 = interface name
        let names = result.column_set(1);
        assert!(names.contains("Drawable"), "Should find Drawable interface");
    });
}

/// Query: find abstract methods.
/// Columns: [0]=entity, [1]=m.getName(), [2]=m.getDeclaringType().getName()
#[test]
fn oqlpack_java_shapes_abstract_methods() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
where m.isAbstract()
select m.getName(), m.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  abstract methods: {:?}", result.rows);
        // col 1 = method name
        let names = result.column_set(1);
        assert!(names.contains("area"), "Shape.area() is abstract");
    });
}

/// Query: find private fields across all classes.
/// Columns: [0]=entity, [1]=f.getName(), [2]=f.getDeclaringType().getName()
#[test]
fn oqlpack_java_shapes_private_fields() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Field f
where f.isPrivate()
select f.getName(), f.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  private fields: {:?}", result.rows);
        assert!(result.count() >= 5, "Should find >=5 private fields");
        // col 1 = field name
        let field_names = result.column_set(1);
        assert!(field_names.contains("name"), "Shape.name is private");
        assert!(field_names.contains("radius"), "Circle.radius is private");
        assert!(field_names.contains("width"), "Rectangle.width is private");
        assert!(field_names.contains("height"), "Rectangle.height is private");
        assert!(field_names.contains("shapes"), "Canvas.shapes is private");
        assert!(field_names.contains("count"), "Canvas.count is private");
    });
}

/// Query: find static methods.
/// Columns: [0]=entity, [1]=m.getName(), [2]=m.getDeclaringType().getName()
#[test]
fn oqlpack_java_shapes_static_methods() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
where m.isStatic()
select m.getName(), m.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  static methods: {:?}", result.rows);
        // col 1 = method name
        let names = result.column_set(1);
        assert!(names.contains("square"), "Rectangle.square is static");
        assert!(names.contains("main"), "Canvas.main is static");
    });
}

/// Query: find constructors across multiple classes.
/// Columns: [0]=entity, [1]=c.getDeclaringType().getName()
#[test]
fn oqlpack_java_shapes_constructors() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Constructor c
select c.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  constructors: {:?}", result.rows);
        assert!(result.count() >= 4, "Should find >= 4 constructors");
        // col 1 = declaring type name
        let classes = result.column_set(1);
        assert!(classes.contains("Shape"), "Shape has constructor");
        assert!(classes.contains("Circle"), "Circle has constructor");
        assert!(classes.contains("Rectangle"), "Rectangle has constructor");
        assert!(classes.contains("Canvas"), "Canvas has constructor");
    });
}

/// Query: find all methods and their declaring class.
/// Columns: [0]=entity, [1]=m.getName(), [2]=m.getDeclaringType().getName()
#[test]
fn oqlpack_java_shapes_methods_per_class() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
select m.getName(), m.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  all methods: {:?}", result.rows);
        // Count methods per class using col 1=name, col 2=class
        let mut per_class: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for row in &result.rows {
            if row.len() >= 3 {
                per_class.entry(row[2].clone()).or_default().push(row[1].clone());
            }
        }
        eprintln!("  methods per class: {:?}", per_class);

        // Shape: getName, area, move, isAt = 4
        assert!(per_class.get("Shape").map(|v| v.len()).unwrap_or(0) >= 4,
            "Shape should have >= 4 methods, got: {:?}", per_class.get("Shape"));
        // Circle: area, getRadius, draw = 3
        assert!(per_class.get("Circle").map(|v| v.len()).unwrap_or(0) >= 3,
            "Circle should have >= 3 methods, got: {:?}", per_class.get("Circle"));
        // Canvas: addShape, getCount, totalArea, main = 4
        assert!(per_class.get("Canvas").map(|v| v.len()).unwrap_or(0) >= 4,
            "Canvas should have >= 4 methods, got: {:?}", per_class.get("Canvas"));
    });
}

/// Query: verify #char population for the shapes fixture.
#[test]
fn oqlpack_java_shapes_char_preds() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
select m.getName()
"#, "Shapes.java");

        let method_char = result.relation_count("Method#char");
        let field_char = result.relation_count("Field#char");
        let class_char = result.relation_count("Class#char");
        let callable_char = result.relation_count("Callable#char");
        let member_char = result.relation_count("Member#char");
        let stmt_char = result.relation_count("Stmt#char");
        let expr_char = result.relation_count("Expr#char");
        let constructor_char = result.relation_count("Constructor#char");
        let interface_char = result.relation_count("Interface#char");
        let parameter_char = result.relation_count("Parameter#char");

        eprintln!("  Method#char: {}", method_char);
        eprintln!("  Constructor#char: {}", constructor_char);
        eprintln!("  Field#char: {}", field_char);
        eprintln!("  Class#char: {}", class_char);
        eprintln!("  Interface#char: {}", interface_char);
        eprintln!("  Callable#char: {}", callable_char);
        eprintln!("  Member#char: {}", member_char);
        eprintln!("  Parameter#char: {}", parameter_char);
        eprintln!("  Stmt#char: {}", stmt_char);
        eprintln!("  Expr#char: {}", expr_char);

        let literal_char = result.relation_count("Literal#char");
        let string_lit_char = result.relation_count("StringLiteral#char");
        let namestrings_count = result.relation_count("namestrings");
        eprintln!("  Literal#char: {}", literal_char);
        eprintln!("  StringLiteral#char: {}", string_lit_char);
        eprintln!("  namestrings: {}", namestrings_count);

        assert!(method_char > 0, "Method#char should be populated");
        assert!(field_char > 0, "Field#char should be populated");
        assert!(class_char > 0, "Class#char should be populated");
        assert!(callable_char > 0, "Callable#char should be populated");
        assert!(member_char > 0, "Member#char should be populated");
        assert!(stmt_char > 0, "Stmt#char should be populated");
        assert!(expr_char > 0, "Expr#char should be populated");
    });
}

/// Query: find methods by specific name using hasName.
/// Columns: [0]=entity, [1]=m.getName(), [2]=m.getDeclaringType().getName()
#[test]
fn oqlpack_java_shapes_find_draw() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
where m.hasName("draw")
select m.getName(), m.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  draw methods: {:?}", result.rows);
        // Circle.draw(), Rectangle.draw(), and Drawable.draw()
        assert!(result.count() >= 2, "At least Circle.draw and Rectangle.draw");
        let classes = result.column_set(2);
        assert!(classes.contains("Circle"), "Circle has draw()");
        assert!(classes.contains("Rectangle"), "Rectangle has draw()");
    });
}

/// Query: find protected fields.
/// Columns: [0]=entity, [1]=f.getName(), [2]=f.getDeclaringType().getName()
#[test]
fn oqlpack_java_shapes_protected_fields() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Field f
where f.isProtected()
select f.getName(), f.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  protected fields: {:?}", result.rows);
        // Shape.x and Shape.y are protected
        let field_names = result.column_set(1);
        assert!(field_names.contains("x"), "Shape.x is protected");
        assert!(field_names.contains("y"), "Shape.y is protected");
    });
}

/// Query: find all methods — just names.
/// Columns: [0]=entity, [1]=m.getName()
#[test]
fn oqlpack_java_shapes_all_method_names() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
select m.getName()
"#, "Shapes.java");

        eprintln!("  all method names: {:?}", result.column_unique_sorted(1));
        let names = result.column_set(1);
        // Verify key methods exist
        assert!(names.contains("draw"), "draw exists");
        assert!(names.contains("area"), "area exists");
        assert!(names.contains("getName"), "getName exists");
        assert!(names.contains("move"), "move exists");
        assert!(names.contains("isAt"), "isAt exists");
        assert!(names.contains("getRadius"), "getRadius exists");
        assert!(names.contains("getWidth"), "getWidth exists");
        assert!(names.contains("getHeight"), "getHeight exists");
        assert!(names.contains("addShape"), "addShape exists");
        assert!(names.contains("getCount"), "getCount exists");
        assert!(names.contains("totalArea"), "totalArea exists");
        assert!(names.contains("square"), "square exists");
        assert!(names.contains("main"), "main exists");
    });
}

/// Query: find return statements in the shapes fixture.
/// Columns: [0]=entity, [1]=r.getEnclosingCallable().getName()
#[test]
fn oqlpack_java_shapes_return_stmts() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from ReturnStmt r
select r.getEnclosingCallable().getName()
"#, "Shapes.java");

        eprintln!("  return stmts: {:?}", result.rows);
        // col 1 = callable name
        let names = result.column_set(1);
        // Methods with return statements: getName, area(x3), getRadius, getWidth, getHeight,
        // getCount, square
        assert!(names.contains("getName"), "getName has return");
        assert!(names.contains("area"), "area has return");
        assert!(names.contains("getRadius"), "getRadius has return");
        assert!(names.contains("getCount"), "getCount has return");
    });
}

/// Query: find block statements (basic statement hierarchy test).
/// Columns: [0]=entity, [1]=b.getEnclosingCallable().getName()
#[test]
fn oqlpack_java_shapes_block_stmts() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from BlockStmt b
select b.getEnclosingCallable().getName()
"#, "Shapes.java");

        eprintln!("  block stmts: {:?}", result.rows);
        // Every method/constructor body is a block statement
        let names = result.column_set(1);
        assert!(names.len() >= 3, "Should have blocks in multiple methods");
        // CodeQL finds 21 blocks in Shapes.java (includes nested blocks for if/for)
        assert_eq!(result.count(), 21, "Should have 21 block statements");
    });
}

// =============================================================================
// Expression-level tests (comparing with CodeQL results)
// =============================================================================

/// Query: find if statements.
/// CodeQL finds 1 if statement in Shapes.java (in Canvas.addShape)
#[test]
fn oqlpack_java_shapes_if_stmts() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from IfStmt s
select s.getEnclosingCallable().getName()
"#, "Shapes.java");

        eprintln!("  if stmts: {:?}", result.rows);
        assert_eq!(result.count(), 1, "Canvas.addShape has 1 if statement");
        assert_eq!(result.rows[0][1], "addShape");
    });
}

/// Query: find for statements.
/// CodeQL finds 1 for statement in Shapes.java (in Canvas.totalArea)
#[test]
fn oqlpack_java_shapes_for_stmts() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from ForStmt s
select s.getEnclosingCallable().getName()
"#, "Shapes.java");

        eprintln!("  for stmts: {:?}", result.rows);
        assert_eq!(result.count(), 1, "Canvas.totalArea has 1 for statement");
        assert_eq!(result.rows[0][1], "totalArea");
    });
}

/// Query: find string literals.
/// CodeQL finds 3: "Circle", "Rectangle", "Total area: "
#[test]
fn oqlpack_java_shapes_string_literals() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from StringLiteral s
select s.getValue()
"#, "Shapes.java");

        eprintln!("  string literals: {:?}", result.rows);
        let values = result.column_set(1);
        assert!(values.contains("Circle"), "Should find 'Circle' literal");
        assert!(values.contains("Rectangle"), "Should find 'Rectangle' literal");
    });
}

/// Query: find assign expressions.
/// CodeQL finds 10 in Shapes.java
#[test]
fn oqlpack_java_shapes_assign_exprs() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from AssignExpr a
select a.getEnclosingCallable().getName()
"#, "Shapes.java");

        eprintln!("  assign exprs: {:?}", result.rows);
        let names = result.column_set(1);
        // Assignments in constructors (this.x = x, etc.) and addShape
        assert!(names.contains("Shape"), "Shape constructor has assignments");
        assert!(names.contains("Canvas"), "Canvas constructor has assignments");
        assert!(names.contains("addShape"), "addShape has assignments");
    });
}

/// Verify interface dedup: Interface query should return exactly 1 Drawable.
/// (Regression test: previously returned 2 due to extract_implements creating stubs.)
#[test]
fn oqlpack_java_shapes_interface_no_duplicates() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Interface i
select i.getName()
"#, "Shapes.java");

        eprintln!("  interfaces (dedup check): {:?}", result.rows);
        assert_eq!(result.count(), 1, "Should find exactly 1 Drawable (no duplicates)");
        assert_eq!(result.rows[0][1], "Drawable");
    });
}

/// Verify abstract methods include interface methods (JLS 9.4).
/// CodeQL finds: draw (Drawable, implicitly abstract) and area (Shape, explicitly abstract)
#[test]
fn oqlpack_java_shapes_abstract_includes_interface() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
where m.isAbstract()
select m.getName(), m.getDeclaringType().getName()
"#, "Shapes.java");

        eprintln!("  abstract methods (incl interface): {:?}", result.rows);
        assert_eq!(result.count(), 2, "Should find 2 abstract methods");
        let names = result.column_set(1);
        assert!(names.contains("draw"), "Drawable.draw() is implicitly abstract");
        assert!(names.contains("area"), "Shape.area() is explicitly abstract");
    });
}

/// Query: verify methods per class counts match CodeQL exactly.
/// CodeQL: Canvas=4, Circle=3, Rectangle=5, Shape=4
#[test]
fn oqlpack_java_shapes_method_counts_match_codeql() {
    run_in_big_stack(|| {
        let ctx = match OqlpackContext::compile_or_skip() { Some(c) => c, None => return };
        let result = ctx.run_query(r#"
import java
from Method m
select m.getName(), m.getDeclaringType().getName()
"#, "Shapes.java");

        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for row in &result.rows {
            if row.len() >= 3 {
                *counts.entry(row[2].clone()).or_default() += 1;
            }
        }
        eprintln!("  method counts: {:?}", counts);

        // Match CodeQL exactly
        assert_eq!(counts.get("Canvas"), Some(&4), "Canvas: addShape, getCount, totalArea, main");
        assert_eq!(counts.get("Circle"), Some(&3), "Circle: area, getRadius, draw");
        assert_eq!(counts.get("Rectangle"), Some(&5), "Rectangle: area, getWidth, getHeight, draw, square");
        assert_eq!(counts.get("Shape"), Some(&4), "Shape: getName, area, move, isAt");
    });
}

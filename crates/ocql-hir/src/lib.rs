pub mod builtins;
pub mod class_hierarchy;
pub mod collect;
pub mod def;
pub mod diagnostics;
pub mod module_graph;
pub mod namespace;
pub mod project;
pub mod resolve;
pub mod source;
pub mod types;

pub use def::{DefId, DefKind, FileId, LocalDefId};
pub use diagnostics::{Diagnostic, DiagnosticNote, Severity};
pub use namespace::ModuleNamespaces;
pub use resolve::ResolvedRef;
pub use types::Type;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use class_hierarchy::{ClassHierarchy, ClassInfo};
use collect::DeclarationCollector;
use module_graph::{ImportEdge, ModuleGraph, UnresolvedImport};
use namespace::PredicateInfo;
use ocql_common::Span;
use ocql_ql_ast::module::{ClassMember, SourceFile};
use project::ProjectIndex;
use source::SourceManager;

/// Per-file analysis results layered on top of the AST.
pub struct FileAnalysis {
    /// The parsed AST (owned).
    pub ast: SourceFile,

    /// DefIds assigned to declarations in this file.
    pub defs: Vec<DefInfo>,

    /// Name resolution: maps reference spans → resolved target.
    pub name_resolution: Vec<(Span, ResolvedRef)>,

    /// Type of each expression, keyed by span.
    pub expr_types: Vec<(Span, Type)>,

    /// Diagnostics produced during analysis of this file.
    pub diagnostics: Vec<Diagnostic>,
}

/// A definition registered during declaration collection.
pub struct DefInfo {
    pub id: DefId,
    pub kind: DefKind,
    pub name: String,
    pub span: Span,
}

/// The central result of HIR analysis.
pub struct HirDatabase {
    /// Source manager (owns all source text, provides line/col mapping).
    pub sources: SourceManager,
    /// Module import graph.
    pub module_graph: ModuleGraph,
    /// Per-file analysis results.
    pub files: HashMap<FileId, FileAnalysis>,
    /// Global diagnostics (not tied to a specific file).
    pub diagnostics: Vec<Diagnostic>,
}

impl HirDatabase {
    /// Iterate all error diagnostics across all files and global diagnostics.
    pub fn all_errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .chain(self.files.values().flat_map(|f| f.diagnostics.iter()))
            .filter(|d| d.severity == Severity::Error)
    }

    /// Returns true if there are no errors.
    pub fn is_ok(&self) -> bool {
        self.all_errors().next().is_none()
    }

    /// Count total errors.
    pub fn error_count(&self) -> usize {
        self.all_errors().count()
    }

    /// Count files that analyzed without errors.
    pub fn clean_file_count(&self) -> usize {
        self.files
            .values()
            .filter(|f| !f.diagnostics.iter().any(|d| d.severity == Severity::Error))
            .count()
    }

    /// Format a diagnostic with source context.
    pub fn format_diagnostic(&self, diag: &Diagnostic) -> String {
        self.sources.format_diagnostic(diag)
    }
}

// ---------------------------------------------------------------------------
// Single-file analysis (backward compat, tests)
// ---------------------------------------------------------------------------

/// Analyze a single QL source file.
pub fn analyze_single_file(source: &str, path: &str) -> HirDatabase {
    let mut sources = SourceManager::new();
    let file_id = sources.add_file(PathBuf::from(path), source.to_string());

    let ast = match ocql_ql_parser::parse_source_file(source) {
        Ok(ast) => ast,
        Err(err) => {
            let diag = Diagnostic {
                severity: Severity::Error,
                message: format!("Parse error: {err:?}"),
                span: Span::dummy(),
                file: file_id,
                notes: vec![],
            };
            return HirDatabase {
                sources,
                module_graph: ModuleGraph::new(),
                files: HashMap::new(),
                diagnostics: vec![diag],
            };
        }
    };

    let mut collector = DeclarationCollector::new(file_id);
    collector.collect_source_file(&ast);

    let empty1 = ModuleNamespaces::default();
    let empty2 = ModuleNamespaces::default();
    let mut resolver =
        resolve::NameResolver::new_single_file(file_id, &collector, &empty1, &empty2);
    resolver.resolve_source_file(&ast);
    let name_resolution = std::mem::take(&mut resolver.name_resolutions);
    let expr_types = std::mem::take(&mut resolver.expr_types);
    let diagnostics = std::mem::take(&mut resolver.diagnostics);
    drop(resolver);

    let analysis = FileAnalysis {
        ast,
        defs: collector.into_defs(),
        name_resolution,
        expr_types,
        diagnostics,
    };

    let mut files = HashMap::new();
    files.insert(file_id, analysis);

    HirDatabase {
        sources,
        module_graph: ModuleGraph::new(),
        files,
        diagnostics: vec![],
    }
}

// ---------------------------------------------------------------------------
// Multi-file project analysis
// ---------------------------------------------------------------------------

/// Analyze all .ql/.qll files under a workspace root.
///
/// This is the main entry point for analyzing a CodeQL project.
/// It discovers all qlpacks, parses all files, resolves imports,
/// and runs name resolution + type checking on each file.
pub fn analyze_project(workspace_root: &Path) -> HirDatabase {
    let project = ProjectIndex::discover(workspace_root);
    let mut builtins = builtins::Builtins::new();

    let mut sources = SourceManager::new();
    let mut module_graph = ModuleGraph::new();
    let mut global_diags = Vec::new();

    // Phase 1: Parse all files
    let mut parsed: HashMap<FileId, SourceFile> = HashMap::new();
    let mut path_to_id: HashMap<PathBuf, FileId> = HashMap::new();
    let mut parse_failures = 0u32;

    let mut all_paths: Vec<_> = project.all_files().cloned().collect();
    all_paths.sort();
    for path in &all_paths {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let file_id = sources.add_file(path.clone(), source.clone());
        path_to_id.insert(path.clone(), file_id);

        match ocql_ql_parser::parse_source_file(&source) {
            Ok(ast) => {
                parsed.insert(file_id, ast);
            }
            Err(_) => {
                parse_failures += 1;
            }
        }
    }

    if parse_failures > 0 {
        global_diags.push(Diagnostic {
            severity: Severity::Warning,
            message: format!("{parse_failures} files failed to parse"),
            span: Span::dummy(),
            file: FileId(0),
            notes: vec![],
        });
    }

    // Phase 2: Build module graph (resolve imports)
    for (&file_id, ast) in &parsed {
        let file_path = sources.path(file_id).to_path_buf();
        for member in &ast.members {
            if let ocql_ql_ast::module::ModuleMember::Import(import) = member {
                let import_path: Vec<String> = import.path.parts.clone();
                let is_private = import
                    .annotations
                    .iter()
                    .any(|a| a.kind == ocql_ql_ast::annotation::AnnotationKind::Private);

                match project.resolve_import(&import_path, &file_path) {
                    Some(resolved_path) => {
                        if let Some(&target_id) = path_to_id.get(&resolved_path) {
                            module_graph.add_edge(
                                file_id,
                                ImportEdge {
                                    target: target_id,
                                    target_path: resolved_path,
                                    import_path: import_path.clone(),
                                    is_private,
                                    alias: import.alias.as_ref().map(|a| a.name.clone()),
                                    span: import.span,
                                },
                            );
                        } else {
                            // File exists on disk but wasn't parsed (parse failure)
                            module_graph.add_unresolved(UnresolvedImport {
                                from_file: file_id,
                                import_path,
                                span: import.span,
                            });
                        }
                    }
                    None => {
                        // Check if this is a local module reference (e.g., `import Cached`
                        // where Cached is a module defined in the same file).
                        let is_local_module = import_path.len() == 1 && ast.members.iter().any(|m| {
                            matches!(m, ocql_ql_ast::module::ModuleMember::Module(md) if md.name.name == import_path[0])
                        });
                        if !is_local_module {
                            module_graph.add_unresolved(UnresolvedImport {
                                from_file: file_id,
                                import_path,
                                span: import.span,
                            });
                        }
                    }
                }
            }
        }
    }


    // Phase 2b: Compute topological order
    let mut all_file_ids: Vec<FileId> = parsed.keys().copied().collect();
    all_file_ids.sort_by_key(|id| id.0);
    module_graph.compute_topo_order(&all_file_ids);

    // Phase 3: Declaration collection (all files)
    let mut collectors: HashMap<FileId, DeclarationCollector> = HashMap::new();
    for (&file_id, ast) in &parsed {
        let mut collector = DeclarationCollector::new(file_id);
        collector.collect_source_file(ast);
        collectors.insert(file_id, collector);
    }

    // Phase 3b: Load database predicates from .dbscheme files
    let mut db_ns = ModuleNamespaces::default();
    for dbscheme_path in project.dbscheme_paths() {
        if let Ok(content) = std::fs::read_to_string(&dbscheme_path) {
            if let Ok(schema) = ocql_schema::parse_dbscheme(&content) {
                for table in schema.tables() {
                    let arity = table.columns.len();
                    let db_def = DefId {
                        file: FileId(u32::MAX),
                        local: LocalDefId(db_ns.predicates.len() as u32),
                    };
                    db_ns.predicates.insert(
                        (table.name.clone(), arity),
                        PredicateInfo {
                            def_id: db_def,
                            result_type: None,
                            arity,
                        },
                    );
                }
                // Register @-prefixed entity types from unions and cases
                for union_t in schema.unions() {
                    let db_def = DefId {
                        file: FileId(u32::MAX),
                        local: LocalDefId(0),
                    };
                    // Strip @ prefix for type registration
                    let type_name = union_t.name.strip_prefix('@').unwrap_or(&union_t.name);
                    db_ns.types.entry(type_name.to_string()).or_insert(db_def);
                    for variant in &union_t.variants {
                        let var_name = variant.strip_prefix('@').unwrap_or(variant);
                        db_ns.types.entry(var_name.to_string()).or_insert(db_def);
                    }
                }
                for case_t in schema.cases() {
                    let db_def = DefId {
                        file: FileId(u32::MAX),
                        local: LocalDefId(0),
                    };
                    for variant in &case_t.variants {
                        let var_name = variant.entity_type.strip_prefix('@').unwrap_or(&variant.entity_type);
                        db_ns.types.entry(var_name.to_string()).or_insert(db_def);
                    }
                }
            }
        }
    }

    // Merge database predicates/types into builtins so they're available everywhere
    builtins.namespaces.merge_from(&db_ns);

    // Phase 4+5: Name resolution + type checking (in dependency order)
    let mut file_analyses: HashMap<FileId, FileAnalysis> = HashMap::new();
    let mut exported_ns: HashMap<FileId, ModuleNamespaces> = HashMap::new();
    for scc in module_graph.processing_order() {
        // For SCCs with multiple files, pre-populate exported_ns with declarations
        // so that mutually-dependent files can see each other's types.
        // Do multiple passes to propagate types through the SCC.
        if scc.len() > 1 {
            // First pass: each file gets its own declarations
            for &fid in scc {
                if let Some(c) = collectors.get(&fid) {
                    exported_ns.insert(fid, c.namespaces.clone());
                }
            }
            // Iterative propagation (3 passes is usually enough for SCC convergence)
            for _ in 0..3 {
                for &fid in scc {
                    let mut ns = collectors.get(&fid).map(|c| c.namespaces.clone()).unwrap_or_default();
                    // Collect module aliases for this file (ALL imports, including private)
                    let mut alias_targets: HashMap<String, FileId> = HashMap::new();
                    for edge in module_graph.file_imports(fid) {
                        if let Some(alias) = &edge.alias {
                            alias_targets.insert(alias.clone(), edge.target);
                        }
                        if !edge.is_private {
                            if let Some(exp) = exported_ns.get(&edge.target) {
                                ns.merge_from(exp);
                            }
                            if let Some(alias) = &edge.alias {
                                let target_def = DefId { file: edge.target, local: LocalDefId(0) };
                                ns.modules.entry(alias.clone()).or_insert(target_def);
                            }
                        }
                    }
                    // Also resolve module-member imports (import Alias::Member)
                    if let Some(ast) = parsed.get(&fid) {
                        for member in &ast.members {
                            if let ocql_ql_ast::module::ModuleMember::Import(import) = member {
                                let is_private = import
                                    .annotations
                                    .iter()
                                    .any(|a| a.kind == ocql_ql_ast::annotation::AnnotationKind::Private);
                                let parts = &import.path.parts;
                                if parts.len() >= 2 {
                                    let first = &parts[0];
                                    let member_name = &parts[1];
                                    let alias_target = alias_targets.get(first).or_else(|| {
                                        ns.modules.get(first).map(|d| &d.file)
                                    }).copied();
                                    if let Some(target_fid) = alias_target {
                                        if let Some(target_exp) = exported_ns.get(&target_fid) {
                                            if let Some(&mod_def) = target_exp.modules.get(member_name.as_str()) {
                                                ns.modules.entry(member_name.clone()).or_insert(mod_def);
                                                if let Some(mod_exp) = exported_ns.get(&mod_def.file) {
                                                    if !is_private {
                                                        ns.merge_from(mod_exp);
                                                    }
                                                }
                                            }
                                            if !is_private {
                                                if let Some(&type_def) = target_exp.types.get(member_name.as_str()) {
                                                    ns.types.entry(member_name.clone()).or_insert(type_def);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    exported_ns.insert(fid, ns);
                }
            }
        }

        for &file_id in scc {
            let Some(ast) = parsed.get(&file_id) else {
                continue;
            };
            let Some(collector) = collectors.get(&file_id) else {
                continue;
            };

            // Build merged imported namespace from this file's resolved imports
            let mut imported = ModuleNamespaces::default();
            // Track module aliases: alias name → FileId
            let mut module_alias_targets: HashMap<String, FileId> = HashMap::new();

            for edge in module_graph.file_imports(file_id) {
                if let Some(exp) = exported_ns.get(&edge.target) {
                    if let Some(alias) = &edge.alias {
                        // `import foo.bar as Baz` → register "Baz" as a module alias
                        let target_def = DefId {
                            file: edge.target,
                            local: LocalDefId(0),
                        };
                        imported.modules.entry(alias.clone()).or_insert(target_def);
                        module_alias_targets.insert(alias.clone(), edge.target);
                    }
                    if edge.alias.is_none() {
                        imported.merge_from(exp);
                    }
                }
            }


            // Phase 4b: Resolve module-member imports (e.g., `import Imports::EdgeKind`)
            // For each AST import that wasn't resolved as a file import, check if it's
            // a module-member access pattern.
            for member in &ast.members {
                if let ocql_ql_ast::module::ModuleMember::Import(import) = member {
                    let parts = &import.path.parts;
                    if parts.len() >= 2 {
                        let first = &parts[0];
                        // Check if first segment is a known module alias
                        if let Some(&alias_target) = module_alias_targets.get(first) {
                            // `import Alias::Member` → look up Member in alias target's exports
                            if let Some(target_exp) = exported_ns.get(&alias_target) {
                                // The remaining segments form the member path
                                let member_name = &parts[1];
                                // Try as a module
                                if let Some(&mod_def) = target_exp.modules.get(member_name.as_str()) {
                                    imported.modules.entry(member_name.clone()).or_insert(mod_def);
                                    // If this module resolves to a file, merge its exports
                                    if let Some(mod_exp) = exported_ns.get(&mod_def.file) {
                                        imported.merge_from(mod_exp);
                                    }
                                }
                                // Try as a type
                                if let Some(&type_def) = target_exp.types.get(member_name.as_str()) {
                                    imported.types.entry(member_name.clone()).or_insert(type_def);
                                }
                                // Try as a predicate (all arities)
                                for ((pred_name, arity), info) in &target_exp.predicates {
                                    if pred_name == member_name {
                                        imported.predicates.entry((pred_name.clone(), *arity)).or_insert_with(|| info.clone());
                                    }
                                }
                            }
                        }
                        // Also check if first segment is in imported modules (from transitive imports)
                        else if let Some(&mod_def) = imported.modules.get(first) {
                            if let Some(target_exp) = exported_ns.get(&mod_def.file) {
                                let member_name = &parts[1];
                                if let Some(&mod_def2) = target_exp.modules.get(member_name.as_str()) {
                                    imported.modules.entry(member_name.clone()).or_insert(mod_def2);
                                    if let Some(mod_exp) = exported_ns.get(&mod_def2.file) {
                                        imported.merge_from(mod_exp);
                                    }
                                }
                                if let Some(&type_def) = target_exp.types.get(member_name.as_str()) {
                                    imported.types.entry(member_name.clone()).or_insert(type_def);
                                }
                                for ((pred_name, arity), info) in &target_exp.predicates {
                                    if pred_name == member_name {
                                        imported.predicates.entry((pred_name.clone(), *arity)).or_insert_with(|| info.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }


            let mut resolver = resolve::NameResolver::new(
                file_id,
                &collector.namespaces,
                &imported,
                &builtins.namespaces,
                collector.next_local_id(),
            );
            resolver.resolve_source_file(ast);

            let name_resolution = std::mem::take(&mut resolver.name_resolutions);
            let expr_types = std::mem::take(&mut resolver.expr_types);
            let diagnostics = std::mem::take(&mut resolver.diagnostics);
            drop(resolver);

            // This file's exported namespace = its own declarations + public re-exports.
            // In CodeQL, a non-private import re-exports everything from the imported module.
            let mut exported = collector.namespaces.clone();
            for edge in module_graph.file_imports(file_id) {
                if !edge.is_private {
                    if let Some(exp) = exported_ns.get(&edge.target) {
                        exported.merge_from(exp);
                    }
                    // For aliased public imports, also export the alias as a module
                    if let Some(alias) = &edge.alias {
                        let target_def = DefId {
                            file: edge.target,
                            local: LocalDefId(0),
                        };
                        exported.modules.entry(alias.clone()).or_insert(target_def);
                    }
                }
            }
            // Also re-export types from non-private Phase 4b resolved module-member imports
            for member in &ast.members {
                if let ocql_ql_ast::module::ModuleMember::Import(import) = member {
                    let is_private = import
                        .annotations
                        .iter()
                        .any(|a| a.kind == ocql_ql_ast::annotation::AnnotationKind::Private);
                    if !is_private {
                        let parts = &import.path.parts;
                        if parts.len() >= 2 {
                            let first = &parts[0];
                            let member_name = &parts[1];
                            // Look up the first segment as a module alias
                            let target_fid = module_alias_targets.get(first).copied()
                                .or_else(|| imported.modules.get(first).map(|d| d.file));
                            if let Some(target_fid) = target_fid {
                                if let Some(target_exp) = exported_ns.get(&target_fid) {
                                    if let Some(&mod_def) = target_exp.modules.get(member_name.as_str()) {
                                        exported.modules.entry(member_name.clone()).or_insert(mod_def);
                                        if let Some(mod_exp) = exported_ns.get(&mod_def.file) {
                                            exported.merge_from(mod_exp);
                                        }
                                    }
                                    if let Some(&type_def) = target_exp.types.get(member_name.as_str()) {
                                        exported.types.entry(member_name.clone()).or_insert(type_def);
                                    }
                                    for ((pred_name, arity), info) in &target_exp.predicates {
                                        if pred_name == member_name {
                                            exported.predicates.entry((pred_name.clone(), *arity)).or_insert_with(|| info.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            exported_ns.insert(file_id, exported);

            // We need to take the collector's defs; but collectors is borrowed.
            // Store results for now without defs, we'll fill them in below.
            file_analyses.insert(
                file_id,
                FileAnalysis {
                    ast: ast.clone(),
                    defs: Vec::new(), // filled below
                    name_resolution,
                    expr_types,
                    diagnostics,
                },
            );
        }
    }


    // Fill in defs from collectors
    for (file_id, collector) in collectors {
        if let Some(analysis) = file_analyses.get_mut(&file_id) {
            analysis.defs = collector.into_defs();
        }
    }

    HirDatabase {
        sources,
        module_graph,
        files: file_analyses,
        diagnostics: global_diags,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_ql_ast::ty::PrimitiveType;

    fn analyze_ok(source: &str) -> HirDatabase {
        let db = analyze_single_file(source, "test.ql");
        let errors: Vec<_> = db.all_errors().collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:#?}");
        db
    }

    fn analyze_errors(source: &str) -> Vec<String> {
        let db = analyze_single_file(source, "test.ql");
        db.all_errors().map(|d| d.message.clone()).collect()
    }

    fn file_analysis(db: &HirDatabase) -> &FileAnalysis {
        db.files.values().next().unwrap()
    }

    #[test]
    fn simple_select() {
        let db = analyze_ok("from int x\nwhere x > 0\nselect x");
        let file = file_analysis(&db);
        assert!(!file.expr_types.is_empty());
        let x_types: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Int))
            .collect();
        assert!(!x_types.is_empty());
    }

    #[test]
    fn predicate_call_in_where() {
        analyze_ok(
            r#"
            predicate isSmall(int x) { x >= 0 and x < 10 }
            from int x
            where isSmall(x)
            select x
            "#,
        );
    }

    #[test]
    fn predicate_with_result() {
        analyze_ok(
            r#"
            int doubleIt(int x) { result = x + x }
            from int x
            where x = doubleIt(3)
            select x
            "#,
        );
    }

    #[test]
    fn undefined_variable_error() {
        let errors = analyze_errors("from int x\nwhere y > 0\nselect x");
        assert!(errors.iter().any(|e| e.contains("undefined variable `y`")));
    }

    #[test]
    fn undefined_predicate_error() {
        let errors = analyze_errors("from int x\nwhere notDefined(x)\nselect x");
        assert!(errors.iter().any(|e| e.contains("undefined predicate `notDefined`")));
    }

    #[test]
    fn type_mismatch_comparison() {
        let errors = analyze_errors(r#"from int x, string s where x = s select x"#);
        assert!(errors.iter().any(|e| e.contains("cannot compare")));
    }

    #[test]
    fn arithmetic_type_checking() {
        let db = analyze_ok("from int x\nwhere x = 1 + 2\nselect x");
        let file = file_analysis(&db);
        let int_exprs: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Int))
            .collect();
        assert!(int_exprs.len() >= 3);
    }

    #[test]
    fn string_concat() {
        let db = analyze_ok(r#"from string s where s = "hello" + " world" select s"#);
        let file = file_analysis(&db);
        let string_exprs: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::String))
            .collect();
        assert!(string_exprs.len() >= 3);
    }

    #[test]
    fn exists_quantifier() {
        analyze_ok(
            r#"
            predicate isSmall(int x) { x >= 0 and x < 10 }
            from int x
            where exists(int y | isSmall(y) and x = y)
            select x
            "#,
        );
    }

    #[test]
    fn quantifier_scoping() {
        let errors = analyze_errors(
            r#"
            from int x
            where exists(int y | y > 0) and y < 10
            select x
            "#,
        );
        assert!(errors.iter().any(|e| e.contains("undefined variable `y`")));
    }

    #[test]
    fn aggregation_count() {
        let db = analyze_ok(
            r#"
            from int n
            where n = count(int x | x = [1 .. 10] | x)
            select n
            "#,
        );
        let file = file_analysis(&db);
        let has_int = file
            .expr_types
            .iter()
            .any(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Int));
        assert!(has_int);
    }

    #[test]
    fn class_basic() {
        analyze_ok(
            r#"
            class SmallInt extends int {
                SmallInt() { this >= 0 and this < 100 }
            }
            from SmallInt x
            select x
            "#,
        );
    }

    #[test]
    fn result_outside_predicate() {
        let errors = analyze_errors("from int x\nwhere x = result\nselect x");
        assert!(errors.iter().any(|e| e.contains("`result` used outside")));
    }

    #[test]
    fn this_outside_class() {
        let errors = analyze_errors("predicate foo() { this = 1 }");
        assert!(errors.iter().any(|e| e.contains("`this` used outside")));
    }

    #[test]
    fn multiple_predicates_different_arity() {
        analyze_ok(
            r#"
            predicate p(int x) { x > 0 }
            predicate p(int x, int y) { x > y }
            from int a, int b
            where p(a) and p(a, b)
            select a, b
            "#,
        );
    }

    #[test]
    fn range_expression() {
        analyze_ok("from int x\nwhere x = [1 .. 10]\nselect x");
    }

    #[test]
    fn float_arithmetic() {
        let db = analyze_ok("from float x\nwhere x = 1.0 + 2.5\nselect x");
        let file = file_analysis(&db);
        let float_exprs: Vec<_> = file
            .expr_types
            .iter()
            .filter(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Float))
            .collect();
        assert!(float_exprs.len() >= 3);
    }

    #[test]
    fn mixed_int_float() {
        let db = analyze_ok("from float x\nwhere x = 1 + 2.5\nselect x");
        let file = file_analysis(&db);
        let float_add = file
            .expr_types
            .iter()
            .any(|(_, ty)| *ty == Type::Primitive(PrimitiveType::Float));
        assert!(float_add);
    }

    #[test]
    fn error_formatting() {
        let db = analyze_single_file("from int x\nwhere y > 0\nselect x", "test.ql");
        for diag in db.all_errors() {
            let formatted = db.format_diagnostic(diag);
            assert!(formatted.contains("error:"));
            assert!(formatted.contains("undefined variable `y`"));
            assert!(formatted.contains("test.ql:"));
        }
    }
}

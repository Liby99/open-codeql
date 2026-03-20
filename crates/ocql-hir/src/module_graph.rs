use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use ocql_common::Span;

use crate::FileId;

/// An edge in the module import graph.
#[derive(Clone, Debug)]
pub struct ImportEdge {
    /// The imported file.
    pub target: FileId,
    /// The target's path (for diagnostics).
    pub target_path: PathBuf,
    /// Import path segments (e.g., ["semmle", "code", "cpp", "Element"]).
    pub import_path: Vec<String>,
    /// Whether this is a `private import`.
    pub is_private: bool,
    /// Optional alias name.
    pub alias: Option<String>,
    /// Source span of the import statement.
    pub span: Span,
}

/// An unresolved import (target file not found).
#[derive(Clone, Debug)]
pub struct UnresolvedImport {
    pub from_file: FileId,
    pub import_path: Vec<String>,
    pub span: Span,
}

/// The module dependency graph.
pub struct ModuleGraph {
    /// Forward edges: file → imports.
    pub imports: HashMap<FileId, Vec<ImportEdge>>,
    /// Reverse edges: file → files that import it.
    pub imported_by: HashMap<FileId, Vec<FileId>>,
    /// Imports that couldn't be resolved.
    pub unresolved: Vec<UnresolvedImport>,
    /// Topological order for processing (files that depend on nothing first).
    /// Files in the same SCC are grouped together.
    pub topo_order: Vec<Vec<FileId>>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            imports: HashMap::new(),
            imported_by: HashMap::new(),
            unresolved: Vec::new(),
            topo_order: Vec::new(),
        }
    }

    /// Add a resolved import edge.
    pub fn add_edge(&mut self, from: FileId, edge: ImportEdge) {
        self.imported_by
            .entry(edge.target)
            .or_default()
            .push(from);
        self.imports.entry(from).or_default().push(edge);
    }

    /// Add an unresolved import.
    pub fn add_unresolved(&mut self, unresolved: UnresolvedImport) {
        self.unresolved.push(unresolved);
    }

    /// Compute topological order using Tarjan's SCC algorithm.
    /// Files within the same SCC (mutual imports) are grouped together.
    pub fn compute_topo_order(&mut self, all_files: &[FileId]) {
        let sccs = tarjan_scc(all_files, &self.imports);
        // Tarjan produces SCCs with leaf dependencies first (sources before sinks).
        // This is already the correct processing order: dependencies before dependents.
        self.topo_order = sccs;
    }

    /// Get the processing order (dependencies first).
    pub fn processing_order(&self) -> impl Iterator<Item = &[FileId]> {
        self.topo_order.iter().map(|v| v.as_slice())
    }

    /// Get imports for a file.
    pub fn file_imports(&self, file: FileId) -> &[ImportEdge] {
        self.imports.get(&file).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Total number of resolved import edges.
    pub fn total_edge_count(&self) -> usize {
        self.imports.values().map(|v| v.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// Tarjan's SCC algorithm
// ---------------------------------------------------------------------------

struct TarjanState {
    index_counter: u32,
    stack: Vec<FileId>,
    on_stack: HashSet<FileId>,
    index: HashMap<FileId, u32>,
    lowlink: HashMap<FileId, u32>,
    sccs: Vec<Vec<FileId>>,
}

fn tarjan_scc(
    all_nodes: &[FileId],
    edges: &HashMap<FileId, Vec<ImportEdge>>,
) -> Vec<Vec<FileId>> {
    let mut state = TarjanState {
        index_counter: 0,
        stack: Vec::new(),
        on_stack: HashSet::new(),
        index: HashMap::new(),
        lowlink: HashMap::new(),
        sccs: Vec::new(),
    };

    for &node in all_nodes {
        if !state.index.contains_key(&node) {
            strongconnect(node, edges, &mut state);
        }
    }

    state.sccs
}

fn strongconnect(
    v: FileId,
    edges: &HashMap<FileId, Vec<ImportEdge>>,
    state: &mut TarjanState,
) {
    state.index.insert(v, state.index_counter);
    state.lowlink.insert(v, state.index_counter);
    state.index_counter += 1;
    state.stack.push(v);
    state.on_stack.insert(v);

    if let Some(neighbors) = edges.get(&v) {
        for edge in neighbors {
            let w = edge.target;
            if !state.index.contains_key(&w) {
                strongconnect(w, edges, state);
                let w_lowlink = state.lowlink[&w];
                let v_lowlink = state.lowlink.get_mut(&v).unwrap();
                *v_lowlink = (*v_lowlink).min(w_lowlink);
            } else if state.on_stack.contains(&w) {
                let w_index = state.index[&w];
                let v_lowlink = state.lowlink.get_mut(&v).unwrap();
                *v_lowlink = (*v_lowlink).min(w_index);
            }
        }
    }

    if state.lowlink[&v] == state.index[&v] {
        let mut scc = Vec::new();
        loop {
            let w = state.stack.pop().unwrap();
            state.on_stack.remove(&w);
            scc.push(w);
            if w == v {
                break;
            }
        }
        state.sccs.push(scc);
    }
}

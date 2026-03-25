//! Emit MIR predicates as engine-level Datalog rules.
//!
//! Converts `MirProgram` → `ocql_engine::rule::Program`.

use ocql_engine::rule::{
    AggFunction, Atom, ArithExpr, ArithOp, BodyElement, CompOp, Guard, Program, Rule, Term,
};
use ocql_database::Value;
use ordered_float::OrderedFloat;

use crate::nodes::*;

/// Convert a MIR program to an engine program.
pub fn emit_program(mir: &MirProgram) -> Program {
    let mut rules = Vec::new();
    let mut anon_counter = 0u32;
    for pred in &mir.predicates {
        emit_predicate(pred, &mut rules, &mut anon_counter);
    }
    Program::new(rules)
}

fn emit_predicate(pred: &MirPredicate, rules: &mut Vec<Rule>, anon: &mut u32) {
    match &pred.body {
        MirBody::None => {
            // Abstract — no rules
        }
        MirBody::Conjunction(atoms) => {
            let head = emit_head(&pred.name, &pred.params, anon);
            let body = emit_atoms(atoms, anon);
            rules.push(Rule::new(head, body));
        }
        MirBody::Disjunction(clauses) => {
            for clause in clauses {
                let head = emit_head(&pred.name, &pred.params, anon);
                let body = emit_atoms(clause, anon);
                rules.push(Rule::new(head, body));
            }
        }
    }
}

fn emit_head(name: &str, params: &[MirParam], _anon: &mut u32) -> Atom {
    let terms: Vec<Term> = params
        .iter()
        .map(|p| Term::Var(p.name.clone()))
        .collect();
    Atom::new(name, terms)
}

fn emit_atoms(atoms: &[MirAtom], anon: &mut u32) -> Vec<BodyElement> {
    atoms.iter().map(|atom| emit_atom(atom, anon)).collect()
}

fn emit_atom(atom: &MirAtom, anon: &mut u32) -> BodyElement {
    match atom {
        MirAtom::Scan(scan) => {
            BodyElement::Positive(emit_scan(scan, anon))
        }
        MirAtom::NegScan(scan) => {
            BodyElement::Negated(emit_scan(scan, anon))
        }
        MirAtom::Guard(guard) => {
            BodyElement::Guard(Guard {
                left: emit_term(&guard.left, anon),
                op: emit_comp_op(guard.op),
                right: emit_term(&guard.right, anon),
            })
        }
        MirAtom::Assign(assign) => {
            BodyElement::Assign {
                result_var: assign.result_var.clone(),
                expr: ArithExpr {
                    left: emit_term(&assign.expr.left, anon),
                    op: emit_arith_op(assign.expr.op),
                    right: emit_term(&assign.expr.right, anon),
                },
            }
        }
        MirAtom::Aggregate(agg) => {
            // Build sub-rule: head is the sub_predicate, body is empty
            // (the sub_predicate should already exist as a separate MIR predicate)
            let group_terms: Vec<Term> = agg.group_by.iter()
                .map(|v| Term::Var(v.clone()))
                .collect();
            let mut sub_terms = group_terms.clone();
            sub_terms.push(Term::Var(agg.agg_var.clone()));
            let sub_head = Atom::new(&agg.sub_predicate, sub_terms.clone());
            let sub_body = vec![BodyElement::Positive(Atom::new(&agg.sub_predicate, sub_terms))];
            let sub_rule = Rule::new(sub_head, sub_body);

            BodyElement::Aggregate {
                result_var: agg.result_var.clone(),
                function: emit_agg_function(agg.function),
                sub_rule: Box::new(sub_rule),
                group_by: agg.group_by.clone(),
                agg_var: agg.agg_var.clone(),
            }
        }
        MirAtom::TypeCheck(tc) => {
            // TypeCheck lowers to a scan on the characteristic predicate
            BodyElement::Positive(Atom::new(
                &tc.type_predicate,
                vec![Term::Var(tc.var.clone())],
            ))
        }
    }
}

fn emit_scan(scan: &MirScan, anon: &mut u32) -> Atom {
    let terms: Vec<Term> = scan.args.iter().map(|t| emit_term(t, anon)).collect();
    Atom::new(&scan.predicate, terms)
}

fn emit_term(term: &MirTerm, anon: &mut u32) -> Term {
    match term {
        MirTerm::Var(name) => Term::Var(name.clone()),
        MirTerm::Const(c) => Term::Const(emit_const(c)),
        MirTerm::Wildcard => {
            let name = format!("_anon{}", anon);
            *anon += 1;
            Term::Var(name)
        }
    }
}

fn emit_const(c: &MirConst) -> Value {
    match c {
        MirConst::Int(v) => Value::Int(*v),
        MirConst::Float(v) => Value::Float(OrderedFloat(*v)),
        MirConst::String(_s) => {
            // Note: strings need to be resolved via StringInterner before evaluation.
            // We use a sentinel value here; the caller should call resolve_strings.
            Value::Null // Placeholder — see note below
        }
        MirConst::Bool(b) => Value::Bool(*b),
    }
}

fn emit_comp_op(op: MirCompOp) -> CompOp {
    match op {
        MirCompOp::Eq => CompOp::Eq,
        MirCompOp::Ne => CompOp::Ne,
        MirCompOp::Lt => CompOp::Lt,
        MirCompOp::Le => CompOp::Le,
        MirCompOp::Gt => CompOp::Gt,
        MirCompOp::Ge => CompOp::Ge,
    }
}

fn emit_arith_op(op: MirArithOp) -> ArithOp {
    match op {
        MirArithOp::Add => ArithOp::Add,
        MirArithOp::Sub => ArithOp::Sub,
        MirArithOp::Mul => ArithOp::Mul,
        MirArithOp::Div => ArithOp::Div,
        MirArithOp::Mod => ArithOp::Mod,
    }
}

fn emit_agg_function(f: MirAggFunction) -> AggFunction {
    match f {
        MirAggFunction::Count | MirAggFunction::StrictCount => AggFunction::Count,
        MirAggFunction::Sum | MirAggFunction::StrictSum => AggFunction::Sum,
        MirAggFunction::Min => AggFunction::Min,
        MirAggFunction::Max => AggFunction::Max,
        // These don't have direct engine support yet — fall back to Count
        MirAggFunction::Avg | MirAggFunction::Concat | MirAggFunction::StrictConcat
        | MirAggFunction::Rank | MirAggFunction::Any => AggFunction::Count,
    }
}

/// Emit a MIR program to engine rules, using StrLit for string constants
/// so they can be properly resolved against a database's StringInterner.
pub fn emit_program_with_strings(mir: &MirProgram) -> Program {
    let mut rules = Vec::new();
    let mut anon_counter = 0u32;
    for pred in &mir.predicates {
        emit_predicate_with_strings(pred, &mut rules, &mut anon_counter);
    }
    Program::new(rules)
}

fn emit_predicate_with_strings(pred: &MirPredicate, rules: &mut Vec<Rule>, anon: &mut u32) {
    match &pred.body {
        MirBody::None => {}
        MirBody::Conjunction(atoms) => {
            let head = emit_head(&pred.name, &pred.params, anon);
            let body = emit_atoms_with_strings(atoms, anon);
            rules.push(Rule::new(head, body));
        }
        MirBody::Disjunction(clauses) => {
            for clause in clauses {
                let head = emit_head(&pred.name, &pred.params, anon);
                let body = emit_atoms_with_strings(clause, anon);
                rules.push(Rule::new(head, body));
            }
        }
    }
}

fn emit_atoms_with_strings(atoms: &[MirAtom], anon: &mut u32) -> Vec<BodyElement> {
    atoms.iter().map(|atom| emit_atom_with_strings(atom, anon)).collect()
}

fn emit_atom_with_strings(atom: &MirAtom, anon: &mut u32) -> BodyElement {
    match atom {
        MirAtom::Scan(scan) => {
            let terms: Vec<Term> = scan.args.iter().map(|t| emit_term_with_strings(t, anon)).collect();
            BodyElement::Positive(Atom::new(&scan.predicate, terms))
        }
        MirAtom::NegScan(scan) => {
            let terms: Vec<Term> = scan.args.iter().map(|t| emit_term_with_strings(t, anon)).collect();
            BodyElement::Negated(Atom::new(&scan.predicate, terms))
        }
        MirAtom::Guard(guard) => {
            BodyElement::Guard(Guard {
                left: emit_term_with_strings(&guard.left, anon),
                op: emit_comp_op(guard.op),
                right: emit_term_with_strings(&guard.right, anon),
            })
        }
        MirAtom::Assign(assign) => {
            BodyElement::Assign {
                result_var: assign.result_var.clone(),
                expr: ArithExpr {
                    left: emit_term_with_strings(&assign.expr.left, anon),
                    op: emit_arith_op(assign.expr.op),
                    right: emit_term_with_strings(&assign.expr.right, anon),
                },
            }
        }
        _ => emit_atom(atom, anon), // aggregate, typecheck — same handling
    }
}

fn emit_term_with_strings(term: &MirTerm, anon: &mut u32) -> Term {
    match term {
        MirTerm::Var(name) => Term::Var(name.clone()),
        MirTerm::Const(MirConst::String(s)) => Term::StrLit(s.clone()),
        MirTerm::Const(c) => Term::Const(emit_const(c)),
        MirTerm::Wildcard => {
            let name = format!("_anon{}", anon);
            *anon += 1;
            Term::Var(name)
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_simple_predicate() {
        let mir = MirProgram {
            predicates: vec![MirPredicate::new(
                "isSmall",
                vec![MirParam::new("x", MirType::Int)],
                vec![
                    MirAtom::Scan(MirScan::new("vals", vec![MirTerm::var("x")])),
                    MirAtom::Guard(MirGuard {
                        left: MirTerm::var("x"),
                        op: MirCompOp::Lt,
                        right: MirTerm::int(10),
                    }),
                ],
            )],
        };

        let program = emit_program(&mir);
        assert_eq!(program.rules.len(), 1);
        assert_eq!(program.rules[0].head.predicate, "isSmall");
        assert_eq!(program.rules[0].body.len(), 2);
    }

    #[test]
    fn emit_disjunction_creates_multiple_rules() {
        let mir = MirProgram {
            predicates: vec![MirPredicate {
                name: "test".to_string(),
                params: vec![MirParam::new("x", MirType::Int)],
                body: MirBody::Disjunction(vec![
                    vec![MirAtom::Scan(MirScan::new("a", vec![MirTerm::var("x")]))],
                    vec![MirAtom::Scan(MirScan::new("b", vec![MirTerm::var("x")]))],
                ]),
                annotations: MirAnnotations::default(),
                is_abstract: false,
            }],
        };

        let program = emit_program(&mir);
        assert_eq!(program.rules.len(), 2);
        assert_eq!(program.rules[0].head.predicate, "test");
        assert_eq!(program.rules[1].head.predicate, "test");
    }

    #[test]
    fn emit_abstract_no_rules() {
        let mir = MirProgram {
            predicates: vec![MirPredicate::abstract_pred(
                "myAbstract",
                vec![MirParam::new("x", MirType::Int)],
            )],
        };

        let program = emit_program(&mir);
        assert_eq!(program.rules.len(), 0);
    }

    #[test]
    fn emit_typecheck_becomes_scan() {
        let mir = MirProgram {
            predicates: vec![MirPredicate::new(
                "test",
                vec![MirParam::new("x", MirType::Any)],
                vec![MirAtom::TypeCheck(MirTypeCheck {
                    var: "x".to_string(),
                    type_predicate: "Function#char".to_string(),
                })],
            )],
        };

        let program = emit_program(&mir);
        assert_eq!(program.rules.len(), 1);
        match &program.rules[0].body[0] {
            BodyElement::Positive(atom) => {
                assert_eq!(atom.predicate, "Function#char");
            }
            _ => panic!("expected Positive atom"),
        }
    }

    #[test]
    fn emit_wildcard_becomes_anon_var() {
        let mir = MirProgram {
            predicates: vec![MirPredicate::new(
                "test",
                vec![MirParam::new("x", MirType::Any)],
                vec![MirAtom::Scan(MirScan::new("rel", vec![
                    MirTerm::var("x"),
                    MirTerm::Wildcard,
                    MirTerm::Wildcard,
                ]))],
            )],
        };

        let program = emit_program(&mir);
        if let BodyElement::Positive(atom) = &program.rules[0].body[0] {
            if let Term::Var(name) = &atom.terms[1] {
                assert!(name.starts_with("_anon"));
            }
            // Two wildcards should have different names
            if let (Term::Var(n1), Term::Var(n2)) = (&atom.terms[1], &atom.terms[2]) {
                assert_ne!(n1, n2);
            }
        }
    }

    #[test]
    fn emit_string_with_strlit() {
        let mir = MirProgram {
            predicates: vec![MirPredicate::new(
                "test",
                vec![MirParam::new("x", MirType::Any)],
                vec![MirAtom::Scan(MirScan::new("rel", vec![
                    MirTerm::var("x"),
                    MirTerm::string("hello"),
                ]))],
            )],
        };

        let program = emit_program_with_strings(&mir);
        if let BodyElement::Positive(atom) = &program.rules[0].body[0] {
            assert!(matches!(&atom.terms[1], Term::StrLit(s) if s == "hello"));
        }
    }
}

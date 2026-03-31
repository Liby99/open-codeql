//! Pretty printer for LIR programs.
//!
//! Outputs a human-readable representation of the relational algebra plan.

use crate::nodes::*;

/// Pretty-print a LIR program.
pub fn pretty_print(program: &LirProgram) -> String {
    let mut out = String::new();
    for (i, stratum) in program.strata.iter().enumerate() {
        let kind = if stratum.recursive { "recursive" } else { "non-recursive" };
        out.push_str(&format!("--- stratum {} ({}) ---\n", i, kind));
        for rule in &stratum.rules {
            print_rule(rule, &mut out, 0);
            out.push('\n');
        }
    }
    out
}

fn print_rule(rule: &LirRule, out: &mut String, indent: usize) {
    let pad = "  ".repeat(indent);
    out.push_str(&format!("{}{}({}) ←\n", pad, rule.target, rule.target_columns.join(", ")));
    print_plan(&rule.body, out, indent + 1);
}

fn print_plan(plan: &LirPlan, out: &mut String, indent: usize) {
    let pad = "  ".repeat(indent);
    match plan {
        LirPlan::Scan { relation, bindings } => {
            let bs: Vec<String> = bindings.iter().map(|b| match b {
                LirBinding::Var(name) => name.clone(),
                LirBinding::Const(val) => format!("{}", val),
            }).collect();
            out.push_str(&format!("{}Scan {}({})\n", pad, relation, bs.join(", ")));
        }

        LirPlan::WcoJoin { atoms, variable_order, project } => {
            out.push_str(&format!("{}WcoJoin order=[{}] project=[{}]\n",
                pad, variable_order.join(", "), project.join(", ")));
            for atom in atoms {
                let bs: Vec<String> = atom.bindings.iter().map(|b| match b {
                    LirBinding::Var(name) => name.clone(),
                    LirBinding::Const(val) => format!("{}", val),
                }).collect();
                out.push_str(&format!("{}  {}({})\n", pad, atom.relation, bs.join(", ")));
            }
        }

        LirPlan::Filter { input, condition } => {
            out.push_str(&format!("{}Filter {}\n", pad, format_filter(condition)));
            print_plan(input, out, indent + 1);
        }

        LirPlan::Project { input, columns } => {
            out.push_str(&format!("{}Project [{}]\n", pad, columns.join(", ")));
            print_plan(input, out, indent + 1);
        }

        LirPlan::Union { inputs } => {
            out.push_str(&format!("{}Union ({} branches)\n", pad, inputs.len()));
            for input in inputs {
                print_plan(input, out, indent + 1);
            }
        }

        LirPlan::AntiJoin { positive, negative, key_columns } => {
            out.push_str(&format!("{}AntiJoin on [{}]\n", pad, key_columns.join(", ")));
            out.push_str(&format!("{}  positive:\n", pad));
            print_plan(positive, out, indent + 2);
            out.push_str(&format!("{}  negative:\n", pad));
            print_plan(negative, out, indent + 2);
        }

        LirPlan::Aggregate { input, group_by, function, agg_column, result_column } => {
            out.push_str(&format!("{}Aggregate {} = {}({}) group_by=[{}]\n",
                pad, result_column, function, agg_column, group_by.join(", ")));
            print_plan(input, out, indent + 1);
        }

        LirPlan::Extend { input, column, expr } => {
            out.push_str(&format!("{}Extend {} = {} {} {}\n",
                pad, column, format_operand(&expr.left), expr.op, format_operand(&expr.right)));
            print_plan(input, out, indent + 1);
        }

        LirPlan::Constant { columns, rows } => {
            out.push_str(&format!("{}Constant [{}] ({} rows)\n",
                pad, columns.join(", "), rows.len()));
        }
    }
}

fn format_filter(filter: &LirFilter) -> String {
    match filter {
        LirFilter::Comparison { left, op, right } => {
            format!("{} {} {}", format_operand(left), op, format_operand(right))
        }
        LirFilter::And(conditions) => {
            conditions.iter().map(format_filter).collect::<Vec<_>>().join(" AND ")
        }
    }
}

fn format_operand(op: &LirOperand) -> String {
    match op {
        LirOperand::Column(name) => name.clone(),
        LirOperand::Literal(val) => format!("{}", val),
    }
}

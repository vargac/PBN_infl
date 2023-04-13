use std::collections::HashSet;

use biodivine_lib_bdd::{Bdd, BddVariable, BddPartialValuation, BddValuation};
use biodivine_lib_param_bn::symbolic_async_graph::
    {SymbolicContext, GraphVertices};

use crate::symbolic_sync_graph::SymbSyncGraph;


pub fn attr_from_str(attr_str: &[&str], sync_graph: &SymbSyncGraph)
-> GraphVertices {
    let attr_vertex_ids = attr_str.iter()
        .map(|name| sync_graph.as_network().as_graph()
            .find_variable(name).unwrap())
        .collect::<HashSet<_>>();
    sync_graph.as_network().variables()
        .map(|var_id| (var_id, attr_vertex_ids.contains(&var_id)))
//TODO .fold(sync_graph().symbolic_context().mk_constant(true))
        .fold(sync_graph.unit_colored_vertices().vertices(),
            |acc, (var_id, val)| acc.fix_network_variable(var_id, val))
}

pub fn bdd_pick_unsupported(bdd: Bdd, variables: &[BddVariable]) -> Bdd {
    let support_set = bdd.support_set();
    variables.iter()
        .filter(|bdd_var| !support_set.contains(bdd_var))
        .fold(bdd, |acc, bdd_var| acc.var_pick(*bdd_var))
}


pub fn bdd_to_str(bdd: &Bdd, context: &SymbolicContext) -> String {
    format!("{}", bdd.to_boolean_expression(context.bdd_variable_set()))
}

pub fn bdd_var_to_str(bdd_var: BddVariable, context: &SymbolicContext)
-> String {
    context.bdd_variable_set().name_of(bdd_var)
}

pub fn partial_valuation_to_str(
    valuation: &BddPartialValuation,
    context: &SymbolicContext)
-> String {
    format!("[ {}]", valuation.to_values().iter()
        .map(|&(bdd_var, val)|
            format!("{}={} ", bdd_var_to_str(bdd_var, &context), val))
        .collect::<String>())
}

pub fn valuation_to_str(
    valuation: &BddValuation,
    support_set: impl IntoIterator<Item = BddVariable>,
    context: &SymbolicContext)
-> String {
    format!("[ {}]", support_set.into_iter()
        .map(|bdd_var| format!("{}{} ",
            if valuation[bdd_var] { "" } else { "!" },
            bdd_var_to_str(bdd_var, context)))
        .collect::<String>())
}

pub fn vertices_to_str(vertices: &GraphVertices, context: &SymbolicContext)
-> String {
    let all_false: Bdd = BddValuation::all_false(
        context.bdd_variable_set().num_vars()).into();
    format!("{{ {}}}", all_false
        .project(context.state_variables())
        .and(vertices.as_bdd())
        .sat_valuations()
        .map(|bdd_valuation|
            format!("{}; ", context
                .state_variables().iter()
                .filter(|&bdd_var| bdd_valuation[*bdd_var])
                .map(|bdd_var|
                    format!("{} ", bdd_var_to_str(*bdd_var, &context)))
                .collect::<String>()))
        .collect::<String>())
}

use biodivine_lib_bdd::{Bdd, BddVariable, BddPartialValuation, BddValuation};
use biodivine_lib_param_bn::{BooleanNetwork, FnUpdate,
    symbolic_async_graph::{SymbolicContext, GraphVertices}};

use crate::symbolic_sync_graph::SymbSyncGraph;


/// Adds self-regulation to input variables of `model`
/// without an update function.
///
/// Thus we can fix them as vertices and not as parametrizations.
pub fn add_self_regulations(mut model: BooleanNetwork) -> BooleanNetwork {
    for variable in model.variables() {
        if model.regulators(variable).is_empty()
        && model.get_update_function(variable).is_none() {
            let name = model.get_variable_name(variable);
            let regulation = format!("{} -> {}", name, name);
            model.as_graph_mut().add_string_regulation(&regulation).unwrap();
            model.add_update_function(
                variable, FnUpdate::Var(variable)).unwrap();
        }
    }
    model
}

/** A number of functions used for printing follows. They convert a type
 * from the biodivine-lib-bdd or biodivine-lib-param-bn to `String`. */

pub fn bdd_to_str(bdd: &Bdd, context: &SymbolicContext) -> String {
    format!("{}", bdd.to_boolean_expression(context.bdd_variable_set()))
}

pub fn bdd_var_to_str(bdd_var: BddVariable, context: &SymbolicContext)
-> String {
    format!("{}({bdd_var})", context.bdd_variable_set().name_of(bdd_var))
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

/// Prints the parametrized update functions for `sync_graph`.
pub fn print_update_functions(sync_graph: &SymbSyncGraph) {
    for pupdate_function in sync_graph.get_pupdate_functions() {
        let parametrizations = pupdate_function.get_parametrizations();
        println!("{}", 
            bdd_to_str(&parametrizations, sync_graph.symbolic_context()));

        let pars = sync_graph.get_all_false()
            .project(pupdate_function.get_parameters())
            .and(parametrizations);

        for parametrization in pars.sat_valuations() {
            println!("Parametrization: {}", valuation_to_str(
                    &parametrization,
                    pupdate_function.get_parameters().iter().copied(),
                    sync_graph.symbolic_context()));
            let f = pupdate_function.restricted(&parametrization);
            println!("Update function: {}",
                bdd_to_str(&f, sync_graph.symbolic_context()));

            for valuation in f.sat_clauses() {
                println!("\t{}", partial_valuation_to_str(
                        &valuation, sync_graph.symbolic_context()));
            }
        }
        println!();
    }
}

/// Returns a `Vec` of variations with replacement
///
/// * `values` - Values to choose from. One value may be chosen multiple times.
/// * `num` - The number of values to build one variation.
pub fn variations_with_replacement<T: Default + Clone>(
    values: &[T],
    num: usize
) -> Vec<Vec<T>> {
    let mut result = Vec::new();
    let mut first = vec![T::default(); num];
    variations_with_replacement_r(values, &mut first, 0, &mut result);
    result
}

fn variations_with_replacement_r<T: Default + Clone>(
    values: &[T],
    current: &mut Vec<T>,
    index: usize,
    result: &mut Vec<Vec<T>>
) {
    if index == current.len() {
        result.push(current.to_vec());
        return;
    }

    for value in values {
        current[index] = value.clone();
        variations_with_replacement_r(values, current, index + 1, result);
    }
}

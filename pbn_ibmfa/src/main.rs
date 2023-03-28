#![allow(unused_imports)]
#![allow(unused_mut)]

use std::{env, process, fs};
use std::collections::HashMap;

use biodivine_lib_param_bn::{BooleanNetwork, VariableId, FnUpdate};
use biodivine_lib_param_bn::symbolic_async_graph::
    {SymbolicContext, GraphColoredVertices};
use biodivine_lib_bdd::{Bdd, BddVariable, BddPartialValuation};

mod _impl_regulation_constraint;
mod _impl_ibmfa_computations;

use crate::_impl_regulation_constraint::apply_regulation_constraints;
use crate::_impl_ibmfa_computations::*;


#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
enum FixingItem {
    Variable(usize),  // index of the Variable in BooleanNetwork
    Parameter(BddVariable), // BddVariable in SymbolicContext
}

impl FixingItem {
    fn to_str(&self, context: &SymbolicContext) -> String {
        match self {
            FixingItem::Variable(var_i) => bdd_var_to_str(
                &context.get_state_variable(VariableId::from_index(*var_i)),
                &context),
            FixingItem::Parameter(bdd_var) => bdd_var_to_str(bdd_var, &context),
        }
    }
}

#[derive(Clone, Debug)]
struct ParedUpdateFunction {
    function: Bdd,
    parametrizations: Bdd,
}

impl ParedUpdateFunction {
    fn new(update_function: &Bdd, unit_bdd: &Bdd) -> ParedUpdateFunction {
        let support_set = update_function.support_set();
        let mut parametrizations = unit_bdd.clone();
        for bdd_var in unit_bdd.support_set() {
            if !support_set.contains(&bdd_var) {
                parametrizations = parametrizations.var_project(bdd_var);
            }
        }
        ParedUpdateFunction {
            function: update_function.and(&parametrizations),
            parametrizations,
        }
    }

    fn restricted_parametrizations(&self, restriction: &Bdd) -> Bdd {
        let support_set = self.function.support_set();
        let mut restriction = restriction.clone();
        for bdd_var in restriction.support_set() {
            if !support_set.contains(&bdd_var) {
                restriction = restriction.var_project(bdd_var);
            }
        }
        self.parametrizations.and(&restriction)
    }
}

type VarIndex = HashMap<BddVariable, usize>;

struct BNetwork {
    context: SymbolicContext,
    unit_bdd: Bdd,
    pupdate_functions: Vec<ParedUpdateFunction>,
    total_update_function: Bdd,
    extra_state_var_equivalence: Bdd,
    var_index: VarIndex,
}

impl BNetwork {
    fn new(bn: &BooleanNetwork) -> BNetwork {
        let extra_vars = bn.variables()
            .map(|var_id| (var_id, 1))
            .collect::<HashMap<_, _>>();
        let context = SymbolicContext::with_extra_state_variables(
            &bn, &extra_vars).unwrap();

        let mut var_index = HashMap::new();
        for var_id in bn.variables() {
            var_index
                .insert(context.get_state_variable(var_id), var_id.to_index());
        }

        let update_functions: Vec<Bdd> = bn
            .variables()
            .map(|variable| {
                let regulators = bn.regulators(variable);
                bn.get_update_function(variable)
                    .as_ref()
                    .map(|fun| context.mk_fn_update_true(fun))
                    .unwrap_or_else(|| context.mk_implicit_function_is_true(
                            variable, &regulators)
                    )
            })
            .collect();

        // used to store the next network state to the extra variables
        let total_update_function = update_functions.iter()
            .zip(bn.variables())
            .map(|(bdd, var_id)| context
                .mk_extra_state_variable_is_true(var_id, 0)
                .iff(&bdd))
            .fold(context.mk_constant(true), |acc, bdd| acc.and(&bdd));

        // used to copy a state from the extra varaibles to the state variables
        let extra_state_var_equivalence = bn.variables()
            .map(|var_id| context
                .mk_extra_state_variable_is_true(var_id, 0)
                .iff(&context.mk_state_variable_is_true(var_id)))
            .fold(context.mk_constant(true), |acc, bdd| acc.and(&bdd));

        let unit_bdd = apply_regulation_constraints(context.mk_constant(true),
                                                    &bn, &context).unwrap();

        let pupdate_functions = update_functions.iter()
            .map(|fun| ParedUpdateFunction::new(fun, &unit_bdd))
            .collect::<Vec<_>>();

        BNetwork {
            context,
            unit_bdd,
            pupdate_functions,
            total_update_function,
            extra_state_var_equivalence,
            var_index
        }
    }

    fn post_synch(&self, initial: &GraphColoredVertices)
-> GraphColoredVertices {
        let output = initial.as_bdd() // (prev, ?)
            .and(&self.total_update_function) // (prev, next)
            .project(self.context.state_variables()) // (?, next)
            .and(&self.extra_state_var_equivalence) // (next, next)
            .project(self.context.all_extra_state_variables()); // (next, ?)
        GraphColoredVertices::new(output, &self.context)
    }

    fn pre_synch(&self, initial: &GraphColoredVertices)
-> GraphColoredVertices {
        let output = initial.as_bdd() // (next, ?)
            .and(&self.extra_state_var_equivalence) // (next, next)
            .project(self.context.state_variables()) // (?, next)
            .and(&self.total_update_function) // (prev, next)
            .project(self.context.all_extra_state_variables()); // (prev, ?)
        GraphColoredVertices::new(output, &self.context)
    }

    fn unit_colored_vertices(&self) -> GraphColoredVertices {
        GraphColoredVertices::new(self.unit_bdd.clone(), &self.context)
    }

    fn fix_network_variable(&self, variable: VariableId, value: bool)
-> GraphColoredVertices {
        let bdd_var = self.context.get_state_variable(variable);
        GraphColoredVertices::new(
            self.unit_bdd.var_select(bdd_var, value), &self.context)
    }

    fn bdd_to_str(&self, bdd: &Bdd) -> String {
        format!("{}",
            bdd.to_boolean_expression(self.context.bdd_variable_set()))
    }
}

fn bdd_var_to_str(bdd_var: &BddVariable, context: &SymbolicContext) -> String {
    context.bdd_variable_set().name_of(*bdd_var)
}

fn valuation_to_str(valuation: &BddPartialValuation, context: &SymbolicContext)
-> String {
    format!("{:?}", valuation
        .to_values()
        .iter()
        .map(|&(bdd_var, val)|
            format!("{}={}", bdd_var_to_str(&bdd_var, &context), val))
        .collect::<Vec<_>>()
    )
}


fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Use with one parameter -- path to the .aeon model");
        process::exit(1);
    }
    let model_string = fs::read_to_string(&args[1]).unwrap_or_else(|err| {
        eprintln!("Cannot read the file, err: {}", err);
        process::exit(1);
    });
    let mut model = BooleanNetwork::try_from(model_string.as_str()).unwrap();
    println!("vars: {}, pars: {}", model.num_vars(), model.num_parameters());
    println!("vars: {:?}", model.variables()
        .map(|var_id| model.get_variable_name(var_id))
        .collect::<Vec<_>>()
    );
    println!();

    let bnetwork = BNetwork::new(&model);

    println!("{:?}", bnetwork.var_index);
    println!();

    for pupdate_function in &bnetwork.pupdate_functions {
        for parametrization in pupdate_function.parametrizations.sat_clauses() {
            println!("{}",
                valuation_to_str(&parametrization, &bnetwork.context));
            let f = pupdate_function.function
                .restrict(&parametrization.to_values());
            println!("\t{}",
                f.to_boolean_expression(bnetwork.context.bdd_variable_set()));

            for valuation in f.sat_clauses() {
                println!("\t{}",
                    valuation_to_str(&valuation, &bnetwork.context));
            }
        }
        println!();
    }


    let iterations = 10;
    let mut fixings = HashMap::new();

    println!("Entropy: {}",
        ibmfa_entropy(&bnetwork, &fixings, iterations, true, true));

    let mut available_fixings = Vec::new();
    for var_i in 0..model.num_vars() {
        available_fixings.push((FixingItem::Variable(var_i), 0.0));
        available_fixings.push((FixingItem::Variable(var_i), 1.0));
    }
    for bdd_var in bnetwork.context.parameter_variables() {
        for val in [false, true] {
            // TODO we don't want is_true() neither, but  it has to be checked
            // on the dependent variable level.
            // TODO as well, update it after each fixing of any parameter var,
            // as it could prohibit some other parameter var fixing
            if !bnetwork.unit_bdd.var_select(*bdd_var, val).is_false() {
                available_fixings.push((FixingItem::Parameter(bdd_var.clone()),
                                        val as i32 as f32));
            }
        }
    }

    let mut restrictions = bnetwork.context.mk_constant(true);
    let mut fixings = HashMap::new();

    while available_fixings.len() > 0 {
        println!("======= {} ========", available_fixings.len());
        let (min_entropy_index, min_entropy) = available_fixings
            .iter()
            .map(|&(fixing_item, value)| {
                println!("Try fix {fixing_item:?} ({}) to {value}",
                    fixing_item.to_str(&bnetwork.context));
                fixings.insert(fixing_item, value);
                let ent = ibmfa_entropy(
                    &bnetwork, &fixings, iterations, false, false);
                println!("{ent}");
                fixings.remove(&fixing_item);
                ent
            })
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        let (fixing_item, value) = available_fixings.remove(min_entropy_index);
        available_fixings.remove(min_entropy_index / 2 * 2);

        println!("Fixing {fixing_item:?} ({}) = {value}, entropy:{min_entropy}",
            fixing_item.to_str(&bnetwork.context));
        fixings.insert(fixing_item, value);

        if min_entropy == 0.0 {
            break;
        }
    }

    println!("{:?}", fixings);
    println!();

    let init_state = vec![false, false, false, false, true, false];
    let start = init_state.iter()
        .enumerate()
        .fold(bnetwork.unit_colored_vertices(),
            |acc, (i, &val)|
                acc.fix_network_variable(VariableId::from_index(i), val));

    println!("{}", bnetwork.bdd_to_str(bnetwork.pre_synch(&start).as_bdd()));
    println!("{}", bnetwork.bdd_to_str(start.as_bdd()));
    println!("{}", bnetwork.bdd_to_str(bnetwork.post_synch(&start).as_bdd()));
}

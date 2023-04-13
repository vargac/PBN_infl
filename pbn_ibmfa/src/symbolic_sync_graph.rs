use std::collections::HashMap;

use biodivine_lib_param_bn::{BooleanNetwork};
use biodivine_lib_param_bn::symbolic_async_graph::SymbolicContext;
use biodivine_lib_bdd::{Bdd, BddVariable, BddValuation};

use regulation_constraints::apply_regulation_constraints;


mod attractors;
mod regulation_constraints;
mod graph_operations;


#[derive(Clone, Debug)]
pub struct ParedUpdateFunction {
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

    pub fn get_function(&self) -> &Bdd {
        &self.function
    }

    pub fn get_parametrizations(&self) -> &Bdd {
        &self.parametrizations
    }

    pub fn restricted(&self, restriction: &BddValuation) -> Bdd {
        self.parametrizations.support_set().iter()
            .fold(self.function.clone(),
                |acc, &bdd_var| acc.var_restrict(bdd_var, restriction[bdd_var]))
    }

    /// Colors has to be a subset of unit_bdd
    pub fn restricted_parametrizations(&self, colors: Bdd) -> Bdd {
        let support_set = self.function.support_set();
        colors.support_set().iter()
            .filter(|bdd_var| !support_set.contains(bdd_var))
            .fold(colors, |acc, bdd_var| acc.var_project(*bdd_var))
    }

    pub fn restricted_parametrizations_safe(&self, restriction: Bdd) -> Bdd {
        self.parametrizations.and(
            &self.restricted_parametrizations(restriction))
    }
}

pub type VarIndex = HashMap<BddVariable, usize>;

#[derive(Clone)]
pub struct SymbSyncGraph {
    bn: BooleanNetwork,
    context: SymbolicContext,
    unit_bdd: Bdd,
    pupdate_functions: Vec<ParedUpdateFunction>,
    total_update_function: Bdd,
    extra_state_var_equivalence: Bdd,
    var_index: VarIndex,
    all_false_bdd: Bdd,
}

impl SymbSyncGraph {
    pub fn new(bn: BooleanNetwork) -> SymbSyncGraph {
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

        let all_false_bdd = Bdd::from(
            BddValuation::all_false(context.bdd_variable_set().num_vars()));

        SymbSyncGraph {
            bn,
            context,
            unit_bdd,
            pupdate_functions,
            total_update_function,
            extra_state_var_equivalence,
            var_index,
            all_false_bdd,
        }
    }

    pub fn as_network(&self) -> &BooleanNetwork {
        &self.bn
    }

    pub fn symbolic_context(&self) -> &SymbolicContext {
        &self.context
    }

    pub fn get_pupdate_functions(&self) -> &[ParedUpdateFunction] {
        &self.pupdate_functions
    }

    pub fn get_var_index(&self) -> &VarIndex {
        &self.var_index
    }

    pub fn get_all_false(&self) -> &Bdd {
        &self.all_false_bdd
    }
}

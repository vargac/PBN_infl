use std::collections::HashMap;

use biodivine_lib_param_bn::{BooleanNetwork, VariableId, FnUpdate};
use biodivine_lib_param_bn::symbolic_async_graph::SymbolicContext;
use biodivine_lib_bdd::{Bdd, BddVariable, BddValuation};

use regulation_constraints::apply_regulation_constraints;


mod attractors;
mod regulation_constraints;
mod graph_operations;


/// Parametrized update function.
#[derive(Clone, Debug)]
pub struct ParedUpdateFunction {
    function: Bdd,
    parametrizations: Bdd,
    par_bdd_vars: Vec<BddVariable>,
}

/// Parametrized update function in an explicit form, so iterating over
/// all instantiated update fucntions is faster.
pub type PUpdateFunExplicit = Vec<Bdd>;

impl ParedUpdateFunction {
    /// Creates new `ParedUpdateFunction`
    ///
    /// * `update_function` - The update function. It usually contains
    ///     all the parametrizations.
    /// * `unit_bdd` - Valid parametrizations of the network
    /// * `context` - network's context
    /// * `fn_update` - Corresponding `FnUpdate`, if not implicit
    /// * `var_id` - Corresponding variable
    fn new(
        update_function: &Bdd,
        unit_bdd: &Bdd,
        context: &SymbolicContext,
        fn_update: &Option<FnUpdate>,
        var_id: VariableId)
    -> ParedUpdateFunction {
        let support_set = update_function.support_set();
        let mut parametrizations = unit_bdd.clone();
        for bdd_var in unit_bdd.support_set() {
            if !support_set.contains(&bdd_var) {
                parametrizations = parametrizations.var_project(bdd_var);
            }
        }

        let par_bdd_vars: Vec<BddVariable>;
        if let Some(fn_update) = fn_update {
            par_bdd_vars = fn_update.collect_parameters().iter()
                .map(|par_id| {
                    let table = context.get_explicit_function_table(*par_id);
                    table.into_iter().map(|(_, bdd_var)| bdd_var)
                })
                .flatten()
                .collect::<Vec<_>>();
        } else {
            let table = context.get_implicit_function_table(var_id);
            par_bdd_vars = table.into_iter()
                .map(|(_, bdd_var)| bdd_var)
                .collect::<Vec<_>>();
        }

        ParedUpdateFunction {
            function: update_function.and(&parametrizations),
            parametrizations,
            par_bdd_vars,
        }
    }

    /// Get the update function.
    ///
    /// The returned bdd contains all valid parametrizations.
    pub fn get_function(&self) -> &Bdd {
        &self.function
    }

    /// Get the valid parametrizations
    pub fn get_parametrizations(&self) -> &Bdd {
        &self.parametrizations
    }

    /// Get the parametrization variables.
    pub fn get_parameters(&self) -> &[BddVariable] {
        &self.par_bdd_vars
    }

    /// Restricted update function
    ///
    /// * `restriction` - Only `.get_parameters()` variables are taken
    ///     into account
    pub fn restricted(&self, restriction: &BddValuation) -> Bdd {
        self.par_bdd_vars.iter()
            .fold(self.function.clone(),
                |acc, &bdd_var| acc.var_restrict(bdd_var, restriction[bdd_var]))
    }

    /// Restricted valid parametrizations by `colors`.
    ///
    /// `colors` has to be a subset of the valid parametrizations.
    pub fn restricted_parametrizations(&self, colors: Bdd) -> Bdd {
        let support_set = self.function.support_set();
        colors.support_set().iter()
            .filter(|bdd_var| !support_set.contains(bdd_var))
            .fold(colors, |acc, bdd_var| acc.var_project(*bdd_var))
    }

    /// As well as the "unsafe" version, without the condition of being
    /// a subset.
    pub fn restricted_parametrizations_safe(&self, restriction: Bdd) -> Bdd {
        self.parametrizations.and(
            &self.restricted_parametrizations(restriction))
    }

    /// Computes the explicit form.
    ///
    /// * `colors` - used as in `restricted_parametrizations`.
    pub fn explicit_in(&self, colors: &Bdd, sync_graph: &SymbSyncGraph)
    -> PUpdateFunExplicit {
        self.restricted_parametrizations(colors.clone())
            .and(&sync_graph
                .get_all_false()
                .project(&self.par_bdd_vars))
            .sat_valuations()
            .map(|parametrization| self.restricted(&parametrization))
            .collect::<PUpdateFunExplicit>()
    }
}

/// Mapping a bdd-bariable to its index in symbolic context
pub type VarIndex = HashMap<BddVariable, usize>;

/// Symbolicaly stored graph of synchronuous semantics
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
    /// Creates new `SymbSyncGraph` from a `BooleanNetwork`
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
            .enumerate()
            .map(|(i, fun)| {
                let var_id = VariableId::from_index(i);
                ParedUpdateFunction::new(
                    fun,
                    &unit_bdd,
                    &context,
                    bn.get_update_function(var_id),
                    var_id)
            }).collect::<Vec<_>>();

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

    /// Return underlying boolean network
    pub fn as_network(&self) -> &BooleanNetwork {
        &self.bn
    }

    /// Return underlying symbolic context
    pub fn symbolic_context(&self) -> &SymbolicContext {
        &self.context
    }

    /// Return parametrized update functions
    ///
    /// The order of variables in `BooleanNetwork` gives the order of
    /// corresponding functions in the returned slice.
    pub fn get_pupdate_functions(&self) -> &[ParedUpdateFunction] {
        &self.pupdate_functions
    }

    /// Get the mapping from bbd-variable to index
    pub fn get_var_index(&self) -> &VarIndex {
        &self.var_index
    }

    /// All bdd-variables set to zero
    pub fn get_all_false(&self) -> &Bdd {
        &self.all_false_bdd
    }

    /// `get_pupdate_functions()` mapped to the explicit form
    pub fn explicit_pupdate_functions(&self, colors: &Bdd)
    -> Vec<PUpdateFunExplicit> {
        self.pupdate_functions.iter()
            .map(|pupdate_function|
                pupdate_function.explicit_in(colors, &self))
            .collect()
    }
}

use biodivine_lib_param_bn::symbolic_async_graph::SymbolicContext;
use biodivine_lib_param_bn::{BooleanNetwork, Monotonicity};
use biodivine_lib_bdd::{bdd, Bdd};

pub(crate) fn apply_regulation_constraints(
    initial: Bdd,
    network: &BooleanNetwork,
    context: &SymbolicContext,
) -> Result<Bdd, String> {
    // For each variable, compute Bdd that is true exactly when its update function is true.
    let update_function_is_true: Vec<Bdd> = network
        .variables()
        .map(|variable| {
            if let Some(function) = network.get_update_function(variable) {
                context.mk_fn_update_true(function)
            } else {
                context.mk_implicit_function_is_true(variable, &network.regulators(variable))
            }
        })
        .collect();

    let mut error_message = String::new();
    let mut unit_bdd = initial;
    for regulation in network.as_graph().regulations() {
        let regulator = context.get_state_variable(regulation.get_regulator());
        let regulator_is_true = context.bdd_variable_set().mk_var(regulator);
        let regulator_is_false = context.bdd_variable_set().mk_not_var(regulator);

        let fn_is_true = &update_function_is_true[regulation.get_target().to_index()];
        let fn_is_false = fn_is_true.not();

        /*
                           "Exists an input where value of f changes."
           (implicit \exists p_1, ..., p_m):
               \exists s_1, ..., s_n:
                   a <- \exists s_r: F(s_1, ..., s_r, ..., s_n, p_1, ..., p_m) = 1 and s_r = 1
                   b <- \exists s_r: F(s_1, ..., s_r, ..., s_n, p_1, ..., p_m) = 1 and s_r = 0
                   a != b
        */
        let observability = if regulation.is_observable() {
            // \exists x_r : F(x_1, ..., x_r, ..., x_n) & x_r | Context where F is one for x_r, but with x_r erased.
            let fn_x1_to_1 = bdd!(fn_is_true & regulator_is_true).var_project(regulator);
            // \exists x_r : F(x_1, ..., x_r, ..., x_m) & !x_r | Context where F is one for !x_r, but with x_r erased.
            let fn_x0_to_1 = bdd!(fn_is_true & regulator_is_false).var_project(regulator);
            // Context where F for x_r is not equal F for !x_r (i.e. all witnesses of observability)
            // and then with all states erased.
            bdd!(fn_x1_to_1 ^ fn_x0_to_1).project(context.state_variables())
        } else {
            context.mk_constant(true)
        };

        /* If observability failed, report error and continue. */
        if observability.is_false() {
            let problem = format!(
                " - {} has no effect in {}.\n",
                network.get_variable_name(regulation.get_regulator()),
                network.get_variable_name(regulation.get_target()),
            );
            error_message = format!("{}{}", error_message, problem);
        }

        /*
               "Exists an input where the functions monotonicity is reversed"
           (implicit \exists p_1, ..., p_m):
               \exists s_1, ..., s_m:
                   a <- \exists s_r: F(s_1, ..., s_r, ..., s_n, p_1, ..., p_m) = 0 and s_r = 1
                   b <- \exists s_r: F(s_1, ..., s_r, ..., s_n, p_1, ..., p_m) = 1 and s_r = 0
                   a & b   // "I can go from 1 to 0 by increasing s_r."
        */
        let non_monotonous = match regulation.get_monotonicity() {
            Some(Monotonicity::Activation) => {
                let fn_x1_to_0 = bdd!(fn_is_false & regulator_is_true).var_project(regulator);
                let fn_x0_to_1 = bdd!(fn_is_true & regulator_is_false).var_project(regulator);
                bdd!(fn_x0_to_1 & fn_x1_to_0).project(context.state_variables())
            }
            Some(Monotonicity::Inhibition) => {
                let fn_x0_to_0 = bdd!(fn_is_false & regulator_is_false).var_project(regulator);
                let fn_x1_to_1 = bdd!(fn_is_true & regulator_is_true).var_project(regulator);
                bdd!(fn_x0_to_0 & fn_x1_to_1).project(context.state_variables())
            }
            None => context.mk_constant(false),
        };
        let monotonicity = non_monotonous.not();

        if monotonicity.is_false() {
            let monotonicity_str = match regulation.get_monotonicity() {
                Some(Monotonicity::Activation) => "activating",
                Some(Monotonicity::Inhibition) => "inhibiting",
                None => "monotonous",
            };
            let problem = format!(
                " - {} not {} in {}.\n",
                network.get_variable_name(regulation.get_regulator()),
                monotonicity_str,
                network.get_variable_name(regulation.get_target()),
            );
            error_message = format!("{}{}", error_message, problem);
        }

        unit_bdd = bdd!(unit_bdd & (monotonicity & observability));
    }

    if unit_bdd.is_false() {
        Err(format!(
            "No update functions satisfy given constraints: \n{}",
            error_message
        ))
    } else {
        Ok(unit_bdd)
    }
}

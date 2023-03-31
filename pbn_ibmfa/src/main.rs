#![allow(unused_imports)]
#![allow(unused_mut)]

use std::{env, process, fs};
use std::collections::{HashMap, HashSet};

use biodivine_lib_param_bn::{BooleanNetwork, VariableId, FnUpdate};
use biodivine_lib_param_bn::biodivine_std::traits::Set;
use biodivine_lib_param_bn::symbolic_async_graph::
    {SymbolicContext, GraphColoredVertices, GraphColors, GraphVertices};
use biodivine_lib_bdd::{Bdd, BddVariable, BddPartialValuation, BddValuation};

mod _impl_regulation_constraint;
mod _impl_ibmfa_computations;

use crate::_impl_regulation_constraint::apply_regulation_constraints;
use crate::_impl_ibmfa_computations::*;


#[derive(Debug, Clone)]
struct UnitVertexFix {
    var_id: VariableId,
    value: bool,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct UnitParameterFix {
    bdd_var: BddVariable,
    value: bool,
}

#[derive(Debug, Clone)]
enum UnitFix {
    Vertex(UnitVertexFix),
    Parameter(UnitParameterFix),
}

type DriverSet = HashMap<VariableId, bool>;
type ColorsFix = Bdd;

#[derive(Debug)]
struct PBNFix {
    driver_set: DriverSet,
    colors_fix: ColorsFix,
    parameter_fixes: HashSet<UnitParameterFix>,
    unit_bdd: ColorsFix,
}

impl UnitVertexFix {
    fn to_str(&self, context: &SymbolicContext) -> String {
        let bdd_var = context.get_state_variable(self.var_id);
        format!("{}({})={}",
            bdd_var_to_str(bdd_var, &context),
            self.var_id.to_index(),
            if self.value { 1 } else { 0 })
    }
}

impl UnitParameterFix {
    fn to_str(&self, context: &SymbolicContext) -> String {
        format!("{}({})={}",
            bdd_var_to_str(self.bdd_var, &context),
            self.bdd_var,
            if self.value { 1 } else { 0 })
    }
}

impl UnitFix {
    fn to_str(&self, context: &SymbolicContext) -> String {
        match self {
            UnitFix::Vertex(fix) => fix.to_str(&context),
            UnitFix::Parameter(fix) => fix.to_str(&context),
        }
    }
}

impl PBNFix {
    fn new(unit_bdd: Bdd) -> Self {
        PBNFix {
            driver_set: HashMap::new(),
            colors_fix: unit_bdd.iff(&unit_bdd), // hack to create true bdd
            parameter_fixes: HashSet::new(),
            unit_bdd,
        }
    }

    fn insert(&mut self, fix: &UnitFix) {
        match fix {
            UnitFix::Vertex(fix) =>
                if self.driver_set.insert(fix.var_id, fix.value).is_some() {
                    panic!("Overriding driver-set by {:?}", fix);
                }
            UnitFix::Parameter(fix) => {
                self.colors_fix =
                    self.colors_fix.var_select(fix.bdd_var, fix.value);
                self.parameter_fixes.insert(fix.clone());
            }
        }
    }

    fn remove(&mut self, fix: &UnitFix) {
        match fix {
            UnitFix::Vertex(fix) =>
                if self.driver_set.remove(&fix.var_id).is_none() {
                    panic!("Fix not in the driver-set {:?}", fix);
                }
            UnitFix::Parameter(fix) => {
                if !self.parameter_fixes.remove(fix) {
                    panic!("Fix not among the parameter-fixes {:?}", fix);
                }
                self.colors_fix = self.colors_fix.var_project(fix.bdd_var);
            }
        }
    }

    fn colors(&self) -> ColorsFix {
        self.unit_bdd.and(&self.colors_fix)
    }

    fn get_vertex(&self, vertex: VariableId) -> Option<bool> {
        self.driver_set.get(&vertex).copied()
    }

    fn driver_set_to_str(&self, context: &SymbolicContext) -> String {
        format!("{{ {}}}", self.driver_set.iter()
            .map(|(&var_id, &value)| format!("{} ",
                (UnitVertexFix { var_id, value }).to_str(&context)))
            .collect::<String>())
    }

    fn par_fixes_to_str(&self, context: &SymbolicContext) -> String {
        format!("{{ {}}}", self.parameter_fixes.iter()
            .map(|par_fix| format!("{} ", par_fix.to_str(&context)))
            .collect::<String>())
    }

    fn colors_to_str(&self, context: &SymbolicContext) -> String {
        format!("{}",
            self.colors().to_boolean_expression(context.bdd_variable_set()))
    }

    fn to_str(&self, context: &SymbolicContext) -> String {
        format!("Driver-set: {}\nParameter-fixes: {}",
            self.driver_set_to_str(&context),
            self.par_fixes_to_str(&context))
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
    bn: BooleanNetwork,
    context: SymbolicContext,
    unit_bdd: Bdd,
    pupdate_functions: Vec<ParedUpdateFunction>,
    total_update_function: Bdd,
    extra_state_var_equivalence: Bdd,
    var_index: VarIndex,
}

impl BNetwork {
    fn new(bn: BooleanNetwork) -> BNetwork {
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
            bn,
            context,
            unit_bdd,
            pupdate_functions,
            total_update_function,
            extra_state_var_equivalence,
            var_index
        }
    }

    fn prepare_fixes(
        &self,
        attr_opt: Option<&GraphVertices>,
        colors: Bdd
    ) -> (Vec<UnitFix>, PBNFix) {
        let mut available_fixes = Vec::new();

        // Fixings of state variables
        for var_id in self.bn.variables() {
            for value in [false, true] {
                if let Some(attr) = attr_opt.as_ref() {
                    if attr.fix_network_variable(var_id, value).is_empty() {
                        continue;
                    }
                }
                let fix = UnitVertexFix { var_id, value };
                available_fixes.push(UnitFix::Vertex(fix));
            }
        }

        // Fixings of parameter variables
        for bdd_var in self.context.parameter_variables() {
            for value in [false, true] {
                let fix = UnitParameterFix { bdd_var: *bdd_var, value };
                available_fixes.push(UnitFix::Parameter(fix));
            }
        }

        let pbn_fix = PBNFix::new(colors);
        (self.filter_fixes(&available_fixes, &pbn_fix), pbn_fix)
    }

    fn filter_fixes(
        &self,
        fixes: &[UnitFix],
        pbn_fix: &PBNFix,
    ) -> Vec<UnitFix> {
        fixes.iter()
            .filter(|fix| match fix {
                UnitFix::Parameter(UnitParameterFix { bdd_var, value }) => {
                    let before = pbn_fix.colors();
                    let after = before.var_select(*bdd_var, *value);
                    !after.is_false() && after != before
                },
                UnitFix::Vertex(UnitVertexFix { var_id, .. }) =>
                    !pbn_fix.driver_set.contains_key(var_id)
            })
            .cloned()
            .collect()
    }

    // `fixings` is `mut`, but after the function call it remains the same as
    // before.
    fn minimize_entropy(
        &self,
        iterations: usize,
        pbn_fix: &mut PBNFix,
        available_fixes: &[UnitFix],
        verbose: bool,
    ) -> (usize, f32, Vec<f32>) {
        let (index, (entropy, probs)) = available_fixes.iter()
            .map(|unit_fix| {
                if verbose {
                    println!("Try fix {}", unit_fix.to_str(&self.context));
                }

                pbn_fix.insert(unit_fix);
                let (ent, probs) = ibmfa_entropy(
                    &self, &pbn_fix, iterations, false, false);
                pbn_fix.remove(unit_fix);

                if verbose {
                    println!("{ent}");
                }

                (ent, probs)
            })
            .enumerate()
            .min_by(|(_, (a, _)), (_, (b, _))| a.partial_cmp(b).unwrap())
            .unwrap();
        (index, entropy, probs)
    }

    fn find_driver_set(
        &self,
        iterations: usize,
        attr_opt: Option<(&GraphVertices, &GraphColors)>,
        verbose: bool
    ) -> (PBNFix, Vec<f32>) {
        let colors = match attr_opt {
            Some((_, attr_colors)) => attr_colors.as_bdd().clone(),
            None => self.unit_bdd.clone(),
        };
        let (mut available_fixes, mut pbn_fix) =
            self.prepare_fixes(attr_opt.map(|tup| tup.0), colors);

        let mut final_probs = Vec::new();

        while available_fixes.len() > 0 {
            if verbose {
                println!("======= {} ========", available_fixes.len());
            }

            let (min_entropy_index, min_entropy, probs) = self.minimize_entropy(
                iterations, &mut pbn_fix, &available_fixes, verbose);

            let unit_fix = &available_fixes[min_entropy_index];
            pbn_fix.insert(unit_fix);

            if verbose {
                println!("Fixing {}, entropy:{min_entropy}",
                    unit_fix.to_str(&self.context));
                println!("{}", pbn_fix.to_str(&self.context));
            }

            available_fixes = self.filter_fixes(&available_fixes, &pbn_fix);
            final_probs = probs;

            if min_entropy == 0.0 {
                break;
            }
        }
        (pbn_fix, final_probs)
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

    fn empty_colored_vertices(&self) -> GraphColoredVertices {
        GraphColoredVertices::new(
            self.context.mk_constant(false), &self.context)
    }

    fn unit_colors(&self) -> GraphColors {
        GraphColors::new(self.unit_bdd.clone(), &self.context)
    }

    fn fix_network_variable(&self, variable: VariableId, value: bool)
    -> GraphColoredVertices {
        let bdd_var = self.context.get_state_variable(variable);
        GraphColoredVertices::new(
            self.unit_bdd.var_select(bdd_var, value), &self.context)
    }

    fn search_loop(&self, initial: &GraphColoredVertices)
    -> GraphColoredVertices {
        let mut new = initial.clone();
        let mut all = initial.clone();
        let mut last = self.empty_colored_vertices();
        // find loop, "last" will be the first repeated vertex
        while !new.is_empty() {
            last = self.post_synch(&new);
            new = last.minus(&all);
            all = all.union(&new);
        }
        let mut result = last.clone();
        // get the whole loop
        while !last.is_empty() {
            last = self.post_synch(&last);
            last = last.minus(&result);
            result = result.union(&last);
        }
        result
    }

    fn predecessors(&self, initial: &GraphColoredVertices)
    -> GraphColoredVertices {
        let mut new = initial.clone();
        let mut result = initial.clone();
        while !new.is_empty() {
            new = self.pre_synch(&new);
            new = new.minus(&result);
            result = result.union(&new);
        }
        result
    }

    fn attractors_in(&self, set: &GraphColoredVertices)
    -> Vec<GraphColoredVertices> {
        let mut result = Vec::new();
        let mut all = set.clone();
        while !all.is_empty() {
            let attr = self.search_loop(&all.pick_vertex());
            all = all.minus(&self.predecessors(&attr));
            result.push(attr);
        }
        result
    }

    fn attractors(&self) -> Vec<GraphColoredVertices> {
        self.attractors_in(&self.unit_colored_vertices())
    }

    fn bdd_to_str(&self, bdd: &Bdd) -> String {
        format!("{}",
            bdd.to_boolean_expression(self.context.bdd_variable_set()))
    }

    fn attr_to_str(&self, attr: &GraphVertices) -> String {
        let all_false: Bdd = BddValuation::all_false(
            self.context.bdd_variable_set().num_vars()).into();
        format!("{{ {}}}", all_false
            .project(self.context.state_variables())
            .and(attr.as_bdd())
            .sat_valuations()
            .map(|bdd_valuation|
                format!("{}; ", self.context
                    .state_variables().iter()
                    .filter(|&bdd_var| bdd_valuation[*bdd_var])
                    .map(|bdd_var|
                        format!("{} ", bdd_var_to_str(*bdd_var, &self.context)))
                    .collect::<String>()))
            .collect::<String>())
    }
}

fn bdd_var_to_str(bdd_var: BddVariable, context: &SymbolicContext) -> String {
    context.bdd_variable_set().name_of(bdd_var)
}

fn valuation_to_str(valuation: &BddPartialValuation, context: &SymbolicContext)
-> String {
    format!("{:?}", valuation
        .to_values()
        .iter()
        .map(|&(bdd_var, val)|
            format!("{}={}", bdd_var_to_str(bdd_var, &context), val))
        .collect::<Vec<_>>()
    )
}

fn compute_attrs_map(attrs: &[GraphColoredVertices])
-> HashMap<GraphVertices, GraphColors> {
    let mut attrs_map = HashMap::new();
    for attr in attrs {
        let mut attr = attr.clone();
        while !attr.is_empty() {
            let mut wanted_vertices = attr
                .intersect_colors(&attr.colors().pick_singleton())
                .vertices();

            let one_attr_vertices = wanted_vertices.clone();

            let other_vertices = attr.vertices().minus(&one_attr_vertices);
            let mut one_attr_colors = attr
                .colors()
                .minus(&attr.intersect_vertices(&other_vertices).colors());

            while !wanted_vertices.is_empty() { // TODO just iterate over them
                let one_attr_vertex = wanted_vertices.pick_singleton();
                one_attr_colors = one_attr_colors.intersect(
                    &attr.intersect_vertices(&wanted_vertices).colors());
                wanted_vertices = wanted_vertices.minus(&one_attr_vertex);
            }

            attr = attr.minus_colors(&one_attr_colors);

            attrs_map
                .entry(one_attr_vertices)
                .and_modify(|colors: &mut GraphColors|
                    *colors = colors.union(&one_attr_colors))
                .or_insert(one_attr_colors);
        }
    }
    attrs_map
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
    let model = BooleanNetwork::try_from(model_string.as_str()).unwrap();
    println!("vars: {}, pars: {}", model.num_vars(), model.num_parameters());
    println!("vars: {:?}", model.variables()
        .map(|var_id| model.get_variable_name(var_id))
        .collect::<Vec<_>>()
    );
    println!();

    let bnetwork = BNetwork::new(model);

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
    let mut pbn_fix = PBNFix::new(bnetwork.unit_colors().into_bdd());

    let (e, p) = ibmfa_entropy(&bnetwork, &pbn_fix, iterations, true, true);
    println!("Entropy: {}, Probs: {:?}", e, p);

    let (pbn_fix, probs) = bnetwork.find_driver_set(iterations, None, true);

    println!("{}\n{:?}", pbn_fix.to_str(&bnetwork.context), probs);
    println!();

    let init_state = vec![false, false, false, false, false, false];
    let start = init_state.iter()
        .zip(bnetwork.bn.variables())
        .fold(bnetwork.unit_colored_vertices(),
            |acc, (&val, var_id)| acc.fix_network_variable(var_id, val));

    println!("{}", bnetwork.bdd_to_str(bnetwork.pre_synch(&start).as_bdd()));
    println!("{}", bnetwork.bdd_to_str(start.as_bdd()));
    println!("{}", bnetwork.bdd_to_str(bnetwork.post_synch(&start).as_bdd()));
    println!();

    let attrs = bnetwork.attractors();
    let attrs_map = compute_attrs_map(&attrs);

    println!("Attractors: {}", attrs_map.len());
    for (i, (attr, colors)) in attrs_map.iter().enumerate() {
        println!("{i} (size {}): {}",
            colors.approx_cardinality(), bnetwork.attr_to_str(attr));
        if attr.approx_cardinality() == 1.0 {
            let (pbn_fix, probs) = bnetwork.find_driver_set(
                iterations, Some((&attr, &colors)), false);
            println!("{:?}", probs);
            if !bnetwork.bn.variables().enumerate().all(|(i, var_id)|
                    (probs[i] == 1.0 || probs[i] == 0.0)
                    && !attr.fix_network_variable(
                        var_id, probs[i] != 0.0).is_empty()) {
                println!("WRONG");
            }
            for (var_id, value) in pbn_fix.driver_set {
                let unit_ver_fix = UnitVertexFix { var_id, value };
                println!("\t{}", unit_ver_fix.to_str(&bnetwork.context));
            }
            for unit_par_fix in pbn_fix.parameter_fixes {
                println!("\t{}", unit_par_fix.to_str(&bnetwork.context));
            }
        }
    }

    println!();

    let attr = ["v_miR_9", "v_zic5"];
    let attr_vertex_ids = attr.iter()
        .map(|name| bnetwork.bn.as_graph().find_variable(name).unwrap())
        .collect::<HashSet<_>>();
    let attr_vertices = bnetwork.bn.variables()
        .map(|var_id| (var_id, attr_vertex_ids.contains(&var_id)))
        .fold(bnetwork.unit_colored_vertices().vertices(),
            |acc, (var_id, val)| acc.fix_network_variable(var_id, val));
    println!("{}", bnetwork.attr_to_str(&attr_vertices));
    println!("{}", bnetwork.bdd_to_str(attrs_map[&attr_vertices].as_bdd()));

    bnetwork.find_driver_set(
        iterations,
        Some((&attr_vertices, &attrs_map[&attr_vertices])),
        true
    );
}

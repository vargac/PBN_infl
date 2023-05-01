use std::collections::HashSet;

use biodivine_lib_bdd::{Bdd, BddVariableSet};
use biodivine_lib_param_bn::symbolic_async_graph::
    {GraphVertices, GraphColors, SymbolicContext};

use crate::ibmfa_computations::minimize_entropy;
use crate::driver_set::{find_driver_set, driver_set_to_str, PBNFix, UnitFix,
    fixes::{DriverSet, UnitParameterFix}};
use crate::symbolic_sync_graph::SymbSyncGraph;
use crate::utils::bdd_to_str;

#[derive(Clone, Debug)]
pub struct DecisionNode {
    childs: [Box<DecisionTree>; 2],
    color_fix: Bdd,
}

#[derive(Clone, Debug)]
pub enum DecisionTree {
    Node(DecisionNode),
    Leaf(DriverSet),
}

impl DecisionNode {
    pub fn get_childs(&self) -> &[Box<DecisionTree>; 2] {
        &self.childs
    }

    pub fn get_fix(&self) -> &Bdd {
        &self.color_fix
    }
}

impl DecisionTree {
    pub fn to_str(&self, context: &SymbolicContext) -> String {
        self.to_str_rec(0, context)
    }

    fn to_str_rec(&self, level: usize, context: &SymbolicContext) -> String {
        match self {
            DecisionTree::Node(node) => {
                let indent = " ".repeat(level);
                format!("{}\n{indent}-0- {}\n{indent}-1- {}",
                    bdd_to_str(&node.color_fix, context),
                    node.childs[0].to_str_rec(level + 4, context),
                    node.childs[1].to_str_rec(level + 4, context))
                },
            DecisionTree::Leaf(driver_set) =>
                driver_set_to_str(driver_set, context),
        }
    }
}


/*******************************************************************************
 * Finding a decision tree for a colors partition
 * ==============================================
 * Having the colors partitioned and assinged a driver set for each
 * partition, find a binary decision tree leading to leaves consisting of
 * the partitions. A decision node contains only a single parameter for now.
 ******************************************************************************/

struct UnresolvedNode {
    colors: Bdd,
    driver_sets: Vec<(Bdd, DriverSet)>,
}

impl UnresolvedNode {
    fn new(colors: Bdd) -> Self {
        UnresolvedNode { colors, driver_sets: Vec::new() }
    }

    fn entropy(&self) -> f64 {
        let e = self.driver_sets.iter()
            .map(|(colors, _)| {
                let c = colors.cardinality();
                if c == 0.0 { 0.0 } else { c * c.log2() }
            }).sum::<f64>();
        let total = self.colors.cardinality();
        if total == 0.0 { 0.0 } else { total.log2() - e / total }
    }
}

pub fn decision_tree_from_partition(
    all_colors: &Bdd,
    driver_sets: &[(Bdd, DriverSet)],
    bdd_variable_set: &BddVariableSet,
) -> DecisionTree {
    fn split_unresolved(
        mut node: UnresolvedNode,
        mut pars: HashSet<Bdd>,
    ) -> DecisionTree {
        if node.driver_sets.is_empty() {
            return DecisionTree::Leaf(DriverSet::new());
        }
        if node.driver_sets.len() == 1 {
            return DecisionTree::Leaf(node.driver_sets.pop().unwrap().1);
        }

        let (par, split_nodes) =
            best_decision_par(&pars, &node).unwrap();
        pars.remove(&par);
        let trees = split_nodes.map(|split_node|
                Box::new(split_unresolved(split_node, pars.clone())));

        DecisionTree::Node(DecisionNode {
            childs: trees,
            color_fix: par
        })
    }

    let node = UnresolvedNode {
        colors: all_colors.clone(),
        driver_sets: Vec::from(driver_sets)
    };
    let pars = all_colors.support_set().iter()
        .map(|bdd_var| bdd_variable_set.mk_var(*bdd_var))
        .collect::<HashSet<_>>();

    split_unresolved(node, pars)
}

fn best_decision_par<'a>(
    pars: impl IntoIterator<Item = &'a Bdd>,
    node: &UnresolvedNode,
) -> Option<(Bdd, [UnresolvedNode; 2])> {
    // Maximization of information gain = Minimization of information entropy
    pars.into_iter()
        .map(|par| {
            let mut split_nodes = [
                UnresolvedNode::new(node.colors.and_not(par)),
                UnresolvedNode::new(node.colors.and(par))
            ];

            for (colors, driver_set) in &node.driver_sets {
                for split_node in split_nodes.as_mut_slice() {
                    let subcolors = colors.and(&split_node.colors);
                    if !subcolors.is_false() {
                        split_node.driver_sets.push(
                            (subcolors, driver_set.clone()));
                    }
                }
            }

            (par, split_nodes)
        })
        .min_by(|(_, sns1), (_, sns2)| {
            let e1 = sns1[0].entropy() + sns1[1].entropy();
            let e2 = sns2[0].entropy() + sns2[1].entropy();
            e1.partial_cmp(&e2).unwrap()
        })
        .map(|(par, sns)| (par.clone(), sns))
}


/*******************************************************************************
 * Building decision trees along with running the ibmfa simulation.
 * ================================================================
 * ibmfa -> driver set -> choose one parameter fix -> make decision node ->
 * -> restrict colors to a negation of the fix -> ibmfa -> ...
 * Does not work well. It depends on heuristics on possible parameter
 * fixes -- single fixes of one parameter are not sufficient, the minimization
 * greedy search favors fixing vertices rather than parameters (after running
 * the postprocessing reduction). Even with better heuristics (and thus
 * having many more parameter fixes -> time complexity grows), the reduction
 * may still remove all parameter fixes if the "simplest" driver set (e.i.
 * the first one found) is the one that works as well for other colors.
 * Those colors may have been fixed because the entropy gets low faster in them.
 * So it seems to be a good idea to use entropy minimization for finding
 * vertices fix (the original "driver-set" meaning) but not parameters fix.
 ******************************************************************************/

pub fn decision_tree(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    attr: (&GraphVertices, &GraphColors),
    reduced: bool,
) -> DecisionTree {
    let (pbn_fix, _) = find_driver_set(
        sync_graph, iterations, reduced, Some(attr), false, false);
    decision_tree_recursive(sync_graph, iterations, attr, pbn_fix, reduced)
}

fn decision_tree_recursive(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    attr: (&GraphVertices, &GraphColors),
    mut pbn_fix: PBNFix,
    reduced: bool,
) -> DecisionTree {
    if pbn_fix.get_parameter_fixes().is_empty() {
        return DecisionTree::Leaf(pbn_fix.get_driver_set().clone());
    }

    let mut pbn_fix_copy = pbn_fix.clone();
    pbn_fix_copy.clear_colors_fix();

    let available_fixings = pbn_fix.get_parameter_fixes()
        .iter()
        .map(|fix| UnitFix::Parameter(fix.clone()))
        .collect::<Vec<_>>();
    let unit_fix = minimize_entropy(
        &sync_graph, iterations, &mut pbn_fix_copy,
        &available_fixings, None, false
    ).map(|(unit_fix, _, _)| unit_fix).unwrap();

    let UnitParameterFix { bdd_var, value } = match unit_fix {
        UnitFix::Parameter(fix) => fix.clone(),
        UnitFix::Vertex(_) => panic!("Expected parameter"),
    };

    pbn_fix.remove(unit_fix);

    let subtree_value = Box::new(decision_tree_recursive(
            &sync_graph,
            iterations,
            (&attr.0,
             &attr.1.copy(attr.1.as_bdd().var_select(bdd_var, value))),
            pbn_fix,
            reduced
    ));

    let colors_neg_value = 
        attr.1.copy(attr.1.as_bdd().var_select(bdd_var, !value));
    let attr_neg_value = (attr.0, &colors_neg_value);

    let (pbn_fix, _) = find_driver_set(
        &sync_graph,
        iterations,
        reduced,
        Some(attr_neg_value),
        false,
        false
    );

    let subtree_neg_value = Box::new(decision_tree_recursive(
            &sync_graph,
            iterations,
            attr_neg_value,
            pbn_fix,
            reduced,
    ));

    let (low, high) =
        if value { (subtree_neg_value, subtree_value) }
        else     { (subtree_value, subtree_neg_value) };

    let color_fix = sync_graph.symbolic_context()
        .bdd_variable_set().mk_var(bdd_var);

    DecisionTree::Node(DecisionNode { childs: [low, high], color_fix })
}

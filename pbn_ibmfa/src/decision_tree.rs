use biodivine_lib_bdd::BddVariable;
use biodivine_lib_param_bn::symbolic_async_graph::
    {GraphVertices, GraphColors, SymbolicContext};

use crate::ibmfa_computations::minimize_entropy;
use crate::driver_set::{find_driver_set, find_reduced_driver_set,
    driver_set_to_str, PBNFix, UnitFix, fixes::{DriverSet, UnitParameterFix}};
use crate::symbolic_sync_graph::SymbSyncGraph;
use crate::utils::bdd_var_to_str;

#[derive(Clone, Debug)]
pub struct DecisionNode {
    childs: [Box<DecisionTree>; 2],
    color_fix: BddVariable,
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

    pub fn get_fix(&self) -> BddVariable {
        self.color_fix
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
                    bdd_var_to_str(node.color_fix, context),
                    node.childs[0].to_str_rec(level + 4, context),
                    node.childs[1].to_str_rec(level + 4, context))
                },
            DecisionTree::Leaf(driver_set) =>
                driver_set_to_str(driver_set, context),
        }
    }
}

pub fn decision_tree(
    sync_graph: &SymbSyncGraph,
    iterations: usize,
    attr: (&GraphVertices, &GraphColors),
    reduced: bool,
) -> DecisionTree {
    let (pbn_fix, _) = if reduced {
        find_reduced_driver_set(&sync_graph, iterations, Some(attr), false)
    } else {
        find_driver_set(&sync_graph, iterations, Some(attr), false)
    };
    decision_tree_recursive(&sync_graph, iterations, attr, pbn_fix, reduced)
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
        &available_fixings, false
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

    let (pbn_fix, _) = if reduced {
        find_reduced_driver_set(
            &sync_graph,
            iterations,
            Some(attr_neg_value),
            false
        )
    } else {
        find_driver_set(
            &sync_graph,
            iterations,
            Some(attr_neg_value),
            false
        )
    };

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

    DecisionTree::Node(DecisionNode { childs: [low, high], color_fix: bdd_var })
}

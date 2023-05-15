use std::collections::HashMap;

use biodivine_lib_param_bn::{
    symbolic_async_graph::{GraphColoredVertices, GraphVertices, GraphColors},
    biodivine_std::traits::Set};

use super::SymbSyncGraph;

impl SymbSyncGraph {
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

    /// Finds attractors (both fixed-point and cyclic).
    ///
    /// Typically should be postprocessed by `compute_attrs_map()` function.
    ///
    /// * `set` - Attractors are found only in this set.
    pub fn attractors_in(&self, set: &GraphColoredVertices)
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

    /// Finds all attractors (both fixed-point and cyclic).
    pub fn attractors(&self) -> Vec<GraphColoredVertices> {
        self.attractors_in(&self.unit_colored_vertices())
    }

    /// Finds fixed-point attractors.
    ///
    /// * `set` - Attractors are found only in this set.
    pub fn fixed_point_attractors_in(&self, set: &GraphColoredVertices)
    -> Vec<GraphColoredVertices> {
        let attrs = set.copy(set.as_bdd()
            .and(&self.total_update_function)
            .and(&self.extra_state_var_equivalence)
            .project(self.context.all_extra_state_variables()));
        attrs.raw_projection(self.context.parameter_variables())
            .iter()
            .map(|valuation|
                set.copy(attrs.as_bdd().select(&valuation.to_values())))
            .collect::<Vec<_>>()
    }

    /// Finds all fixed-point attractors.
    pub fn fixed_point_attractors(&self) -> Vec<GraphColoredVertices> {
        self.fixed_point_attractors_in(&self.unit_colored_vertices())
    }
}

/// Postprocess after running `SymbSyncGraph::attractors`.
#[allow(dead_code)]
pub fn compute_attrs_map(attrs: &[GraphColoredVertices])
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

            while !wanted_vertices.is_empty() {
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

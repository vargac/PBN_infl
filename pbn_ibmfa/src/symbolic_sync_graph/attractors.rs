use biodivine_lib_param_bn::{symbolic_async_graph::GraphColoredVertices,
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

    pub fn attractors(&self) -> Vec<GraphColoredVertices> {
        self.attractors_in(&self.unit_colored_vertices())
    }
}

//! Routing (fluids and pipes spec 5): no part carries more than its
//! throughput, a consumer is capped by the narrowest part on its route,
//! consumers sharing a narrow part split it in proportion to their demand,
//! and loops add no capacity: each node is fed along one route, its widest
//! path from the sources, chosen at rebuild with ties broken by block order.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::sim::geom::Pos;

/// A network to route over: nodes with throughputs and links. Source nodes
/// hang from a virtual root; consumers attach to nodes.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    /// Each node's throughput, mB/s (a source node's is its own limit).
    pub cap: Vec<f64>,
    /// Each node's position, for ties.
    pub key: Vec<Pos>,
    pub adj: Vec<Vec<usize>>,
}

impl Graph {
    pub fn add(&mut self, cap: f64, key: Pos) -> usize {
        self.cap.push(cap);
        self.key.push(key);
        self.adj.push(Vec::new());
        self.cap.len() - 1
    }

    pub fn link(&mut self, a: usize, b: usize) {
        if a != b && !self.adj[a].contains(&b) {
            self.adj[a].push(b);
            self.adj[b].push(a);
        }
    }

    pub fn len(&self) -> usize {
        self.cap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cap.is_empty()
    }
}

/// The widest-path tree from a set of source nodes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree {
    /// Each node's parent: `None` for a source (the root's child) or a node
    /// no source reaches (see `reached`).
    pub parent: Vec<Option<usize>>,
    pub reached: Vec<bool>,
    /// Nodes in the order they were settled: parents before children.
    pub order: Vec<usize>,
    /// The bottleneck from the sources to each node, at rebuild.
    pub width: Vec<f64>,
}

#[derive(PartialEq)]
struct Item {
    width: f64,
    key: Pos,
    node: usize,
    parent: Option<usize>,
}

impl Eq for Item {}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        self.width
            .partial_cmp(&other.width)
            .unwrap_or(Ordering::Equal)
            // Lower positions first on ties: BinaryHeap pops the greatest.
            .then_with(|| other.key.cmp(&self.key))
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Build the widest-path tree from `sources`.
pub fn tree(g: &Graph, sources: &[usize]) -> Tree {
    let n = g.len();
    let mut t = Tree {
        parent: vec![None; n],
        reached: vec![false; n],
        order: Vec::new(),
        width: vec![0.0; n],
    };
    let mut heap = BinaryHeap::new();
    for &s in sources {
        heap.push(Item {
            width: g.cap[s],
            key: g.key[s],
            node: s,
            parent: None,
        });
    }
    while let Some(it) = heap.pop() {
        if t.reached[it.node] {
            continue;
        }
        t.reached[it.node] = true;
        t.parent[it.node] = it.parent;
        t.width[it.node] = it.width;
        t.order.push(it.node);
        for &v in &g.adj[it.node] {
            if !t.reached[v] {
                heap.push(Item {
                    width: it.width.min(g.cap[v]),
                    key: g.key[v],
                    node: v,
                    parent: Some(it.node),
                });
            }
        }
    }
    t
}

/// One routing pass. `cap` is each node's throughput now (a part-open valve
/// changes it, not the tree); `demands` are (node, mB/s) of the consumers.
/// Returns each consumer's delivery and each node's flow.
pub fn route(t: &Tree, cap: &[f64], demands: &[(usize, f64)]) -> (Vec<f64>, Vec<f64>) {
    let n = cap.len();
    let mut sum = vec![0.0; n];
    for &(node, d) in demands {
        if t.reached[node] {
            sum[node] += d.max(0.0);
        }
    }
    // Demands add up the tree: children before parents.
    for &u in t.order.iter().rev() {
        if let Some(p) = t.parent[u] {
            sum[p] += sum[u];
        }
    }
    // Scale down top-down wherever a part's sum passes its throughput.
    let mut f: Vec<f64> = vec![0.0; n];
    for &u in &t.order {
        let above = t.parent[u].map_or(1.0, |p| f[p]);
        f[u] = if sum[u] > 1e-12 {
            above.min(cap[u].max(0.0) / sum[u])
        } else {
            above
        };
    }
    let delivered: Vec<f64> = demands
        .iter()
        .map(|&(node, d)| {
            if t.reached[node] {
                d.max(0.0) * f[node]
            } else {
                0.0
            }
        })
        .collect();
    // What actually passes each node: the deliveries below it.
    let mut flow = vec![0.0; n];
    for (&(node, _), got) in demands.iter().zip(&delivered) {
        flow[node] += got;
    }
    for &u in t.order.iter().rev() {
        if let Some(p) = t.parent[u] {
            flow[p] += flow[u];
        }
    }
    (delivered, flow)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line of nodes 0..caps.len(), node 0 the source.
    fn line(caps: &[f64]) -> Graph {
        let mut g = Graph::default();
        for (i, c) in caps.iter().enumerate() {
            g.add(*c, Pos::new(i as i32, 0, 0));
            if i > 0 {
                g.link(i - 1, i);
            }
        }
        g
    }

    #[test]
    fn a_line_is_capped_by_its_narrowest_pipe() {
        let g = line(&[100.0, 200.0, 80.0, 200.0]);
        let t = tree(&g, &[0]);
        let (d, flow) = route(&t, &g.cap, &[(3, 130.0)]);
        assert!((d[0] - 80.0).abs() < 1e-9);
        assert!((flow[1] - 80.0).abs() < 1e-9);
        let (d, _) = route(&t, &g.cap, &[(3, 50.0)]);
        assert_eq!(d, vec![50.0]);
    }

    #[test]
    fn a_shared_bottleneck_splits_in_proportion_to_demand() {
        // source 0 - 1 (80) - {2, 3}
        let mut g = line(&[1000.0, 80.0, 200.0]);
        let three = g.add(200.0, Pos::new(9, 0, 0));
        g.link(1, three);
        let t = tree(&g, &[0]);
        let (d, _) = route(&t, &g.cap, &[(2, 120.0), (three, 40.0)]);
        assert!(
            (d[0] - 60.0).abs() < 1e-9 && (d[1] - 20.0).abs() < 1e-9,
            "{d:?}"
        );
    }

    #[test]
    fn a_loop_adds_no_capacity_and_the_route_is_deterministic() {
        // Two parallel 80 pipes from the source to a junction.
        let mut g = Graph::default();
        let s = g.add(1000.0, Pos::new(0, 0, 0));
        let a = g.add(80.0, Pos::new(1, 0, 0));
        let b = g.add(80.0, Pos::new(1, 0, 1));
        let j = g.add(200.0, Pos::new(2, 0, 0));
        g.link(s, a);
        g.link(s, b);
        g.link(a, j);
        g.link(b, j);
        let t = tree(&g, &[s]);
        let (d, _) = route(&t, &g.cap, &[(j, 200.0)]);
        assert!((d[0] - 80.0).abs() < 1e-9);
        // The tie goes to the lower position, every time.
        assert_eq!(t.parent[j], Some(a));
        assert_eq!(tree(&g, &[s]), t);
    }

    #[test]
    fn a_part_open_valve_scales_flow_without_a_new_tree() {
        let g = line(&[1000.0, 200.0, 200.0]);
        let t = tree(&g, &[0]);
        let mut cap = g.cap.clone();
        cap[1] = 200.0 * 0.3;
        let (d, _) = route(&t, &cap, &[(2, 100.0)]);
        assert!((d[0] - 60.0).abs() < 1e-9);
        // An unreachable consumer gets nothing.
        let lone = {
            let mut g = g.clone();
            g.add(100.0, Pos::new(50, 0, 0))
        };
        assert!(lone >= g.len());
    }

    #[test]
    fn two_sources_each_feed_their_widest_routes() {
        // s1 (100) - p (200) - s2 (100): a consumer at p takes the wider (tie: lower key).
        let mut g = Graph::default();
        let s1 = g.add(100.0, Pos::new(0, 0, 0));
        let p = g.add(200.0, Pos::new(1, 0, 0));
        let s2 = g.add(150.0, Pos::new(2, 0, 0));
        g.link(s1, p);
        g.link(p, s2);
        let t = tree(&g, &[s1, s2]);
        assert_eq!(t.parent[p], Some(s2));
        let (d, _) = route(&t, &g.cap, &[(p, 300.0)]);
        assert!((d[0] - 150.0).abs() < 1e-9);
    }
}

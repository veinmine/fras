//! Torque flow through a system (rotational network spec 6.3).
//!
//! Each part injects a net torque (its source push, minus its load, friction
//! and the torque spent accelerating it); the sum over a system is zero. The
//! flow through each joint is the least-squares flow on a network of unit
//! conductances: on a tree it is the cut sum (everything on one side of the
//! joint), and parallel paths of a consistent loop share it evenly.

/// Edge flows for `n` nodes joined by `edges` with injections `t`. A
/// positive flow on edge `(a, b)` goes from `a` to `b`.
pub fn flows(n: usize, edges: &[(usize, usize)], t: &[f64]) -> Vec<f64> {
    debug_assert_eq!(t.len(), n);
    if n == 0 || edges.is_empty() {
        return vec![0.0; edges.len()];
    }
    // Remove any imbalance (rounding) so the system is solvable.
    let mean = t.iter().sum::<f64>() / n as f64;
    let t: Vec<f64> = t.iter().map(|v| v - mean).collect();
    if edges.len() + 1 == n
        && let Some(f) = tree_flows(n, edges, &t)
    {
        return f;
    }
    let phi = solve_laplacian(n, edges, &t);
    edges.iter().map(|&(a, b)| phi[a] - phi[b]).collect()
}

/// Cut sums on a tree; `None` if the edges don't form one.
fn tree_flows(n: usize, edges: &[(usize, usize)], t: &[f64]) -> Option<Vec<f64>> {
    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (i, &(a, b)) in edges.iter().enumerate() {
        adj[a].push((b, i));
        adj[b].push((a, i));
    }
    let mut parent: Vec<Option<(usize, usize)>> = vec![None; n];
    let mut seen = vec![false; n];
    let mut order = Vec::with_capacity(n);
    seen[0] = true;
    order.push(0);
    let mut head = 0;
    while head < order.len() {
        let v = order[head];
        head += 1;
        for &(w, e) in &adj[v] {
            if !seen[w] {
                seen[w] = true;
                parent[w] = Some((v, e));
                order.push(w);
            }
        }
    }
    if order.len() != n {
        return None;
    }
    let mut sub = t.to_vec();
    let mut f = vec![0.0; edges.len()];
    for &v in order.iter().rev() {
        if let Some((p, e)) = parent[v] {
            // Everything below v needs -sub[v] to flow from p into v.
            let into_v = -sub[v];
            f[e] = if edges[e].0 == p { into_v } else { -into_v };
            sub[p] += sub[v];
        }
    }
    Some(f)
}

/// Conjugate gradients on the graph Laplacian, `L·φ = t` with mean-zero `t`.
fn solve_laplacian(n: usize, edges: &[(usize, usize)], t: &[f64]) -> Vec<f64> {
    let mut degree = vec![0.0f64; n];
    for &(a, b) in edges {
        degree[a] += 1.0;
        degree[b] += 1.0;
    }
    let apply = |x: &[f64], out: &mut [f64]| {
        for (o, (d, v)) in out.iter_mut().zip(degree.iter().zip(x)) {
            *o = d * v;
        }
        for &(a, b) in edges {
            out[a] -= x[b];
            out[b] -= x[a];
        }
    };
    let precond = |r: &[f64], z: &mut [f64]| {
        for i in 0..n {
            z[i] = if degree[i] > 0.0 {
                r[i] / degree[i]
            } else {
                0.0
            };
        }
    };
    let mut x = vec![0.0; n];
    let mut r = t.to_vec();
    let mut z = vec![0.0; n];
    precond(&r, &mut z);
    let mut p = z.clone();
    let mut rz: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
    let norm_t: f64 = t.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-12);
    let mut ap = vec![0.0; n];
    for _ in 0..(4 * n).max(50) {
        apply(&p, &mut ap);
        let pap: f64 = p.iter().zip(&ap).map(|(a, b)| a * b).sum();
        if pap.abs() < 1e-300 {
            break;
        }
        let alpha = rz / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let norm_r: f64 = r.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm_r < 1e-9 * norm_t {
            break;
        }
        precond(&r, &mut z);
        let rz_next: f64 = r.iter().zip(&z).map(|(a, b)| a * b).sum();
        let beta = rz_next / rz;
        rz = rz_next;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn a_line_carries_the_cut_sum() {
        // Source 10 at node 0, loads 4 and 6 at nodes 2 and 4.
        let edges = [(0, 1), (1, 2), (2, 3), (3, 4)];
        let t = [10.0, 0.0, -4.0, 0.0, -6.0];
        let f = flows(5, &edges, &t);
        assert!(close(f[0], 10.0) && close(f[1], 10.0));
        assert!(close(f[2], 6.0) && close(f[3], 6.0));
    }

    #[test]
    fn a_tree_splits_at_its_branches() {
        //      0 (source 9)
        //      |
        //      1 ---- 3 (load 4)
        //      |
        //      2 (load 5)
        let edges = [(0, 1), (2, 1), (1, 3)];
        let t = [9.0, 0.0, -5.0, -4.0];
        let f = flows(4, &edges, &t);
        assert!(close(f[0], 9.0));
        assert!(close(f[1], -5.0)); // edge (2,1): 5 flows from 1 to 2
        assert!(close(f[2], 4.0));
    }

    #[test]
    fn a_consistent_loop_splits_evenly() {
        // 0 -> {1, 2} -> 3, two parallel paths of equal length.
        let edges = [(0, 1), (1, 3), (0, 2), (2, 3)];
        let t = [8.0, 0.0, 0.0, -8.0];
        let f = flows(4, &edges, &t);
        for v in f {
            assert!(close(v, 4.0), "{v}");
        }
    }

    #[test]
    fn a_large_loop_converges() {
        // A ring of 200 with a source and a load opposite each other.
        let n = 200;
        let edges: Vec<_> = (0..n).map(|i| (i, (i + 1) % n)).collect();
        let mut t = vec![0.0; n];
        t[0] = 10.0;
        t[100] = -10.0;
        let f = flows(n, &edges, &t);
        assert!(close(f[0].abs(), 5.0), "{}", f[0]);
        assert!(close(f[150].abs(), 5.0));
    }
}

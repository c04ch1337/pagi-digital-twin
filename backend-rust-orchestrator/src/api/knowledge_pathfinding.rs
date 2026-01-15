//! Depth-limited multi-hop pathfinding used by `/api/knowledge/path`.
//!
//! Implementation notes:
//! - Uses `petgraph::algo::astar` with a zero heuristic (i.e. Dijkstra).
//! - Supports `max_depth` (max hops) by expanding the base graph into layered states `(node, depth)`.
//! - Edge weight is `1.0 - cross_encoder_score` clamped to `[0.0, 1.0]`.
//! - If an edge score is missing or invalid (NaN/inf), we use a conservative default score of `0.0`
//!   (i.e. maximum weight/cost of `1.0`).

use petgraph::{
    algo::astar,
    graph::{DiGraph, NodeIndex},
};
use std::collections::HashMap;

/// Conservative default when a score is missing.
///
/// `0.0` means the edge is treated as maximally costly (`weight = 1.0`).
const DEFAULT_MISSING_SCORE: f64 = 0.0;

#[derive(Clone, Debug)]
pub struct EdgeInput {
    pub source_id: String,
    pub target_id: String,
    pub cross_encoder_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathEdge {
    pub source_id: String,
    pub target_id: String,
    pub cross_encoder_score: f64,
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathResult {
    pub node_ids: Vec<String>,
    pub edges: Vec<PathEdge>,
    pub total_weight: f64,
}

#[derive(Clone, Copy, Debug)]
struct EdgeMeta {
    score: f64,
    weight: f64,
}

fn sanitize_score(score: Option<f64>) -> f64 {
    match score {
        Some(s) if s.is_finite() => s.clamp(0.0, 1.0),
        _ => DEFAULT_MISSING_SCORE,
    }
}

fn score_to_weight(score: f64) -> f64 {
    (1.0 - score).clamp(0.0, 1.0)
}

/// Find a minimum-total-weight path from `source_id` to `target_id` with at most `max_depth` hops.
///
/// Returns `None` if either node is missing, or if no path exists within the hop limit.
pub fn find_path_with_max_depth(
    nodes: &[String],
    edges: &[EdgeInput],
    source_id: &str,
    target_id: &str,
    max_depth: usize,
) -> Option<PathResult> {
    if source_id == target_id {
        return Some(PathResult {
            node_ids: vec![source_id.to_string()],
            edges: Vec::new(),
            total_weight: 0.0,
        });
    }

    if max_depth == 0 {
        return None;
    }

    // Map node id -> ordinal index (stable).
    let mut node_ord: HashMap<&str, usize> = HashMap::with_capacity(nodes.len());
    for (i, id) in nodes.iter().enumerate() {
        node_ord.insert(id.as_str(), i);
    }

    let &src = node_ord.get(source_id)?;
    let &dst = node_ord.get(target_id)?;

    // Deduplicate edges and compute weights.
    // If duplicates exist, keep the cheapest (lowest weight) deterministically.
    let mut edge_meta: HashMap<(usize, usize), EdgeMeta> = HashMap::new();
    for e in edges {
        let Some(&u) = node_ord.get(e.source_id.as_str()) else {
            continue;
        };
        let Some(&v) = node_ord.get(e.target_id.as_str()) else {
            continue;
        };

        let score = sanitize_score(e.cross_encoder_score);
        let weight = score_to_weight(score);
        let meta = EdgeMeta { score, weight };

        edge_meta
            .entry((u, v))
            .and_modify(|prev| {
                // Prefer lower weight. If tied, prefer higher score.
                if meta.weight < prev.weight || (meta.weight == prev.weight && meta.score > prev.score) {
                    *prev = meta;
                }
            })
            .or_insert(meta);
    }

    // Deterministic edge iteration.
    let mut edge_list: Vec<((usize, usize), EdgeMeta)> = edge_meta.into_iter().collect();
    edge_list.sort_by(|a, b| a.0.cmp(&b.0));

    // Build layered graph: (node, depth) where each hop increments depth.
    // Also add a super-goal node to allow `astar` to target "any depth" reaching dst.
    let n = nodes.len();
    let mut g: DiGraph<(usize, usize), f64> = DiGraph::new();
    let mut layer_nodes: Vec<Vec<NodeIndex>> = vec![vec![NodeIndex::end(); max_depth + 1]; n];
    for i in 0..n {
        for d in 0..=max_depth {
            layer_nodes[i][d] = g.add_node((i, d));
        }
    }

    let super_goal = g.add_node((usize::MAX, max_depth + 1));
    for d in 0..=max_depth {
        g.add_edge(layer_nodes[dst][d], super_goal, 0.0);
    }

    for ((u, v), meta) in &edge_list {
        for d in 0..max_depth {
            g.add_edge(layer_nodes[*u][d], layer_nodes[*v][d + 1], meta.weight);
        }
    }

    let start = layer_nodes[src][0];

    let (_cost, path) = astar(
        &g,
        start,
        |finish| finish == super_goal,
        |e| *e.weight(),
        |_| 0.0,
    )?;

    // Convert layered path into original node id list, skipping the super-goal.
    if path.len() < 2 {
        return None;
    }

    let mut ord_path: Vec<usize> = Vec::with_capacity(path.len());
    for &idx in &path {
        if idx == super_goal {
            break;
        }
        let (ord, _depth) = g[idx];
        ord_path.push(ord);
    }

    if ord_path.is_empty() || *ord_path.last()? != dst {
        return None;
    }

    let mut out_nodes: Vec<String> = Vec::with_capacity(ord_path.len());
    for ord in &ord_path {
        out_nodes.push(nodes[*ord].clone());
    }

    // Rebuild per-edge metadata.
    let mut out_edges: Vec<PathEdge> = Vec::with_capacity(ord_path.len().saturating_sub(1));
    let mut total_weight = 0.0;

    // Reconstruct an index for quick lookup.
    let meta_lookup: HashMap<(usize, usize), EdgeMeta> = edge_list.into_iter().collect();

    for w in ord_path.windows(2) {
        let u = w[0];
        let v = w[1];
        let meta = meta_lookup.get(&(u, v)).copied().unwrap_or(EdgeMeta {
            score: DEFAULT_MISSING_SCORE,
            weight: score_to_weight(DEFAULT_MISSING_SCORE),
        });

        total_weight += meta.weight;
        out_edges.push(PathEdge {
            source_id: nodes[u].clone(),
            target_id: nodes[v].clone(),
            cross_encoder_score: meta.score,
            weight: meta.weight,
        });
    }

    Some(PathResult {
        node_ids: out_nodes,
        edges: out_edges,
        total_weight,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_stronger_multi_hop_when_allowed() {
        let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let edges = vec![
            EdgeInput {
                source_id: "A".to_string(),
                target_id: "B".to_string(),
                cross_encoder_score: Some(0.9),
            },
            EdgeInput {
                source_id: "B".to_string(),
                target_id: "C".to_string(),
                cross_encoder_score: Some(0.9),
            },
            EdgeInput {
                source_id: "A".to_string(),
                target_id: "C".to_string(),
                cross_encoder_score: Some(0.2),
            },
        ];

        let res = find_path_with_max_depth(&nodes, &edges, "A", "C", 2).unwrap();
        assert_eq!(res.node_ids, vec!["A", "B", "C"]);
        assert_eq!(res.edges.len(), 2);
        assert!((res.total_weight - 0.2).abs() < 1e-9);
    }

    #[test]
    fn respects_max_depth_limit() {
        let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let edges = vec![
            EdgeInput {
                source_id: "A".to_string(),
                target_id: "B".to_string(),
                cross_encoder_score: Some(0.9),
            },
            EdgeInput {
                source_id: "B".to_string(),
                target_id: "C".to_string(),
                cross_encoder_score: Some(0.9),
            },
            EdgeInput {
                source_id: "A".to_string(),
                target_id: "C".to_string(),
                cross_encoder_score: Some(0.2),
            },
        ];

        let res = find_path_with_max_depth(&nodes, &edges, "A", "C", 1).unwrap();
        assert_eq!(res.node_ids, vec!["A", "C"]);
        assert_eq!(res.edges.len(), 1);
        assert!((res.total_weight - 0.8).abs() < 1e-9);
    }

    #[test]
    fn returns_none_when_no_path() {
        let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let edges = vec![EdgeInput {
            source_id: "A".to_string(),
            target_id: "B".to_string(),
            cross_encoder_score: Some(0.9),
        }];

        assert_eq!(find_path_with_max_depth(&nodes, &edges, "A", "C", 4), None);
    }
}


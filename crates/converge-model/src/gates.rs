//! Gate graph validation and change impact (g02.026 batch 26.1).
//!
//! The graph used to be written once, by `repo create`, from a literal
//! the code controlled — so nothing ever had to ask whether a graph made
//! sense. Batch 22.4 finding 33 is what that cost: `promote` is one of
//! the six contract verbs and no user could reach it, because no user
//! could make a second gate.
//!
//! The moment a person can supply a graph, every shape they can type
//! needs an answer. Two of them are not cosmetic. `promote` walks
//! upstreams to decide whether a promotion is legal, so a cycle is an
//! unbounded walk inside a request handler; and `publish` resolves an
//! entry gate, so a graph without one refuses every publication with an
//! error about something else.
//!
//! Pure functions on purpose. This is the part most likely to be wrong
//! in an interesting way, and purity is what lets it be tested
//! exhaustively without a server, a database or a repo.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{GateGraph, GateNode};

/// Coalesce strategies the merge engine implements (doc 17 §4).
///
/// Listed here rather than in the server so a graph can be checked
/// before it is sent. A gate naming a strategy nothing implements would
/// otherwise be accepted and then fail at merge time, on data already
/// committed.
pub const STRATEGIES: &[&str] = &["whole-file", "text-line-merge"];

/// Why a graph was refused.
///
/// One variant per reason rather than a string, so callers can render
/// them their own way and tests can name what they expect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphFault {
    Empty,
    DuplicateGate {
        gate_id: String,
    },
    UnknownUpstream {
        gate_id: String,
        upstream: String,
    },
    SelfUpstream {
        gate_id: String,
    },
    /// Gates that reach themselves through upstreams, named in a stable
    /// order so the message does not change between runs.
    Cycle {
        gates: Vec<String>,
    },
    /// Nothing to publish into: every gate has an upstream.
    NoEntryGate,
    UnknownStrategy {
        gate_id: String,
        strategy: String,
    },
    /// A gate that may release but that no publication can ever reach.
    UnreachableRelease {
        gate_id: String,
    },
    EmptyGateId,
}

impl std::fmt::Display for GraphFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "a gate graph needs at least one gate"),
            Self::EmptyGateId => write!(f, "a gate id cannot be empty"),
            Self::DuplicateGate { gate_id } => {
                write!(f, "gate {gate_id} is declared more than once")
            }
            Self::UnknownUpstream { gate_id, upstream } => write!(
                f,
                "gate {gate_id} names upstream {upstream}, which is not a gate in this graph"
            ),
            Self::SelfUpstream { gate_id } => {
                write!(f, "gate {gate_id} lists itself as its own upstream")
            }
            Self::Cycle { gates } => write!(
                f,
                "gates {} form a cycle; promotion walks upstreams and would not terminate",
                gates.join(" -> ")
            ),
            Self::NoEntryGate => write!(
                f,
                "every gate has an upstream, so there is nowhere to publish; \
                 at least one gate must have none"
            ),
            Self::UnknownStrategy { gate_id, strategy } => write!(
                f,
                "gate {gate_id} uses strategy {strategy}, which is not one of: {}",
                STRATEGIES.join(", ")
            ),
            Self::UnreachableRelease { gate_id } => write!(
                f,
                "gate {gate_id} may release but nothing can reach it from an entry gate"
            ),
        }
    }
}

/// Check a graph before anything is allowed to depend on it.
///
/// Returns every fault rather than the first. A person editing a graph
/// by hand wants the whole list; fixing one problem per round trip is
/// the same experience `converge doctor` was built to end.
pub fn validate(graph: &GateGraph) -> Vec<GraphFault> {
    let mut faults = Vec::new();
    if graph.gates.is_empty() {
        return vec![GraphFault::Empty];
    }

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for gate in &graph.gates {
        if gate.gate_id.trim().is_empty() {
            faults.push(GraphFault::EmptyGateId);
            continue;
        }
        if !seen.insert(gate.gate_id.as_str()) {
            faults.push(GraphFault::DuplicateGate {
                gate_id: gate.gate_id.clone(),
            });
        }
        if !STRATEGIES.contains(&gate.strategy.as_str()) {
            faults.push(GraphFault::UnknownStrategy {
                gate_id: gate.gate_id.clone(),
                strategy: gate.strategy.clone(),
            });
        }
    }

    for gate in &graph.gates {
        for upstream in &gate.upstreams {
            if upstream == &gate.gate_id {
                faults.push(GraphFault::SelfUpstream {
                    gate_id: gate.gate_id.clone(),
                });
            } else if !seen.contains(upstream.as_str()) {
                faults.push(GraphFault::UnknownUpstream {
                    gate_id: gate.gate_id.clone(),
                    upstream: upstream.clone(),
                });
            }
        }
    }

    // Cycles are only meaningful once the edges are known to exist;
    // reporting a cycle through a gate that does not exist would be a
    // second confusing answer to the same typo.
    if !faults
        .iter()
        .any(|f| matches!(f, GraphFault::UnknownUpstream { .. }))
        && let Some(cycle) = find_cycle(graph)
    {
        faults.push(GraphFault::Cycle { gates: cycle });
    }

    let entries: Vec<&GateNode> = graph
        .gates
        .iter()
        .filter(|g| g.upstreams.is_empty())
        .collect();
    if entries.is_empty() {
        faults.push(GraphFault::NoEntryGate);
    } else if !faults.iter().any(|f| matches!(f, GraphFault::Cycle { .. })) {
        // A release gate nothing can reach is a graph that looks staged
        // and can never produce a release. Only checked once the graph
        // is known to be acyclic, or the walk below would not terminate.
        let reachable = reachable_from_entries(graph);
        for gate in graph.gates.iter().filter(|g| g.may_release) {
            if !reachable.contains(gate.gate_id.as_str()) {
                faults.push(GraphFault::UnreachableRelease {
                    gate_id: gate.gate_id.clone(),
                });
            }
        }
    }

    faults
}

/// The gates on one cycle, or `None`.
///
/// Depth-first with an explicit colour map: white unvisited, grey on the
/// current path, black finished. A grey edge closes a cycle, and the
/// path from that gate onwards is the cycle to name.
fn find_cycle(graph: &GateGraph) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Grey,
        Black,
    }
    let downstream_of: BTreeMap<&str, Vec<&str>> = graph
        .gates
        .iter()
        .map(|g| {
            (
                g.gate_id.as_str(),
                g.upstreams.iter().map(|u| u.as_str()).collect(),
            )
        })
        .collect();

    let mut marks: BTreeMap<&str, Mark> = BTreeMap::new();
    // Sorted, so the cycle reported for a given graph is always the same
    // one; an error message that varies between runs is a bad bug report.
    let mut roots: Vec<&str> = graph.gates.iter().map(|g| g.gate_id.as_str()).collect();
    roots.sort_unstable();

    for root in roots {
        if marks.contains_key(root) {
            continue;
        }
        let mut path: Vec<&str> = Vec::new();
        if let Some(cycle) = walk(root, &downstream_of, &mut marks, &mut path) {
            return Some(cycle);
        }
    }
    return None;

    fn walk<'a>(
        gate: &'a str,
        edges: &BTreeMap<&'a str, Vec<&'a str>>,
        marks: &mut BTreeMap<&'a str, Mark>,
        path: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        marks.insert(gate, Mark::Grey);
        path.push(gate);
        for next in edges.get(gate).into_iter().flatten() {
            match marks.get(next) {
                Some(Mark::Grey) => {
                    let from = path.iter().position(|g| g == next).unwrap_or(0);
                    let mut cycle: Vec<String> =
                        path[from..].iter().map(|g| g.to_string()).collect();
                    cycle.push(next.to_string());
                    return Some(cycle);
                }
                Some(Mark::Black) => {}
                None => {
                    if let Some(found) = walk(next, edges, marks, path) {
                        return Some(found);
                    }
                }
            }
        }
        path.pop();
        marks.insert(gate, Mark::Black);
        None
    }
}

/// Gates a publication can arrive at, following promotions from an entry
/// gate. Assumes an acyclic graph.
fn reachable_from_entries(graph: &GateGraph) -> BTreeSet<&str> {
    let mut reachable: BTreeSet<&str> = graph
        .gates
        .iter()
        .filter(|g| g.upstreams.is_empty())
        .map(|g| g.gate_id.as_str())
        .collect();
    // Fixed point rather than a walk: a gate becomes reachable once any
    // upstream is, and the graph is small enough that repeating until
    // nothing changes is clearer than ordering it.
    loop {
        let mut grew = false;
        for gate in &graph.gates {
            if reachable.contains(gate.gate_id.as_str()) {
                continue;
            }
            if gate
                .upstreams
                .iter()
                .any(|u| reachable.contains(u.as_str()))
            {
                reachable.insert(gate.gate_id.as_str());
                grew = true;
            }
        }
        if !grew {
            return reachable;
        }
    }
}

/// What lives in a gate, and would therefore be stranded by removing it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateOccupancy {
    pub gate_id: String,
    pub candidates: u64,
    /// Publications above the window floor: the ones a fold still reads.
    pub open_publications: u64,
    pub has_partition_state: bool,
}

impl GateOccupancy {
    pub fn is_empty(&self) -> bool {
        self.candidates == 0 && self.open_publications == 0 && !self.has_partition_state
    }
}

/// What changing from one graph to another would do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphImpact {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// Gates whose upstreams change: `(gate, before, after)`.
    pub reparented: Vec<(String, Vec<String>, Vec<String>)>,
    /// Gates whose settings change but whose place in the graph does not.
    pub retuned: Vec<String>,
    /// Occupancy of every gate this change removes or re-parents.
    pub occupancy: Vec<GateOccupancy>,
}

impl GraphImpact {
    /// Would this change strand work that exists?
    ///
    /// Removing an empty gate is housekeeping. Removing one with candidates
    /// in it makes them unaddressable — the same shape as batch 22.4
    /// finding 34, where a dangling reference wedged a partition with no
    /// way back through the CLI.
    pub fn strands_work(&self) -> bool {
        self.occupancy.iter().any(|o| !o.is_empty())
    }

    pub fn is_noop(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.reparented.is_empty()
            && self.retuned.is_empty()
    }
}

/// Compare two graphs. `occupancy` is supplied by the caller, which is
/// what keeps this pure — the counts come from storage, the judgement
/// does not.
pub fn impact_of(
    before: &GateGraph,
    after: &GateGraph,
    occupancy: &[GateOccupancy],
) -> GraphImpact {
    let old: BTreeMap<&str, &GateNode> = before
        .gates
        .iter()
        .map(|g| (g.gate_id.as_str(), g))
        .collect();
    let new: BTreeMap<&str, &GateNode> = after
        .gates
        .iter()
        .map(|g| (g.gate_id.as_str(), g))
        .collect();

    let mut impact = GraphImpact {
        added: new
            .keys()
            .filter(|id| !old.contains_key(*id))
            .map(|id| id.to_string())
            .collect(),
        removed: old
            .keys()
            .filter(|id| !new.contains_key(*id))
            .map(|id| id.to_string())
            .collect(),
        ..Default::default()
    };

    for (id, old_gate) in &old {
        let Some(new_gate) = new.get(id) else {
            continue;
        };
        // Upstream order is presentation, not meaning: the same parents
        // listed differently is not a re-parenting.
        let before_up: BTreeSet<&String> = old_gate.upstreams.iter().collect();
        let after_up: BTreeSet<&String> = new_gate.upstreams.iter().collect();
        if before_up != after_up {
            impact.reparented.push((
                id.to_string(),
                old_gate.upstreams.clone(),
                new_gate.upstreams.clone(),
            ));
        } else if old_gate.required_approvals != new_gate.required_approvals
            || old_gate.strategy != new_gate.strategy
            || old_gate.may_release != new_gate.may_release
            || old_gate.name != new_gate.name
        {
            impact.retuned.push(id.to_string());
        }
    }

    // Only for gates this change actually disturbs. Reporting occupancy
    // for untouched gates would bury the two lines that matter.
    let disturbed: BTreeSet<&str> = impact
        .removed
        .iter()
        .map(|s| s.as_str())
        .chain(impact.reparented.iter().map(|(id, _, _)| id.as_str()))
        .collect();
    impact.occupancy = occupancy
        .iter()
        .filter(|o| disturbed.contains(o.gate_id.as_str()))
        .cloned()
        .collect();
    impact.occupancy.sort_by(|a, b| a.gate_id.cmp(&b.gate_id));

    impact
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(id: &str, upstreams: &[&str]) -> GateNode {
        GateNode {
            gate_id: id.into(),
            name: id.into(),
            upstreams: upstreams.iter().map(|s| s.to_string()).collect(),
            required_approvals: 0,
            strategy: "whole-file".into(),
            may_release: false,
        }
    }

    fn graph(gates: Vec<GateNode>) -> GateGraph {
        GateGraph { gates }
    }

    #[test]
    fn a_staged_graph_is_legal() {
        let mut release = gate("release", &["review"]);
        release.may_release = true;
        let g = graph(vec![
            gate("intake", &[]),
            gate("review", &["intake"]),
            release,
        ]);
        assert_eq!(validate(&g), vec![]);
    }

    #[test]
    fn the_single_gate_default_is_legal() {
        // What `repo create` has always produced. If this ever fails,
        // every existing repo is retroactively invalid.
        let mut intake = gate("intake", &[]);
        intake.may_release = true;
        assert_eq!(validate(&graph(vec![intake])), vec![]);
    }

    #[test]
    fn an_unknown_upstream_is_named_with_its_gate() {
        let g = graph(vec![gate("intake", &[]), gate("review", &["intak"])]);
        assert_eq!(
            validate(&g),
            vec![GraphFault::UnknownUpstream {
                gate_id: "review".into(),
                upstream: "intak".into(),
            }]
        );
    }

    #[test]
    fn a_cycle_is_refused_because_promotion_walks_upstreams() {
        let g = graph(vec![
            gate("a", &["c"]),
            gate("b", &["a"]),
            gate("c", &["b"]),
        ]);
        let faults = validate(&g);
        let cycle = faults
            .iter()
            .find_map(|f| match f {
                GraphFault::Cycle { gates } => Some(gates.clone()),
                _ => None,
            })
            .expect("cycle not detected");
        assert_eq!(
            cycle.first(),
            cycle.last(),
            "a cycle names its closing gate"
        );
        assert!(cycle.len() >= 4, "three gates and the repeat: {cycle:?}");
    }

    #[test]
    fn a_cycle_report_does_not_vary_between_runs() {
        let g = graph(vec![
            gate("a", &["c"]),
            gate("b", &["a"]),
            gate("c", &["b"]),
        ]);
        let first = validate(&g);
        for _ in 0..20 {
            assert_eq!(validate(&g), first, "cycle reporting is not stable");
        }
    }

    #[test]
    fn self_upstream_is_its_own_message() {
        // Reported as itself rather than as a one-gate cycle, because
        // the fix a person needs to hear is different.
        let g = graph(vec![gate("intake", &["intake"])]);
        assert!(validate(&g).contains(&GraphFault::SelfUpstream {
            gate_id: "intake".into()
        }));
    }

    #[test]
    fn a_graph_with_no_entry_has_nowhere_to_publish() {
        let g = graph(vec![gate("a", &["b"]), gate("b", &["a"])]);
        let faults = validate(&g);
        assert!(faults.contains(&GraphFault::NoEntryGate), "{faults:?}");
    }

    #[test]
    fn an_unknown_strategy_lists_the_known_ones() {
        let mut g = gate("intake", &[]);
        g.strategy = "three-way-magic".into();
        let text = validate(&graph(vec![g]))[0].to_string();
        assert!(text.contains("three-way-magic"), "{text}");
        for known in STRATEGIES {
            assert!(text.contains(known), "known strategy missing: {text}");
        }
    }

    #[test]
    fn a_release_gate_nothing_reaches_is_refused() {
        // Legal as a graph, useless as a workflow: `island` may release
        // and no publication can ever arrive there.
        let mut island = gate("island", &["orphan"]);
        island.may_release = true;
        let g = graph(vec![
            gate("intake", &[]),
            gate("orphan", &["island"]),
            island,
        ]);
        let faults = validate(&g);
        assert!(
            faults.iter().any(|f| matches!(
                f,
                GraphFault::UnreachableRelease { gate_id } if gate_id == "island"
            )) || faults.iter().any(|f| matches!(f, GraphFault::Cycle { .. })),
            "{faults:?}"
        );
    }

    #[test]
    fn every_fault_is_reported_not_just_the_first() {
        // One round trip per problem is the experience `doctor` exists
        // to end; a graph editor should not reintroduce it.
        let mut bad = gate("intake", &[]);
        bad.strategy = "nope".into();
        let g = graph(vec![bad, gate("review", &["missing"])]);
        let faults = validate(&g);
        assert!(faults.len() >= 2, "only reported {faults:?}");
    }

    #[test]
    fn adding_a_gate_strands_nothing() {
        let before = graph(vec![gate("intake", &[])]);
        let after = graph(vec![gate("intake", &[]), gate("review", &["intake"])]);
        let occupancy = vec![GateOccupancy {
            gate_id: "intake".into(),
            candidates: 11,
            open_publications: 12,
            has_partition_state: false,
        }];
        let impact = impact_of(&before, &after, &occupancy);
        assert_eq!(impact.added, vec!["review".to_string()]);
        assert!(impact.removed.is_empty());
        assert!(
            !impact.strands_work(),
            "a busy untouched gate was reported as at risk"
        );
    }

    #[test]
    fn removing_an_occupied_gate_reports_what_it_would_strand() {
        let before = graph(vec![gate("intake", &[]), gate("review", &["intake"])]);
        let after = graph(vec![gate("intake", &[])]);
        let occupancy = vec![GateOccupancy {
            gate_id: "review".into(),
            candidates: 3,
            open_publications: 2,
            has_partition_state: true,
        }];
        let impact = impact_of(&before, &after, &occupancy);
        assert_eq!(impact.removed, vec!["review".to_string()]);
        assert!(impact.strands_work());
        assert_eq!(impact.occupancy[0].candidates, 3);
    }

    #[test]
    fn removing_an_empty_gate_is_housekeeping() {
        let before = graph(vec![gate("intake", &[]), gate("unused", &["intake"])]);
        let after = graph(vec![gate("intake", &[])]);
        let occupancy = vec![GateOccupancy {
            gate_id: "unused".into(),
            ..Default::default()
        }];
        assert!(!impact_of(&before, &after, &occupancy).strands_work());
    }

    #[test]
    fn reordering_upstreams_is_not_a_reparenting() {
        let before = graph(vec![gate("a", &[]), gate("b", &[]), gate("c", &["a", "b"])]);
        let after = graph(vec![gate("a", &[]), gate("b", &[]), gate("c", &["b", "a"])]);
        let impact = impact_of(&before, &after, &[]);
        assert!(impact.reparented.is_empty(), "{impact:?}");
        assert!(impact.is_noop(), "{impact:?}");
    }

    #[test]
    fn retuning_a_gate_is_not_reparenting_it() {
        let before = graph(vec![gate("intake", &[])]);
        let mut tuned = gate("intake", &[]);
        tuned.required_approvals = 2;
        let after = graph(vec![tuned]);
        let impact = impact_of(&before, &after, &[]);
        assert_eq!(impact.retuned, vec!["intake".to_string()]);
        assert!(impact.reparented.is_empty());
        // Settings do not move work between gates, so nothing is stranded
        // even when the gate is full.
        assert!(!impact.strands_work());
    }
}

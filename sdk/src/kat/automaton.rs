use {
    super::ast::{KatExpr, Symbol, TestExpr},
    std::collections::{BTreeMap, BTreeSet, VecDeque},
};

/// Identifier for a state in an ε-NFA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateId(pub usize);

impl StateId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Label carried by a consuming transition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionLabel {
    Action(Symbol),
    Test(TestExpr),
}

/// Transition within an ε-NFA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    Epsilon {
        from: StateId,
        to: StateId,
    },
    Symbol {
        from: StateId,
        to: StateId,
        label: TransitionLabel,
    },
}

/// ε-NFA produced from a KAT expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpsilonNfa {
    pub state_count: usize,
    pub start: StateId,
    pub accepts: BTreeSet<StateId>,
    pub transitions: Vec<Transition>,
}

impl EpsilonNfa {
    pub fn from_kat(expr: &KatExpr) -> Self {
        let mut builder = Builder::default();
        let fragment = builder.build(expr);
        builder.finish(fragment)
    }

    /// Determinise this ε-NFA using the classical subset construction.
    ///
    /// The conversion first saturates each subset of states under ε-closure and
    /// then treats each distinct closure as a vertex in the resulting DFA. The
    /// transition structure emerges by grouping all consuming steps by label and
    /// taking their ε-saturated images. This mirrors the algebraic insight that
    /// recognisers for Kleene algebra terms are determined entirely by the set
    /// of derivative behaviours reachable after consuming a symbol.
    pub fn to_dfa(&self) -> DeterministicFiniteAutomaton {
        let mut epsilon_index = vec![Vec::new(); self.state_count];
        let mut symbol_index = vec![Vec::new(); self.state_count];

        for transition in &self.transitions {
            match transition {
                Transition::Epsilon { from, to } => {
                    epsilon_index[from.index()].push(*to);
                }
                Transition::Symbol { from, to, label } => {
                    symbol_index[from.index()].push((label.clone(), *to));
                }
            }
        }

        let start_subset = self.epsilon_closure(std::iter::once(self.start), &epsilon_index);
        let start_id = DfaStateId(0);

        let mut subset_to_state = BTreeMap::new();
        subset_to_state.insert(start_subset.clone(), start_id);

        let mut states = vec![DfaState {
            transitions: Vec::new(),
        }];
        let mut accepts = BTreeSet::new();
        if start_subset
            .iter()
            .any(|state| self.accepts.contains(state))
        {
            accepts.insert(start_id);
        }

        let mut queue = VecDeque::new();
        queue.push_back(start_subset);

        while let Some(current_subset) = queue.pop_front() {
            let current_id = subset_to_state[&current_subset];

            let mut transitions_by_label: BTreeMap<TransitionLabel, BTreeSet<StateId>> =
                BTreeMap::new();
            for state in &current_subset {
                for (label, target) in &symbol_index[state.index()] {
                    transitions_by_label
                        .entry(label.clone())
                        .or_default()
                        .insert(*target);
                }
            }

            let mut transitions = Vec::new();
            for (label, seeds) in transitions_by_label {
                let dest_subset = self.epsilon_closure(seeds.into_iter(), &epsilon_index);

                let dest_id = if let Some(id) = subset_to_state.get(&dest_subset) {
                    *id
                } else {
                    let new_id = DfaStateId(states.len());
                    subset_to_state.insert(dest_subset.clone(), new_id);
                    states.push(DfaState {
                        transitions: Vec::new(),
                    });
                    if dest_subset.iter().any(|state| self.accepts.contains(state)) {
                        accepts.insert(new_id);
                    }
                    queue.push_back(dest_subset.clone());
                    new_id
                };

                transitions.push(DfaTransition { label, to: dest_id });
            }

            states[current_id.index()].transitions = transitions;
        }

        DeterministicFiniteAutomaton {
            states,
            start: start_id,
            accepts,
        }
    }

    fn epsilon_closure<I>(&self, seeds: I, epsilon_index: &[Vec<StateId>]) -> BTreeSet<StateId>
    where
        I: IntoIterator<Item = StateId>,
    {
        let mut closure = BTreeSet::new();
        let mut stack = Vec::new();

        for seed in seeds {
            if closure.insert(seed) {
                stack.push(seed);
            }
        }

        while let Some(state) = stack.pop() {
            for &next in &epsilon_index[state.index()] {
                if closure.insert(next) {
                    stack.push(next);
                }
            }
        }

        closure
    }
}

#[derive(Default)]
struct Builder {
    next_state: usize,
    transitions: Vec<Transition>,
}

#[derive(Clone, Copy)]
struct Fragment {
    start: StateId,
    accept: StateId,
}

impl Builder {
    fn build(&mut self, expr: &KatExpr) -> Fragment {
        match expr {
            KatExpr::Zero => self.build_zero(),
            KatExpr::One => self.build_one(),
            KatExpr::Action(symbol) => self.build_action(symbol.clone()),
            KatExpr::Test(test) => self.build_test(test.clone()),
            KatExpr::Sequence(lhs, rhs) => self.build_sequence(lhs, rhs),
            KatExpr::Choice(lhs, rhs) => self.build_choice(lhs, rhs),
            KatExpr::Star(expr) => self.build_star(expr),
        }
    }

    fn build_zero(&mut self) -> Fragment {
        let start = self.new_state();
        let accept = self.new_state();
        Fragment { start, accept }
    }

    fn build_one(&mut self) -> Fragment {
        let start = self.new_state();
        let accept = self.new_state();
        self.add_epsilon(start, accept);
        Fragment { start, accept }
    }

    fn build_action(&mut self, symbol: Symbol) -> Fragment {
        let start = self.new_state();
        let accept = self.new_state();
        self.add_symbol(start, accept, TransitionLabel::Action(symbol));
        Fragment { start, accept }
    }

    fn build_test(&mut self, test: TestExpr) -> Fragment {
        let start = self.new_state();
        let accept = self.new_state();
        self.add_symbol(start, accept, TransitionLabel::Test(test));
        Fragment { start, accept }
    }

    fn build_sequence(&mut self, lhs: &KatExpr, rhs: &KatExpr) -> Fragment {
        let left = self.build(lhs);
        let right = self.build(rhs);
        self.add_epsilon(left.accept, right.start);
        Fragment {
            start: left.start,
            accept: right.accept,
        }
    }

    fn build_choice(&mut self, lhs: &KatExpr, rhs: &KatExpr) -> Fragment {
        let left = self.build(lhs);
        let right = self.build(rhs);
        let start = self.new_state();
        let accept = self.new_state();
        self.add_epsilon(start, left.start);
        self.add_epsilon(start, right.start);
        self.add_epsilon(left.accept, accept);
        self.add_epsilon(right.accept, accept);
        Fragment { start, accept }
    }

    fn build_star(&mut self, expr: &KatExpr) -> Fragment {
        let inner = self.build(expr);
        let start = self.new_state();
        let accept = self.new_state();
        self.add_epsilon(start, inner.start);
        self.add_epsilon(start, accept);
        self.add_epsilon(inner.accept, inner.start);
        self.add_epsilon(inner.accept, accept);
        Fragment { start, accept }
    }

    fn finish(self, fragment: Fragment) -> EpsilonNfa {
        let mut accepts = BTreeSet::new();
        accepts.insert(fragment.accept);
        EpsilonNfa {
            state_count: self.next_state,
            start: fragment.start,
            accepts,
            transitions: self.transitions,
        }
    }

    fn new_state(&mut self) -> StateId {
        let id = StateId(self.next_state);
        self.next_state += 1;
        id
    }

    fn add_epsilon(&mut self, from: StateId, to: StateId) {
        self.transitions.push(Transition::Epsilon { from, to });
    }

    fn add_symbol(&mut self, from: StateId, to: StateId, label: TransitionLabel) {
        self.transitions
            .push(Transition::Symbol { from, to, label });
    }
}

/// Identifier for a state in the deterministic automaton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DfaStateId(pub usize);

impl DfaStateId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Consuming transition in the DFA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DfaTransition {
    pub label: TransitionLabel,
    pub to: DfaStateId,
}

/// Deterministic state comprising all outgoing labelled moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DfaState {
    pub transitions: Vec<DfaTransition>,
}

/// DFA recognising the same language as the originating ε-NFA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicFiniteAutomaton {
    pub states: Vec<DfaState>,
    pub start: DfaStateId,
    pub accepts: BTreeSet<DfaStateId>,
}

impl DeterministicFiniteAutomaton {
    pub fn from_kat(expr: &KatExpr) -> Self {
        EpsilonNfa::from_kat(expr).to_dfa()
    }

    pub fn state(&self, id: DfaStateId) -> &DfaState {
        &self.states[id.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(name: &str) -> Symbol {
        Symbol::from(name)
    }

    #[test]
    fn builds_epsilon_for_one() {
        let nfa = EpsilonNfa::from_kat(&KatExpr::One);
        assert_eq!(nfa.state_count, 2);
        assert_eq!(nfa.accepts.len(), 1);
        assert!(nfa
            .transitions
            .iter()
            .any(|t| matches!(t, Transition::Epsilon { from, to } if *from == nfa.start && nfa.accepts.contains(to))));
    }

    #[test]
    fn builds_action_transition() {
        let expr = KatExpr::Action(sym("a"));
        let nfa = EpsilonNfa::from_kat(&expr);
        assert_eq!(nfa.state_count, 2);
        let found = nfa.transitions.iter().cloned().any(|t| {
            matches!(
                t,
                Transition::Symbol {
                    from,
                    to,
                    label: TransitionLabel::Action(symbol)
                } if from == nfa.start && nfa.accepts.contains(&to) && symbol == sym("a")
            )
        });
        assert!(found);
    }

    #[test]
    fn builds_choice_structure() {
        let expr = KatExpr::Choice(
            Box::new(KatExpr::Action(sym("a"))),
            Box::new(KatExpr::Action(sym("b"))),
        );
        let nfa = EpsilonNfa::from_kat(&expr);
        assert_eq!(nfa.accepts.len(), 1);
        let branch_count = nfa
            .transitions
            .iter()
            .filter(|t| matches!(t, Transition::Epsilon { from, .. } if *from == nfa.start))
            .count();
        assert_eq!(branch_count, 2);
    }

    #[test]
    fn builds_star_loops_back() {
        let expr = KatExpr::Star(Box::new(KatExpr::Action(sym("a"))));
        let nfa = EpsilonNfa::from_kat(&expr);
        assert!(nfa.transitions.iter().any(|t| matches!(
            t,
            Transition::Epsilon { from, to }
            if nfa.accepts.contains(to) && !nfa.accepts.contains(from) && *to != *from
        )));
    }

    #[test]
    fn determinises_zero() {
        let dfa = DeterministicFiniteAutomaton::from_kat(&KatExpr::Zero);
        assert_eq!(dfa.states.len(), 1);
        assert!(!dfa.accepts.contains(&dfa.start));
        assert!(dfa.state(dfa.start).transitions.is_empty());
    }

    #[test]
    fn determinises_choice_of_actions() {
        let expr = KatExpr::Choice(
            Box::new(KatExpr::Action(sym("a"))),
            Box::new(KatExpr::Action(sym("b"))),
        );
        let dfa = DeterministicFiniteAutomaton::from_kat(&expr);
        let start_state = dfa.state(dfa.start);
        assert_eq!(start_state.transitions.len(), 2);
        let mut labels: Vec<Symbol> = start_state
            .transitions
            .iter()
            .map(|t| match &t.label {
                TransitionLabel::Action(symbol) => symbol.clone(),
                _ => panic!("expected action label"),
            })
            .collect();
        labels.sort();
        assert_eq!(labels, vec![sym("a"), sym("b")]);
        for transition in &start_state.transitions {
            assert!(dfa.accepts.contains(&transition.to));
        }
    }

    #[test]
    fn determinises_test_then_action_sequence() {
        let expr = KatExpr::Sequence(
            Box::new(KatExpr::Test(TestExpr::Atom(sym("p")))),
            Box::new(KatExpr::Action(sym("a"))),
        );
        let dfa = DeterministicFiniteAutomaton::from_kat(&expr);
        let start_state = dfa.state(dfa.start);
        assert_eq!(start_state.transitions.len(), 1);
        let first = &start_state.transitions[0];
        match &first.label {
            TransitionLabel::Test(test) => {
                assert_eq!(test, &TestExpr::Atom(sym("p")));
            }
            _ => panic!("expected test label"),
        }
        let second_state = dfa.state(first.to);
        assert_eq!(second_state.transitions.len(), 1);
        match &second_state.transitions[0].label {
            TransitionLabel::Action(symbol) => {
                assert_eq!(symbol, &sym("a"));
            }
            _ => panic!("expected action label"),
        }
        assert!(dfa.accepts.contains(&second_state.transitions[0].to));
    }
}

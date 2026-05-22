pub struct StateMachine<S> {
    states: Set<S>,
    transitions: Vec<Transition<S>>,
    current_state: S,
    history: Vec<S>,
}
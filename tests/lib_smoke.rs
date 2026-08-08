//! 公共 API smoke：保证 re-export 与模块路径都存在。

use aura::{AgentState, StateMachine};

#[test]
fn lib_exposes_state_machine() {
    let sm = StateMachine::new();
    assert_eq!(sm.state(), AgentState::Ready);
}

//! 状态机、预算、停止原因集成测试。

use aura::{AgentError, AgentState, Budget, StateMachine, StopReason, TransitionError};

#[test]
fn state_machine_starts_ready() {
    let sm = StateMachine::new();
    assert_eq!(sm.state(), AgentState::Ready);
    assert!(sm.last_rejection().is_none());
}

#[test]
fn state_machine_default_equals_new() {
    let sm: StateMachine = StateMachine::default();
    assert_eq!(sm.state(), AgentState::Ready);
}

#[test]
fn legal_transitions_round_trip() {
    let mut sm = StateMachine::new();
    sm.transition(AgentState::Planning).unwrap();
    sm.transition(AgentState::ExecutingTool).unwrap();
    sm.transition(AgentState::Planning).unwrap();
    sm.transition(AgentState::WaitingForUser).unwrap();
    sm.transition(AgentState::Planning).unwrap();
    sm.transition(AgentState::Verifying).unwrap();
    sm.transition(AgentState::Completed).unwrap();
    assert_eq!(sm.state(), AgentState::Completed);
    assert!(sm.last_rejection().is_none());
}

#[test]
fn planning_can_fail_directly() {
    let mut sm = StateMachine::new();
    sm.transition(AgentState::Planning).unwrap();
    sm.transition(AgentState::Failed).unwrap();
    assert_eq!(sm.state(), AgentState::Failed);
}

#[test]
fn executing_tool_can_fail() {
    let mut sm = StateMachine::new();
    sm.transition(AgentState::Planning).unwrap();
    sm.transition(AgentState::ExecutingTool).unwrap();
    sm.transition(AgentState::Failed).unwrap();
    assert_eq!(sm.state(), AgentState::Failed);
}

#[test]
fn waiting_for_user_can_fail() {
    let mut sm = StateMachine::new();
    sm.transition(AgentState::Planning).unwrap();
    sm.transition(AgentState::WaitingForUser).unwrap();
    sm.transition(AgentState::Failed).unwrap();
    assert_eq!(sm.state(), AgentState::Failed);
}

#[test]
fn terminal_state_rejects_further_transitions() {
    let mut sm = StateMachine::new();
    sm.transition(AgentState::Planning).unwrap();
    sm.transition(AgentState::Verifying).unwrap();
    sm.transition(AgentState::Completed).unwrap();
    let err = sm.transition(AgentState::Planning).unwrap_err();
    assert_eq!(err.reason, "current state is terminal");
    assert_eq!(sm.last_rejection(), Some("current state is terminal"));

    // Failed terminal 同样拒绝。
    let mut sm2 = StateMachine::new();
    sm2.transition(AgentState::Planning).unwrap();
    sm2.transition(AgentState::Failed).unwrap();
    assert!(sm2.transition(AgentState::Ready).is_err());
}

#[test]
fn self_transition_rejected() {
    let mut sm = StateMachine::new();
    let err = sm.transition(AgentState::Ready).unwrap_err();
    assert_eq!(err.reason, "target equals current state");
    assert_eq!(sm.last_rejection(), Some("target equals current state"));
}

#[test]
fn illegal_transition_rejected() {
    let mut sm = StateMachine::new();
    let err = sm.transition(AgentState::Completed).unwrap_err();
    assert_eq!(err.reason, "transition not allowed");
    assert_eq!(sm.last_rejection(), Some("transition not allowed"));
}

#[test]
fn more_illegal_transitions_for_coverage() {
    let cases = [
        (AgentState::ExecutingTool, AgentState::Verifying),
        (AgentState::ExecutingTool, AgentState::WaitingForUser),
        (AgentState::ExecutingTool, AgentState::Completed),
        (AgentState::Verifying, AgentState::Planning),
        (AgentState::Verifying, AgentState::ExecutingTool),
        (AgentState::Ready, AgentState::Failed),
    ];
    for (from, to) in cases {
        // 把状态机直接重置为 from（绕过合法性，仅用于构造非法转移测试）。
        let mut sm = StateMachine::new();
        // 用一个合法转移把状态先推进到 from，如果可达。
        if from == AgentState::ExecutingTool {
            sm.transition(AgentState::Planning).unwrap();
            sm.transition(AgentState::ExecutingTool).unwrap();
        } else if from == AgentState::Verifying {
            sm.transition(AgentState::Planning).unwrap();
            sm.transition(AgentState::Verifying).unwrap();
        }
        if sm.state() == from {
            assert!(
                sm.transition(to).is_err(),
                "{from:?} -> {to:?} should be illegal"
            );
        }
    }
}

#[test]
fn transition_error_converts_to_agent_error() {
    let err = TransitionError {
        from: AgentState::Ready,
        to: AgentState::Completed,
        reason: "x",
    };
    let agent: AgentError = err.into();
    assert!(matches!(agent, AgentError::InvalidTransition(_)));
}

#[test]
fn agent_state_is_terminal() {
    assert!(AgentState::Completed.is_terminal());
    assert!(AgentState::Failed.is_terminal());
    assert!(!AgentState::Ready.is_terminal());
    assert!(!AgentState::Planning.is_terminal());
    assert!(!AgentState::ExecutingTool.is_terminal());
    assert!(!AgentState::WaitingForUser.is_terminal());
    assert!(!AgentState::Verifying.is_terminal());
}

#[test]
fn budget_new_rejects_zero() {
    assert!(Budget::new(0, 100).is_err());
    assert!(Budget::new(1, 0).is_err());
    let b = Budget::new(3, 100).unwrap();
    assert_eq!(b.max_turns, 3);
    assert_eq!(b.max_context_bytes, 100);
}

#[test]
fn budget_check_turns() {
    let b = Budget::new(3, 100).unwrap();
    b.check_turns(0).unwrap();
    b.check_turns(2).unwrap();
    assert!(b.check_turns(3).is_err());
    assert!(b.check_turns(10).is_err());
}

#[test]
fn budget_check_context_boundary() {
    let b = Budget::new(3, 100).unwrap();
    b.check_context(0).unwrap();
    b.check_context(100).unwrap();
    assert!(b.check_context(101).is_err());
}

#[test]
fn stop_reason_serde_round_trip() {
    let cases = vec![
        StopReason::Completed {
            summary: "ok".into(),
        },
        StopReason::ModelFailed {
            reason: "no".into(),
        },
        StopReason::TurnBudgetReached { used: 12 },
        StopReason::ContextBudgetReached { used: 9999 },
        StopReason::VerificationFailed {
            message: "fail".into(),
        },
        StopReason::UserAborted,
    ];
    let raw = serde_json::to_string(&serde_json::json!(cases)).unwrap();
    let back: Vec<StopReason> = serde_json::from_str(&raw).unwrap();
    assert_eq!(cases, back);
}

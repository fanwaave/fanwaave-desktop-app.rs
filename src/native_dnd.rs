use ores_dnd_core::{
    DndDropPolicy, DndDropResult, DndEnvelope, DndItemKind, DndOperation, DndSessionSnapshot,
};
use ores_dnd_native::NativeDndBridge;

/// Fanwaave's toolkit-neutral desktop boundary around the shared native bridge.
///
/// GPUI/Slint/Qt/winit integrations can translate OS callbacks into these
/// methods while retaining ownership of file promises, temp files, focus and
/// visual drag state. Definitive data is always re-admitted at drop time.
pub struct FanwaaveNativeDnd {
    bridge: NativeDndBridge,
    policy: DndDropPolicy,
}

impl FanwaaveNativeDnd {
    pub fn new(target_id: impl Into<String>) -> Self {
        Self {
            bridge: NativeDndBridge::new(),
            policy: DndDropPolicy::new(
                target_id,
                &[DndOperation::Copy, DndOperation::Move],
                &[
                    DndItemKind::Text,
                    DndItemKind::Uri,
                    DndItemKind::Json,
                    DndItemKind::Bytes,
                ],
            ),
        }
    }

    pub fn with_policy(policy: DndDropPolicy) -> Self {
        Self {
            bridge: NativeDndBridge::new(),
            policy,
        }
    }

    pub fn external_enter(
        &mut self,
        provisional: DndEnvelope,
        preferred: Option<DndOperation>,
    ) -> DndSessionSnapshot {
        self.bridge
            .external_enter(provisional, self.policy.clone(), preferred)
    }

    pub fn external_drop(
        &mut self,
        definitive: DndEnvelope,
        preferred: Option<DndOperation>,
    ) -> Option<DndDropResult> {
        self.bridge
            .external_drop(definitive, self.policy.clone(), preferred)
    }

    pub fn leave(&mut self) -> DndSessionSnapshot {
        self.bridge.leave(self.policy.target_id.clone())
    }

    pub fn cancel(&mut self) -> Option<DndDropResult> {
        self.bridge.cancel()
    }

    pub fn snapshot(&self) -> &DndSessionSnapshot {
        self.bridge.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ores_dnd_core::{DndItem, DndSessionState, ORES_DND_PROTOCOL};

    fn envelope(drag_id: &str, data: &str) -> DndEnvelope {
        DndEnvelope {
            protocol: ORES_DND_PROTOCOL.to_owned(),
            drag_id: drag_id.to_owned(),
            source_runtime: "fanwaave-native".to_owned(),
            allowed_operations: vec![DndOperation::Copy, DndOperation::Move],
            items: vec![DndItem {
                kind: DndItemKind::Text,
                media_type: "text/plain".to_owned(),
                data: data.to_owned(),
                name: None,
            }],
            traceparent: None,
            form_id: None,
        }
    }

    #[test]
    fn external_hover_then_definitive_drop_reuses_canonical_session() {
        let mut dnd = FanwaaveNativeDnd::new("timeline");
        let hover = dnd.external_enter(envelope("drag-1", ""), None);
        assert_eq!(hover.state, DndSessionState::OverTarget);

        let result = dnd
            .external_drop(envelope("drag-1", "hello"), Some(DndOperation::Copy))
            .expect("terminal result");
        assert!(result.accepted);
        assert_eq!(result.operation, Some(DndOperation::Copy));
        assert_eq!(result.target_id.as_deref(), Some("timeline"));
    }

    #[test]
    fn definitive_payload_is_rechecked_after_provisional_acceptance() {
        let policy = DndDropPolicy::new("timeline", &[DndOperation::Copy], &[DndItemKind::Text])
            .with_max_total_bytes(2);
        let mut dnd = FanwaaveNativeDnd::with_policy(policy);
        assert_eq!(
            dnd.external_enter(envelope("drag-2", ""), None).state,
            DndSessionState::OverTarget
        );

        let result = dnd
            .external_drop(envelope("drag-2", "too-large"), None)
            .expect("rejected terminal result");
        assert!(!result.accepted);
        assert_eq!(dnd.snapshot().state, DndSessionState::Cancelled);
    }

    #[test]
    fn leave_invalidates_provisional_target_before_drop() {
        let mut dnd = FanwaaveNativeDnd::new("timeline");
        dnd.external_enter(envelope("drag-3", ""), None);
        assert_eq!(dnd.leave().state, DndSessionState::Dragging);
        assert_eq!(dnd.cancel().expect("cancel result").accepted, false);
    }
}

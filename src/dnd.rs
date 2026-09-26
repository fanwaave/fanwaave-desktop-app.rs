#![allow(clippy::needless_return)]

use ores_dnd_core::{
    commit_accepted_drop,
    reactive::{
        guarded_next, local_event_subject, reactive_state_for, DndLifecycleGuard,
        DndLocalEventSubject, DndReactiveEvent, DndReactiveState,
    },
    DndDropResult, DndEnvelope, DndError, DndLifecyclePhase, DndOperation, DropCommitPorts,
};

/// Native Fanwaave desktop ownership boundary for the shared ORES DnD state
/// machine. This controller intentionally performs no persistence or telemetry
/// side effect until an accepted drop is committed through the shared ORES
/// forms -> opto-sync -> telemetry port sequence.
pub struct DesktopDndController {
    subject: DndLocalEventSubject,
    guard: DndLifecycleGuard,
}

impl Default for DesktopDndController {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopDndController {
    pub fn new() -> Self {
        Self {
            subject: local_event_subject(),
            guard: DndLifecycleGuard::default(),
        }
    }

    fn emit(&mut self, event: DndReactiveEvent) -> Result<DndReactiveState, DndError> {
        let state = reactive_state_for(&event);
        guarded_next(&mut self.subject, &mut self.guard, event)?;
        Ok(state)
    }

    pub fn start(&mut self, envelope: DndEnvelope) -> Result<DndReactiveState, DndError> {
        self.emit(DndReactiveEvent::new(
            DndLifecyclePhase::DragStart,
            envelope,
            None,
            None,
        )?)
    }

    pub fn enter_external(
        &mut self,
        envelope: DndEnvelope,
        target_id: impl Into<String>,
    ) -> Result<DndReactiveState, DndError> {
        self.emit(DndReactiveEvent::new(
            DndLifecyclePhase::DragEnter,
            envelope,
            None,
            Some(target_id.into()),
        )?)
    }

    pub fn hover(
        &mut self,
        envelope: DndEnvelope,
        target_id: impl Into<String>,
    ) -> Result<DndReactiveState, DndError> {
        self.emit(DndReactiveEvent::new(
            DndLifecyclePhase::DragOver,
            envelope,
            None,
            Some(target_id.into()),
        )?)
    }

    pub fn drop_on(
        &mut self,
        envelope: DndEnvelope,
        operation: DndOperation,
        target_id: impl Into<String>,
    ) -> Result<DndReactiveState, DndError> {
        self.emit(DndReactiveEvent::new(
            DndLifecyclePhase::Drop,
            envelope,
            Some(operation),
            Some(target_id.into()),
        )?)
    }

    pub fn end(
        &mut self,
        envelope: DndEnvelope,
        operation: Option<DndOperation>,
        target_id: Option<String>,
    ) -> Result<DndReactiveState, DndError> {
        self.emit(DndReactiveEvent::new(
            DndLifecyclePhase::DragEnd,
            envelope,
            operation,
            target_id,
        )?)
    }

    /// Commit an already-admitted drop through the shared ORES integration
    /// ports. The shared core owns validation and the forms -> opto-sync ->
    /// ores-otel ordering so native desktop adapters cannot silently diverge.
    pub fn commit_accepted(
        envelope: &DndEnvelope,
        result: &DndDropResult,
        ports: DropCommitPorts<'_>,
    ) -> Result<(), DndError> {
        return commit_accepted_drop(envelope, result, ports);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ores_dnd_core::{
        DndItem, DndItemKind, DndTelemetryEvent, OptoSyncPort, OresFormsPort, OresOtelPort,
        ORES_DND_PROTOCOL,
    };
    use std::cell::RefCell;

    fn envelope(drag_id: &str) -> DndEnvelope {
        DndEnvelope {
            protocol: ORES_DND_PROTOCOL.to_owned(),
            drag_id: drag_id.to_owned(),
            source_runtime: "fanwaave-desktop".to_owned(),
            allowed_operations: vec![DndOperation::Copy, DndOperation::Move],
            items: vec![DndItem {
                kind: DndItemKind::Text,
                media_type: "text/plain".to_owned(),
                data: "must-remain-process-local".to_owned(),
                name: None,
            }],
            traceparent: None,
            form_id: None,
        }
    }

    struct RecordingForms<'a> {
        calls: &'a RefCell<Vec<&'static str>>,
    }

    impl OresFormsPort for RecordingForms<'_> {
        fn apply_accepted_drop(
            &self,
            _envelope: &DndEnvelope,
            _result: &DndDropResult,
        ) -> Result<(), DndError> {
            self.calls.borrow_mut().push("forms");
            return Ok(());
        }
    }

    struct RecordingSync<'a> {
        calls: &'a RefCell<Vec<&'static str>>,
    }

    impl OptoSyncPort for RecordingSync<'_> {
        fn persist_accepted_drop(
            &self,
            _envelope: &DndEnvelope,
            _result: &DndDropResult,
        ) -> Result<(), DndError> {
            self.calls.borrow_mut().push("opto-sync");
            return Ok(());
        }
    }

    struct RecordingOtel<'a> {
        calls: &'a RefCell<Vec<&'static str>>,
    }

    impl OresOtelPort for RecordingOtel<'_> {
        fn emit_dnd_event(&self, _event: &DndTelemetryEvent) -> Result<(), DndError> {
            self.calls.borrow_mut().push("ores-otel");
            return Ok(());
        }
    }

    #[test]
    fn desktop_controller_accepts_lossless_lifecycle() -> Result<(), DndError> {
        let mut controller = DesktopDndController::new();
        assert!(controller.start(envelope("drag-1"))?.active);
        let hovered = controller.hover(envelope("drag-1"), "timeline")?;
        assert_eq!(hovered.target_id.as_deref(), Some("timeline"));
        let dropped = controller.drop_on(envelope("drag-1"), DndOperation::Copy, "timeline")?;
        assert_eq!(dropped.operation, Some(DndOperation::Copy));
        let ended = controller.end(
            envelope("drag-1"),
            Some(DndOperation::Copy),
            Some("timeline".to_owned()),
        )?;
        assert!(!ended.active);
        Ok(())
    }

    #[test]
    fn desktop_controller_rejects_impossible_and_disallowed_drop() -> Result<(), DndError> {
        let mut drop_first = DesktopDndController::new();
        assert!(drop_first
            .drop_on(envelope("drag-1"), DndOperation::Copy, "timeline")
            .is_err());

        let mut disallowed = DesktopDndController::new();
        disallowed.start(envelope("drag-2"))?;
        assert!(disallowed
            .drop_on(envelope("drag-2"), DndOperation::Link, "timeline")
            .is_err());
        Ok(())
    }

    #[test]
    fn accepted_drop_uses_shared_ores_port_order() -> Result<(), DndError> {
        let calls = RefCell::new(Vec::new());
        let forms = RecordingForms { calls: &calls };
        let opto_sync = RecordingSync { calls: &calls };
        let otel = RecordingOtel { calls: &calls };
        let envelope = envelope("drag-3");
        let result = DndDropResult {
            drag_id: "drag-3".to_owned(),
            accepted: true,
            operation: Some(DndOperation::Copy),
            target_id: Some("timeline".to_owned()),
            error_code: None,
        };

        DesktopDndController::commit_accepted(
            &envelope,
            &result,
            DropCommitPorts {
                otel: Some(&otel),
                opto_sync: Some(&opto_sync),
                forms: Some(&forms),
            },
        )?;

        assert_eq!(
            calls.borrow().as_slice(),
            ["forms", "opto-sync", "ores-otel"]
        );
        return Ok(());
    }
}

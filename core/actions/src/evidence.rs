use ipc::Evidence;

pub fn action_evidence(summary: impl Into<String>, artifact: impl Into<String>) -> Evidence {
    Evidence {
        summary: summary.into(),
        artifacts: vec![artifact.into()],
    }
}

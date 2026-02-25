use crate::traits::StubActionBackend;

pub fn backend() -> StubActionBackend {
    StubActionBackend::new("linux")
}

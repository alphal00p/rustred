//! A wasm32 Python object is only eight-byte aligned. Keep over-aligned native
//! solver payloads behind Rust-owned pointers at the Python allocation boundary.

macro_rules! check_classes {
    ($($class:ty),+ $(,)?) => {
        $(const _: () = assert!(
            std::mem::align_of::<$class>() <= 8,
            concat!(stringify!($class), " exceeds CPython wasm32 object alignment"),
        );)+
    };
}

check_classes!(
    crate::PyIbpFamily,
    crate::PyIbpRule,
    crate::PyIbpSolution,
    crate::certificate::PyIbpCertificate,
);

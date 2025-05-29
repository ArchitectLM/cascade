// Re-export cucumber macros properly
// Instead of re-exporting the macros directly, we'll create public wrapper macros
// This avoids the name conflicts and visibility issues

#[macro_export]
macro_rules! step_given {
    ($($t:tt)*) => {
        cucumber::given!($($t)*)
    };
}

#[macro_export]
macro_rules! step_when {
    ($($t:tt)*) => {
        cucumber::when!($($t)*)
    };
}

#[macro_export]
macro_rules! step_then {
    ($($t:tt)*) => {
        cucumber::then!($($t)*)
    };
} 
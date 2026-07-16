//! Feature based logging for rt-thread
//! More strict than log/release_max_level_info

#[doc(hidden)]
#[macro_export]
macro_rules! __rt_log_hidden {
    ($feature:literal, $log_macro:path, $($arg:tt)+) => {
        #[cfg(feature = $feature)]
        {
            $log_macro!($($arg)+);
        }
    };
}

#[macro_export]
macro_rules! rt_debug {
    ($($arg:tt)+) => {
        $crate::__rt_log_hidden!("rt-debug", ::log::debug, $($arg)+);
    };
}

#[macro_export]
macro_rules! rt_info {
    ($($arg:tt)+) => {
        $crate::__rt_log_hidden!("rt-info", ::log::info, $($arg)+);
    };
}

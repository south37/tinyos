#[derive(PartialEq, PartialOrd, Copy, Clone)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl LogLevel {
    pub const fn from_str(s: &str) -> LogLevel {
        match s.as_bytes() {
            b"error" | b"ERROR" => LogLevel::Error,
            b"warn" | b"WARN" => LogLevel::Warn,
            b"info" | b"INFO" => LogLevel::Info,
            b"debug" | b"DEBUG" => LogLevel::Debug,
            b"trace" | b"TRACE" => LogLevel::Trace,
            _ => LogLevel::Info, // Default
        }
    }
}

// Default to Info if not set
pub const CURRENT_LOG_LEVEL: LogLevel = {
    if let Some(level) = option_env!("LOG_LEVEL") {
        LogLevel::from_str(level)
    } else {
        LogLevel::Info
    }
};

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ({
        if $crate::log::CURRENT_LOG_LEVEL >= $crate::log::LogLevel::Error {
            $crate::uart_println!("\x1b[31m[ERROR]\x1b[0m {}", format_args!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => ({
        if $crate::log::CURRENT_LOG_LEVEL >= $crate::log::LogLevel::Warn {
            $crate::uart_println!("\x1b[33m[WARN]\x1b[0m {}", format_args!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ({
        if $crate::log::CURRENT_LOG_LEVEL >= $crate::log::LogLevel::Info {
            $crate::uart_println!("\x1b[34m[INFO]\x1b[0m {}", format_args!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => ({
        if $crate::log::CURRENT_LOG_LEVEL >= $crate::log::LogLevel::Debug {
            $crate::uart_println!("\x1b[32m[DEBUG]\x1b[0m {}", format_args!($($arg)*));
        }
    });
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => ({
        if $crate::log::CURRENT_LOG_LEVEL >= $crate::log::LogLevel::Trace {
            $crate::uart_println!("\x1b[90m[TRACE]\x1b[0m {}", format_args!($($arg)*));
        }
    });
}

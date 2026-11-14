/// Reads parameter `index` of a parameter slice as the given variant
/// (`String`, `Usize`, …), or returns an `InvalidParameters` error naming the
/// parameter (`$name`), the expected kind and the kind it has.
/// For `set_params` implementations.
#[macro_export]
macro_rules! parse_args {
    ($params:expr, $idx:expr, String, $name:expr) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$params[$idx] {
            TextForgeParamTypes::String(payload) => payload.clone(),
            other => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        format!(
                            "parameter {} (`{}`) must be text, but it is {}",
                            $idx + 1,
                            $name,
                            other.kind_name()
                        )
                        .into(),
                    ),
                    "",
                    "",
                ));
            }
        }
    }};
    ($params:expr, $idx:expr, Usize, $name:expr) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$params[$idx] {
            TextForgeParamTypes::Usize(payload) => payload.clone(),
            other => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        format!(
                            "parameter {} (`{}`) must be a non-negative integer, but it is {}",
                            $idx + 1,
                            $name,
                            other.kind_name()
                        )
                        .into(),
                    ),
                    "",
                    "",
                ));
            }
        }
    }};
    ($params:expr, $idx:expr, Token, $name:expr) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$params[$idx] {
            TextForgeParamTypes::Token(payload) => payload.clone(),
            other => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        format!(
                            "parameter {} (`{}`) must be an instruction, but it is {}",
                            $idx + 1,
                            $name,
                            other.kind_name()
                        )
                        .into(),
                    ),
                    "",
                    "",
                ));
            }
        }
    }};
    ($param:expr, String) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$param {
            TextForgeParamTypes::String(payload) => payload.clone(),
            other => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        format!("a parameter must be text, but it is {}", other.kind_name()).into(),
                    ),
                    "",
                    "",
                ));
            }
        }
    }};
    ($param:expr, Usize) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match $param {
            TextForgeParamTypes::Usize(payload) => payload.clone(),
            other => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        format!(
                            "a parameter must be a non-negative integer, but it is {}",
                            other.kind_name()
                        )
                        .into(),
                    ),
                    "",
                    "",
                ));
            }
        }
    }};
    ($param:expr, Token) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match $param {
            TextForgeParamTypes::Token(payload) => payload.clone(),
            other => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        format!(
                            "a parameter must be an instruction, but it is {}",
                            other.kind_name()
                        )
                        .into(),
                    ),
                    "",
                    "",
                ));
            }
        }
    }};
}

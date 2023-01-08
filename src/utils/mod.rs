pub(crate) mod quantity;

pub(crate) fn escape_json_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

pub(crate) fn convert_filter(filter: log::LevelFilter) -> tracing_subscriber::filter::LevelFilter {
    match filter {
        log::LevelFilter::Off => tracing_subscriber::filter::LevelFilter::OFF,
        log::LevelFilter::Error => tracing_subscriber::filter::LevelFilter::ERROR,
        log::LevelFilter::Warn => tracing_subscriber::filter::LevelFilter::WARN,
        log::LevelFilter::Info => tracing_subscriber::filter::LevelFilter::INFO,
        log::LevelFilter::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
        log::LevelFilter::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_pointer_escaping_slash() {
        let value = "networking.k8s.io/egress-bandwidth";

        assert_eq!(
            escape_json_pointer(value),
            "networking.k8s.io~1egress-bandwidth".to_owned()
        );
    }

    #[test]
    fn test_json_pointer_escaping_tilde() {
        let value = "networking.k8s.io/egress~bandwidth";

        assert_eq!(
            escape_json_pointer(value),
            "networking.k8s.io~1egress~0bandwidth".to_owned()
        );
    }

    #[test]
    fn test_filter_conversion_info() {
        assert_eq!(
            convert_filter(log::LevelFilter::Info),
            tracing_subscriber::filter::LevelFilter::INFO
        );
    }

    #[test]
    fn test_filter_conversion_debug() {
        assert_eq!(
            convert_filter(log::LevelFilter::Debug),
            tracing_subscriber::filter::LevelFilter::DEBUG
        );
    }
}

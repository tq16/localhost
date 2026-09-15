#[derive(Debug, Clone)]
pub struct Route {
    pub path: String,
    pub root: String,
    pub index: String,
    pub allowed_methods: Vec<String>,
    pub redirect: Option<String>,
    pub directory_listing: bool,
    pub cgi: Option<(String, String)>,
}

impl Route {
    pub fn new(path: &str, root: &str) -> Self {
        Self {
            path: path.to_string(),
            root: root.to_string(),
            index: "index.html".to_string(),
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "DELETE".to_string(),
            ],
            redirect: None,
            directory_listing: false,
            cgi: None,
        }
    }
}

pub fn match_route<'a>(routes: &'a [Route], request_path: &str) -> Option<&'a Route> {
    routes
        .iter()
        .filter(|route| path_matches(&route.path, request_path))
        .max_by_key(|route| route.path.len())
}

fn path_matches(route_path: &str, request_path: &str) -> bool {
    if route_path == "/" {
        return request_path.starts_with('/');
    }

    request_path == route_path
        || request_path
            .strip_prefix(route_path)
            .is_some_and(|remaining| remaining.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_route_matches_any_path() {
        let routes = vec![Route::new("/", "www")];

        let route = match_route(&routes, "/about.html").unwrap();

        assert_eq!(route.path, "/");
    }

    #[test]
    fn most_specific_route_wins() {
        let routes = vec![
            Route::new("/", "www"),
            Route::new("/images", "www/images"),
        ];

        let route = match_route(&routes, "/images/logo.png").unwrap();

        assert_eq!(route.path, "/images");
        assert_eq!(route.root, "www/images");
    }

    #[test]
    fn route_does_not_match_part_of_a_word() {
        let routes = vec![Route::new("/app", "www/app")];

        assert!(match_route(&routes, "/apple.html").is_none());
    }
}
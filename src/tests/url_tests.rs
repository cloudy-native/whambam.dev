use url::Url;

#[test]
fn test_valid_urls() {
    // HTTP URLs
    assert!(Url::parse("http://example.com").is_ok());
    assert!(Url::parse("http://example.com/path").is_ok());
    assert!(Url::parse("http://example.com:8080").is_ok());
    assert!(Url::parse("http://example.com/path?query=value").is_ok());
    assert!(Url::parse("http://user:pass@example.com").is_ok());
    
    // HTTPS URLs
    assert!(Url::parse("https://example.com").is_ok());
    assert!(Url::parse("https://example.com/path").is_ok());
    assert!(Url::parse("https://example.com:8443").is_ok());
    assert!(Url::parse("https://example.com/path?query=value").is_ok());
    assert!(Url::parse("https://user:pass@example.com").is_ok());
    
    // Localhost
    assert!(Url::parse("http://localhost").is_ok());
    assert!(Url::parse("http://localhost:8080").is_ok());
    assert!(Url::parse("http://127.0.0.1").is_ok());
    assert!(Url::parse("http://127.0.0.1:8080").is_ok());
}

#[test]
fn test_invalid_urls() {
    // Missing scheme
    assert!(Url::parse("example.com").is_err());
    assert!(Url::parse("localhost").is_err());
    
    // Invalid schemes
    assert!(Url::parse("ftp://example.com").is_err() || Url::parse("ftp://example.com").unwrap().scheme() != "http");
    assert!(Url::parse("file:///etc/hosts").is_err() || Url::parse("file:///etc/hosts").unwrap().scheme() != "http");
    
    // Empty host
    assert!(Url::parse("http://").is_err());
    
    // Invalid characters
    assert!(Url::parse("http://example com").is_err());
}

#[test]
fn test_url_components() {
    let url = Url::parse("https://user:pass@example.com:8443/path?query=value#fragment").unwrap();
    
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.username(), "user");
    assert_eq!(url.password(), Some("pass"));
    assert_eq!(url.host_str(), Some("example.com"));
    assert_eq!(url.port(), Some(8443));
    assert_eq!(url.path(), "/path");
    assert_eq!(url.query(), Some("query=value"));
    assert_eq!(url.fragment(), Some("fragment"));
}
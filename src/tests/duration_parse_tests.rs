use crate::{Args, parse_duration};
use clap::Parser;

#[test]
fn test_duration_parsing_in_args() {
    // Test valid durations through Args
    let test_cases = vec![
        // Raw seconds
        ("0", 0),
        ("42", 42),
        ("3600", 3600),
        
        // With 's' suffix
        ("10s", 10),
        ("60s", 60),
        ("3600s", 3600),
        
        // With 'm' suffix
        ("1m", 60),
        ("5m", 300),
        ("60m", 3600),
        
        // With 'h' suffix
        ("1h", 3600),
        ("2h", 7200),
        ("24h", 86400),
    ];
    
    for (input, expected) in test_cases {
        let args = Args::parse_from([
            "test", 
            "http://example.com",
            "-z", input
        ]);
        
        assert_eq!(args.duration_str, input);
        let parsed_duration = parse_duration(&args.duration_str).unwrap();
        assert_eq!(parsed_duration, expected, "Failed for input: {}", input);
    }
}

#[test]
fn test_duration_edge_cases() {
    // Test very large durations
    let large_duration = "1000000";  // Million seconds (about 11.5 days)
    let args = Args::parse_from([
        "test",
        "http://example.com",
        "-z", large_duration
    ]);
    
    assert_eq!(args.duration_str, large_duration);
    let parsed_duration = parse_duration(&args.duration_str).unwrap();
    assert_eq!(parsed_duration, 1000000);
    
    // Test zero duration (valid, means no time limit)
    let zero_duration = "0";
    let args = Args::parse_from([
        "test",
        "http://example.com",
        "-z", zero_duration
    ]);
    
    assert_eq!(args.duration_str, zero_duration);
    let parsed_duration = parse_duration(&args.duration_str).unwrap();
    assert_eq!(parsed_duration, 0);
}

#[test]
fn test_duration_combined_with_requests() {
    // Test that duration and requests work together
    let args = Args::parse_from([
        "test",
        "http://example.com",
        "-n", "1000",
        "-z", "60s"
    ]);
    
    assert_eq!(args.requests, 1000);
    assert_eq!(args.duration_str, "60s");
    let parsed_duration = parse_duration(&args.duration_str).unwrap();
    assert_eq!(parsed_duration, 60);
    
    // Duration takes precedence over requests at runtime (handled in main.rs),
    // but both values are parsed correctly
}
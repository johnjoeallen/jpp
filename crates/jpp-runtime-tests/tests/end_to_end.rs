use jpp_generator::generate_java;
use jpp_parser::parse_source;
use jpp_runtime_tests::fixture_customer;

#[test]
fn customer_fixture_generates_plain_java() {
    let parsed = parse_source(None, fixture_customer()).unwrap();
    let java = generate_java(&parsed).unwrap();

    assert!(java.contains("public class Customer"));
    assert!(java.contains("private UUID id;"));
    assert!(java.contains("public synchronized Customer id(UUID value)"));
    assert!(java.contains("private final Instant created;"));
    assert!(java.contains("public Customer firstName(String value)"));
    assert!(java.contains("public Customer email(String value)"));
    assert!(java.contains(
        "return java.util.Optional.ofNullable(referrer).map(__jpp_value -> __jpp_value.fullName()).orElse(\"\");"
    ));
    assert!(java.contains("public CustomerSummary summary()"));
    assert!(java.contains("target.displayName(fullName());"));
    assert!(java.contains(
        "target.referrerName(java.util.Optional.ofNullable(referrer).map(__jpp_value -> __jpp_value.fullName()).orElse(\"\"));"
    ));
    assert!(java.contains("public String fullName()"));
    assert!(!java.contains("prop "));
    assert!(!java.contains("mapper "));
}

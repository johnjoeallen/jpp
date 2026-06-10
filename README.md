# JPP

JPP is the Java PreProcessor: a small tooling engine behind the J++ concept.

The goal is not to build a new language. JPP source files are ordinary Java
with a small set of pragmatic extension islands. JPP generates normal `.java`
files that work with `javac`, Maven, Gradle, IntelliJ, and the rest of the Java
tooling ecosystem.

## Principles

- All valid Java remains valid.
- Generated output is ordinary Java.
- No alternate type system.
- No alternate runtime.
- One `.jpp` file generates one `.java`.
- Fluent APIs by default.
- Prefer generation over magic.
- Reduce ceremony, not flexibility.

## Workspace

```text
crates/
├── jpp-cli
├── jpp-parser
├── jpp-model
├── jpp-generator
├── jpp-diagnostics
└── jpp-runtime-tests
```

## Usage

```bash
cargo run -p jpp-cli -- generate src/main/jpp target/generated-sources/jpp
```

The binary also accepts the short form:

```bash
jpp src/main/jpp target/generated-sources/jpp
```

Commands:

```bash
jpp generate <input-dir> <output-dir>
jpp validate <input-dir>
jpp clean <output-dir>
```

## First Milestone: Properties

Input:

```java
public class Customer {

    prop get set String firstName;

    prop get set String lastName;

    prop get set Customer referrer;

    prop get String referrerName {
        return referrer?.fullName() ?: "";
    }

    mapper CustomerSummary summary {
        displayName = fullName();
        referrerName = referrer?.fullName() ?: "";
    }

    prop get String fullName {
        return firstName()
            + " "
            + lastName();
    }
}
```

Generated output uses fluent accessors:

```java
public String firstName()
public Customer firstName(String value)
```

Supported property forms:

- Mutable: `prop get set String firstName;`
- Read-only calculated: `prop get String fullName { return "..."; }`
- Write-only: `prop set String password;`
- Once-only: `prop get once UUID id;`
- Final constructor property: `prop get final Instant created;`
- Setter transformation block:

```java
prop get set String email {
    set {
        value = value.trim().toLowerCase();
    }
}
```

## Mapper Preview

The first mapper milestone generates instance mapping methods that create a
target object and populate it through fluent setters:

```java
mapper CustomerSummary summary {
    displayName = fullName();
    referrerName = referrer?.fullName() ?: "";
}
```

Generated Java:

```java
public CustomerSummary summary() {
    CustomerSummary target = new CustomerSummary();

    target.displayName(fullName());
    target.referrerName(
        java.util.Optional.ofNullable(referrer)
            .map(__jpp_value -> __jpp_value.fullName())
            .orElse("")
    );

    return target;
}
```

JPP also supports a null-safe access operator for simple receiver expressions.
Use `?:` to provide an explicit fallback when the receiver is `null`:

```java
return referrer?.fullName() ?: "";
```

Generated Java:

```java
return java.util.Optional.ofNullable(referrer)
    .map(__jpp_value -> __jpp_value.fullName())
    .orElse("");
```

## Example

The repository includes a customer sample with both source and generated output:

- JPP source: `examples/customer/src/main/jpp/demo/Customer.jpp`
- Generated Java: `examples/customer/generated/demo/Customer.java`

Generate it locally with:

```bash
cargo run -p jpp-cli -- generate examples/customer/src/main/jpp examples/customer/generated
```

The generated file is checked in intentionally so changes to JPP syntax and
generation behavior are easy to review.

## Tests

```bash
cargo test
```

# JPP

JPP is a Maven plugin for generating Java sources from `.jpp` files.

The goal is not to build a new language. JPP source files are ordinary Java
with a small set of pragmatic extension islands. JPP generates normal `.java`
files that work with `javac`, Maven, IntelliJ, and the rest of the Java
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

## Repository Layout

```text
pom.xml
maven-plugin/
scripts/
examples/customer/
```

## Maven Usage

Add the plugin to the build and let it run during `generate-sources`:

```xml
<plugin>
  <groupId>io.github.jpp</groupId>
  <artifactId>jpp-maven-plugin</artifactId>
  <version>0.1.0</version>
  <configuration>
    <inputDirectory>${project.basedir}/src/main/jpp</inputDirectory>
    <outputDirectory>${project.build.directory}/generated-sources/jpp</outputDirectory>
  </configuration>
</plugin>
```

The plugin writes generated Java into the output directory and adds that
directory to the project source roots.

Default behavior writes to `target/generated-sources/jpp`. To generate files
next to the `.jpp` sources instead, set:

```xml
<configuration>
  <inPlace>true</inPlace>
</configuration>
```

Build and install the plugin locally first:

```bash
bash scripts/build-and-install.sh
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

The repository includes a customer sample Maven project at
`examples/customer/`. Its Java is generated from
`examples/customer/src/main/jpp/demo/Customer.jpp` during the Maven build.

Build it locally with:

```bash
mvn -f examples/pom.xml compile
```

# JPP Architecture

JPP is a Maven-integrated source generator for Java. It does not define a new
runtime, type system, compiler, or language toolchain.

The implementation is a single Maven plugin:

1. `jpp-maven-plugin` scans Java source for JPP constructs such as `prop`.
2. Normal Java text is preserved as raw segments.
3. Extension islands are parsed into an internal AST.
4. The plugin emits ordinary Java source into the configured output directory.
5. Maven adds the generated directory to the project source roots during
   `generate-sources`.

The plugin is self-contained and does not depend on a separately installed JPP
binary.

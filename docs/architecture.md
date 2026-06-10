# JPP Architecture

JPP is a source preprocessor for Java. It does not define a new runtime, type
system, compiler, or build toolchain.

The first implementation uses extension islands:

1. `jpp-parser` scans Java source for JPP constructs such as `prop`.
2. Normal Java text is preserved as raw segments.
3. Extension islands are parsed into model nodes from `jpp-model`.
4. `jpp-generator` replaces those model nodes with ordinary Java source.
5. `jpp-cli` writes one `.java` file for each `.jpp` input file.

This keeps the blast radius small and leaves room for future syntax without
requiring a complete Java parser.

## Crates

- `jpp-cli`: command line entrypoint.
- `jpp-parser`: extension island scanner and property parser.
- `jpp-model`: shared AST and semantic model structs.
- `jpp-generator`: plain Java source generation.
- `jpp-diagnostics`: diagnostic types with file, line, column, code, and suggestions.
- `jpp-runtime-tests`: end-to-end fixtures for generated Java behavior.

use jpp_diagnostics::{Diagnostic, JppResult, SourceLocation};
use jpp_model::{JppFile, Mapper, Property, Segment};

pub fn generate_java(file: &JppFile) -> JppResult<String> {
    let class_name = file.class_name.as_deref().ok_or_else(|| {
        Diagnostic::new(
            "JPP2000",
            "Could not find a Java class declaration for generated methods.",
            SourceLocation::new(file.path.clone(), 1, 1),
        )
    })?;

    let final_properties = file
        .segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Property(property) if property.final_field => Some(property),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut emitted_constructor = false;
    let mut output = String::new();

    for segment in &file.segments {
        match segment {
            Segment::Java(java) => output.push_str(&rewrite_null_safe_access(java)),
            Segment::Property(property) => {
                output.push_str(&generate_property(
                    class_name,
                    property,
                    &final_properties,
                    &mut emitted_constructor,
                ));
            }
            Segment::Mapper(mapper) => output.push_str(&generate_mapper(mapper)),
        }
    }

    Ok(output)
}

fn generate_mapper(mapper: &Mapper) -> String {
    let indent = &mapper.indent;
    let mut output = String::new();

    output.push_str(&format!(
        "{indent}public {} {}() {{\n",
        mapper.target_type, mapper.method_name
    ));
    output.push_str(&format!(
        "{indent}    {} target = new {}();\n\n",
        mapper.target_type, mapper.target_type
    ));

    for assignment in &mapper.assignments {
        output.push_str(&format!(
            "{indent}    target.{}({});\n",
            assignment.target_property,
            rewrite_null_safe_access(&assignment.expression)
        ));
    }

    output.push_str(&format!("\n{indent}    return target;\n"));
    output.push_str(&format!("{indent}}}\n"));
    output
}

fn generate_property(
    class_name: &str,
    property: &Property,
    final_properties: &[&Property],
    emitted_constructor: &mut bool,
) -> String {
    let mut output = String::new();
    let indent = &property.indent;

    if property.has_backing_field() {
        if property.final_field {
            output.push_str(&format!(
                "{indent}private final {} {};\n",
                property.ty, property.name
            ));
        } else if !property.is_calculated() {
            output.push_str(&format!(
                "{indent}private {} {};\n",
                property.ty, property.name
            ));
        }
    }

    if property.final_field && !*emitted_constructor {
        output.push('\n');
        output.push_str(&generate_constructor(class_name, final_properties, indent));
        *emitted_constructor = true;
    }

    if property.getter {
        output.push('\n');
        output.push_str(&generate_getter(property, indent));
    }

    if property.setter || property.once {
        output.push('\n');
        output.push_str(&generate_setter(class_name, property, indent));
    }

    output
}

fn generate_constructor(class_name: &str, properties: &[&Property], indent: &str) -> String {
    let mut output = String::new();

    output.push_str(&format!("{indent}public {class_name}(\n"));
    for (index, property) in properties.iter().enumerate() {
        let comma = if index + 1 == properties.len() {
            ""
        } else {
            ","
        };
        output.push_str(&format!(
            "{indent}    {} {}{comma}\n",
            property.ty, property.name
        ));
    }
    output.push_str(&format!("{indent}) {{\n"));
    for property in properties {
        output.push_str(&format!("{indent}    this.{0} = {0};\n", property.name));
    }
    output.push_str(&format!("{indent}}}\n"));

    output
}

fn generate_getter(property: &Property, indent: &str) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "{indent}public {} {}() {{\n",
        property.ty, property.name
    ));

    if let Some(body) = &property.getter_body {
        output.push_str(&indent_body(
            &rewrite_null_safe_access(body),
            indent,
            "    ",
        ));
    } else {
        output.push_str(&format!("{indent}    return this.{};\n", property.name));
    }

    output.push_str(&format!("{indent}}}\n"));
    output
}

fn generate_setter(class_name: &str, property: &Property, indent: &str) -> String {
    let mut output = String::new();
    let synchronized = if property.once { " synchronized" } else { "" };

    output.push_str(&format!(
        "{indent}public{synchronized} {class_name} {}({} value) {{\n",
        property.name, property.ty
    ));

    if property.once {
        output.push_str(&generate_null_check(indent, "value", &property.name));
        output.push('\n');
    }

    if property.once {
        output.push_str(&format!(
            "{indent}    if (this.{0} != null) {{\n{indent}        throw new IllegalStateException(\n{indent}            \"Property '{0}' is once-only\"\n{indent}        );\n{indent}    }}\n\n",
            property.name
        ));
    }

    if let Some(body) = &property.setter_body {
        output.push_str(&indent_body(
            &rewrite_null_safe_access(body),
            indent,
            "    ",
        ));
        output.push('\n');
    }

    output.push_str(&format!("{indent}    this.{} = value;\n\n", property.name));
    output.push_str(&format!("{indent}    return this;\n"));
    output.push_str(&format!("{indent}}}\n"));
    output
}

fn generate_null_check(indent: &str, variable: &str, property_name: &str) -> String {
    format!(
        "{indent}    if ({variable} == null) {{\n{indent}        throw new NullPointerException(\n{indent}            \"Property '{property_name}' cannot be null\"\n{indent}        );\n{indent}    }}\n"
    )
}

fn rewrite_null_safe_access(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut index = 0;

    while index < source.len() {
        if let Some(relative) = find_null_safe_operator(source, index) {
            let operator = relative;
            if let Some(rewrite) = parse_null_safe_access(source, operator) {
                output.push_str(&source[index..rewrite.receiver_start]);
                output.push_str(&generate_null_safe_access(&rewrite));
                index = rewrite.end;
            } else {
                output.push_str(&source[index..operator + 1]);
                index = operator + 1;
            }
        } else {
            output.push_str(&source[index..]);
            break;
        }
    }

    output
}

fn generate_null_safe_access(rewrite: &NullSafeRewrite<'_>) -> String {
    if let Some(fallback) = rewrite.fallback {
        return format!(
            "java.util.Optional.ofNullable({}).map(__jpp_value -> __jpp_value.{}).orElse({})",
            rewrite.receiver, rewrite.member, fallback
        );
    }

    format!(
        "({0} == null ? null : {0}.{1})",
        rewrite.receiver, rewrite.member
    )
}

struct NullSafeRewrite<'a> {
    receiver_start: usize,
    receiver: &'a str,
    member: &'a str,
    fallback: Option<&'a str>,
    end: usize,
}

fn find_null_safe_operator(source: &str, from: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = from;
    let mut state = ScanState::Code;

    while index + 1 < source.len() {
        match state {
            ScanState::Code => match bytes[index] {
                b'"' => state = ScanState::String,
                b'\'' => state = ScanState::Char,
                b'/' if bytes[index + 1] == b'/' => {
                    state = ScanState::LineComment;
                    index += 1;
                }
                b'/' if bytes[index + 1] == b'*' => {
                    state = ScanState::BlockComment;
                    index += 1;
                }
                b'?' if bytes[index + 1] == b'.' => return Some(index),
                _ => {}
            },
            ScanState::String => {
                if bytes[index] == b'\\' {
                    index += 1;
                } else if bytes[index] == b'"' {
                    state = ScanState::Code;
                }
            }
            ScanState::Char => {
                if bytes[index] == b'\\' {
                    index += 1;
                } else if bytes[index] == b'\'' {
                    state = ScanState::Code;
                }
            }
            ScanState::LineComment => {
                if bytes[index] == b'\n' {
                    state = ScanState::Code;
                }
            }
            ScanState::BlockComment => {
                if bytes[index] == b'*' && bytes[index + 1] == b'/' {
                    state = ScanState::Code;
                    index += 1;
                }
            }
        }

        index += 1;
    }

    None
}

#[derive(Clone, Copy)]
enum ScanState {
    Code,
    String,
    Char,
    LineComment,
    BlockComment,
}

fn parse_null_safe_access(source: &str, operator: usize) -> Option<NullSafeRewrite<'_>> {
    let receiver_end = skip_whitespace_back(source, operator);
    let receiver_start = find_receiver_start(source, receiver_end)?;
    let receiver = &source[receiver_start..receiver_end];
    let member_start = operator + 2;
    let member_end = find_member_end(source, member_start)?;
    let member = &source[member_start..member_end];
    let (fallback, end) = parse_null_safe_fallback(source, member_end);

    Some(NullSafeRewrite {
        receiver_start,
        receiver,
        member,
        fallback,
        end,
    })
}

fn parse_null_safe_fallback(source: &str, member_end: usize) -> (Option<&str>, usize) {
    let fallback_operator = skip_whitespace_forward(source, member_end);
    let Some(rest) = source.get(fallback_operator..) else {
        return (None, member_end);
    };

    if !rest.starts_with("?:") {
        return (None, member_end);
    }

    let fallback_start = skip_whitespace_forward(source, fallback_operator + 2);
    let fallback_end = find_fallback_end(source, fallback_start);
    let fallback_trimmed_end = trim_whitespace_back(source, fallback_start, fallback_end);

    (
        Some(&source[fallback_start..fallback_trimmed_end]),
        fallback_end,
    )
}

fn skip_whitespace_forward(source: &str, mut index: usize) -> usize {
    while index < source.len() && source.as_bytes()[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn skip_whitespace_back(source: &str, mut index: usize) -> usize {
    while index > 0 && source.as_bytes()[index - 1].is_ascii_whitespace() {
        index -= 1;
    }
    index
}

fn trim_whitespace_back(source: &str, start: usize, mut end: usize) -> usize {
    while end > start && source.as_bytes()[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}

fn find_fallback_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut index = start;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut state = ScanState::Code;

    while index < source.len() {
        match state {
            ScanState::Code => match bytes[index] {
                b';' | b'\n' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                    return index;
                }
                b'(' => paren_depth += 1,
                b')' if paren_depth > 0 => paren_depth -= 1,
                b'[' => bracket_depth += 1,
                b']' if bracket_depth > 0 => bracket_depth -= 1,
                b'{' => brace_depth += 1,
                b'}' if brace_depth > 0 => brace_depth -= 1,
                b'"' => state = ScanState::String,
                b'\'' => state = ScanState::Char,
                _ => {}
            },
            ScanState::String => {
                if bytes[index] == b'\\' {
                    index += 1;
                } else if bytes[index] == b'"' {
                    state = ScanState::Code;
                }
            }
            ScanState::Char => {
                if bytes[index] == b'\\' {
                    index += 1;
                } else if bytes[index] == b'\'' {
                    state = ScanState::Code;
                }
            }
            ScanState::LineComment | ScanState::BlockComment => {}
        }

        index += 1;
    }

    index
}

fn find_receiver_start(source: &str, end: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = end;

    while index > 0 {
        let byte = bytes[index - 1];
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.' {
            index -= 1;
        } else {
            break;
        }
    }

    if index == end {
        None
    } else {
        Some(index)
    }
}

fn find_member_end(source: &str, start: usize) -> Option<usize> {
    let mut index = read_identifier(source, start)?;

    loop {
        index = match source.as_bytes().get(index) {
            Some(b'(') => read_balanced(source, index, b'(', b')')?,
            Some(b'[') => read_balanced(source, index, b'[', b']')?,
            Some(b'.') => read_identifier(source, index + 1)?,
            _ => return Some(index),
        };
    }
}

fn read_identifier(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let first = *bytes.get(start)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }

    let mut index = start + 1;
    while let Some(byte) = bytes.get(index) {
        if byte.is_ascii_alphanumeric() || *byte == b'_' {
            index += 1;
        } else {
            break;
        }
    }

    Some(index)
}

fn read_balanced(source: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = start;
    let mut depth = 0usize;
    let mut state = ScanState::Code;

    while index < source.len() {
        match state {
            ScanState::Code => match bytes[index] {
                byte if byte == open => depth += 1,
                byte if byte == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                b'"' => state = ScanState::String,
                b'\'' => state = ScanState::Char,
                _ => {}
            },
            ScanState::String => {
                if bytes[index] == b'\\' {
                    index += 1;
                } else if bytes[index] == b'"' {
                    state = ScanState::Code;
                }
            }
            ScanState::Char => {
                if bytes[index] == b'\\' {
                    index += 1;
                } else if bytes[index] == b'\'' {
                    state = ScanState::Code;
                }
            }
            ScanState::LineComment | ScanState::BlockComment => {}
        }

        index += 1;
    }

    None
}

fn indent_body(body: &str, base_indent: &str, extra_indent: &str) -> String {
    let mut output = String::new();
    let common_indent = common_indent(body);

    for line in body.lines() {
        if line.trim().is_empty() {
            output.push('\n');
        } else {
            output.push_str(base_indent);
            output.push_str(extra_indent);
            output.push_str(line.get(common_indent..).unwrap_or(line).trim_end());
            output.push('\n');
        }
    }

    output
}

fn common_indent(body: &str) -> usize {
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .map(char::len_utf8)
                .sum()
        })
        .min()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use jpp_parser::parse_source;

    use super::*;

    #[test]
    fn generates_customer_properties() {
        let source = r#"public class Customer {
    prop get once UUID id;

    prop get final Instant created;

    prop get set String firstName {
        set {
            value = value.trim();
        }
    }

    prop get String fullName {
        return firstName() + " " + lastName();
    }
}
"#;

        let parsed = parse_source(None, source).unwrap();
        let java = generate_java(&parsed).unwrap();

        assert!(java.contains("private UUID id;"));
        assert!(java.contains("public synchronized Customer id(UUID value)"));
        assert!(java.contains("private final Instant created;"));
        assert!(java.contains("public Customer(\n        Instant created\n    )"));
        assert!(java.contains("public Customer firstName(String value)"));
        assert!(java.contains("if (value == null)"));
        assert!(java.contains("value = value.trim();"));
        assert!(java.contains("public String fullName()"));
        assert!(!java.contains("prop get"));
    }

    #[test]
    fn rewrites_null_safe_access() {
        let source = r#"public class Customer {
    private Customer referrer;

    prop get String referrerName {
        return referrer?.fullName() ?: "";
    }
}
"#;

        let parsed = parse_source(None, source).unwrap();
        let java = generate_java(&parsed).unwrap();

        assert!(java.contains(
            "return java.util.Optional.ofNullable(referrer).map(__jpp_value -> __jpp_value.fullName()).orElse(\"\");"
        ));
    }

    #[test]
    fn ignores_null_safe_text_in_strings_and_comments() {
        let java = rewrite_null_safe_access(
            "String s = \"x?.hello()\"; // y?.hello()\nreturn customer?.email();",
        );

        assert!(java.contains("\"x?.hello()\""));
        assert!(java.contains("// y?.hello()"));
        assert!(java.contains("(customer == null ? null : customer.email())"));
    }

    #[test]
    fn rewrites_null_safe_access_with_fallback() {
        let java = rewrite_null_safe_access("return customer?.email() ?: \"unknown\";");

        assert_eq!(
            java,
            "return java.util.Optional.ofNullable(customer).map(__jpp_value -> __jpp_value.email()).orElse(\"unknown\");"
        );
    }

    #[test]
    fn generates_mapper_method() {
        let source = r#"public class Customer {
    mapper CustomerSummary summary {
        displayName = fullName();
        referrerName = referrer?.fullName() ?: "";
    }
}
"#;

        let parsed = parse_source(None, source).unwrap();
        let java = generate_java(&parsed).unwrap();

        assert!(java.contains("public CustomerSummary summary()"));
        assert!(java.contains("CustomerSummary target = new CustomerSummary();"));
        assert!(java.contains("target.displayName(fullName());"));
        assert!(java.contains(
            "target.referrerName(java.util.Optional.ofNullable(referrer).map(__jpp_value -> __jpp_value.fullName()).orElse(\"\"));"
        ));
        assert!(java.contains("return target;"));
    }
}

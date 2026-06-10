use std::path::Path;

use jpp_diagnostics::{Diagnostic, JppResult, SourceLocation};
use jpp_model::{JppFile, Mapper, MapperAssignment, Property, Segment};

pub fn parse_source(path: Option<&Path>, source: &str) -> JppResult<JppFile> {
    let mut parser = Parser {
        path,
        source,
        cursor: 0,
        segments: Vec::new(),
    };

    parser.parse()
}

struct Parser<'a> {
    path: Option<&'a Path>,
    source: &'a str,
    cursor: usize,
    segments: Vec<Segment>,
}

impl Parser<'_> {
    fn parse(&mut self) -> JppResult<JppFile> {
        while let Some(island_start) = find_extension_island(self.source, self.cursor) {
            if island_start.start > self.cursor {
                self.segments.push(Segment::Java(
                    self.source[self.cursor..island_start.start].to_string(),
                ));
            }

            match island_start.kind {
                IslandKind::Property => {
                    let island = self.parse_property_at(island_start.start)?;
                    self.cursor = island.end;
                    self.segments.push(Segment::Property(island.property));
                }
                IslandKind::Mapper => {
                    let island = self.parse_mapper_at(island_start.start)?;
                    self.cursor = island.end;
                    self.segments.push(Segment::Mapper(island.mapper));
                }
            }
        }

        if self.cursor < self.source.len() {
            self.segments
                .push(Segment::Java(self.source[self.cursor..].to_string()));
        }

        Ok(JppFile {
            path: self.path.map(Path::to_path_buf),
            source: self.source.to_string(),
            class_name: find_class_name(self.source),
            segments: std::mem::take(&mut self.segments),
        })
    }

    fn parse_property_at(&self, start: usize) -> JppResult<PropertyIsland> {
        let after_indent = start
            + self.source[start..]
                .find(|ch| ch != ' ' && ch != '\t')
                .unwrap_or(0);
        let indent = self.source[start..after_indent].to_string();
        let location = SourceLocation::at_offset(self.path, self.source, after_indent);
        let header_start = after_indent + "prop".len();
        let header_end = find_header_end(self.source, header_start).ok_or_else(|| {
            Diagnostic::new(
                "JPP1001",
                "Unterminated property declaration.",
                location.clone(),
            )
            .with_suggestion("End the property with ';' or add a balanced '{ ... }' body.")
        })?;

        let header = self.source[header_start..header_end].trim();
        let mut parsed = parse_property_header(header, location.clone())?;
        parsed.indent = indent;

        let marker = self.source.as_bytes()[header_end] as char;
        let end = if marker == ';' {
            header_end + 1
        } else {
            let body_end = find_matching_brace(self.source, header_end).ok_or_else(|| {
                Diagnostic::new(
                    "JPP1002",
                    format!("Property '{}' has an unclosed body.", parsed.name),
                    location.clone(),
                )
            })?;
            let body = &self.source[header_end + 1..body_end];
            apply_property_body(&mut parsed, body, location)?;
            body_end + 1
        };

        Ok(PropertyIsland {
            property: parsed,
            end,
        })
    }

    fn parse_mapper_at(&self, start: usize) -> JppResult<MapperIsland> {
        let after_indent = start
            + self.source[start..]
                .find(|ch| ch != ' ' && ch != '\t')
                .unwrap_or(0);
        let indent = self.source[start..after_indent].to_string();
        let location = SourceLocation::at_offset(self.path, self.source, after_indent);
        let header_start = after_indent + "mapper".len();
        let header_end = find_header_end(self.source, header_start).ok_or_else(|| {
            Diagnostic::new(
                "JPP1101",
                "Unterminated mapper declaration.",
                location.clone(),
            )
            .with_suggestion("Use a form such as 'mapper CustomerDto toDto { name = name(); }'.")
        })?;

        if self.source.as_bytes()[header_end] as char != '{' {
            return Err(Diagnostic::new(
                "JPP1102",
                "Mapper declarations require a body.",
                location,
            ));
        }

        let header = self.source[header_start..header_end].trim();
        let mut mapper = parse_mapper_header(header, location.clone())?;
        mapper.indent = indent;

        let body_end = find_matching_brace(self.source, header_end).ok_or_else(|| {
            Diagnostic::new(
                "JPP1103",
                format!("Mapper '{}' has an unclosed body.", mapper.method_name),
                location.clone(),
            )
        })?;
        let body = &self.source[header_end + 1..body_end];
        mapper.assignments = parse_mapper_body(body, location)?;

        Ok(MapperIsland {
            mapper,
            end: body_end + 1,
        })
    }
}

struct PropertyIsland {
    property: Property,
    end: usize,
}

struct MapperIsland {
    mapper: Mapper,
    end: usize,
}

struct ExtensionIsland {
    start: usize,
    kind: IslandKind,
}

enum IslandKind {
    Property,
    Mapper,
}

fn find_extension_island(source: &str, from: usize) -> Option<ExtensionIsland> {
    let mut offset = from;

    for line in source[from..].split_inclusive('\n') {
        let line_start = offset;
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.starts_with("prop ") || trimmed.starts_with("prop\t") {
            return Some(ExtensionIsland {
                start: line_start,
                kind: IslandKind::Property,
            });
        }
        if trimmed.starts_with("mapper ") || trimmed.starts_with("mapper\t") {
            return Some(ExtensionIsland {
                start: line_start,
                kind: IslandKind::Mapper,
            });
        }
        offset += line.len();
    }

    None
}

fn parse_mapper_header(header: &str, location: SourceLocation) -> JppResult<Mapper> {
    let tokens: Vec<&str> = header.split_whitespace().collect();
    if tokens.len() != 2 {
        return Err(Diagnostic::new(
            "JPP1100",
            "Mapper declaration must include a target type and method name.",
            location,
        )
        .with_suggestion("Use a form such as 'mapper CustomerDto toDto { name = name(); }'."));
    }

    Ok(Mapper {
        target_type: tokens[0].to_string(),
        method_name: tokens[1].to_string(),
        assignments: Vec::new(),
        location,
        indent: String::new(),
    })
}

fn parse_mapper_body(body: &str, location: SourceLocation) -> JppResult<Vec<MapperAssignment>> {
    let mut assignments = Vec::new();

    for statement in split_mapper_statements(body) {
        let trimmed = statement.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((target, expression)) = trimmed.split_once('=') else {
            return Err(Diagnostic::new(
                "JPP1104",
                format!("Mapper assignment '{trimmed}' must use '='."),
                location,
            )
            .with_suggestion("Use a form such as 'displayName = fullName();'."));
        };

        let target = target.trim();
        let expression = expression.trim();
        if target.is_empty() || expression.is_empty() {
            return Err(Diagnostic::new(
                "JPP1105",
                "Mapper assignment must include a target property and expression.",
                location,
            ));
        }

        assignments.push(MapperAssignment {
            target_property: target.to_string(),
            expression: expression.to_string(),
        });
    }

    if assignments.is_empty() {
        return Err(Diagnostic::new(
            "JPP1106",
            "Mapper body must contain at least one assignment.",
            location,
        ));
    }

    Ok(assignments)
}

fn split_mapper_statements(body: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let bytes = body.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;

    while index < body.len() {
        let byte = bytes[index];

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match byte {
            b'\\' if in_string || in_char => escaped = true,
            b'"' if !in_char => in_string = !in_string,
            b'\'' if !in_string => in_char = !in_char,
            b'(' | b'[' | b'{' if !in_string && !in_char => depth += 1,
            b')' | b']' | b'}' if !in_string && !in_char && depth > 0 => depth -= 1,
            b';' if !in_string && !in_char && depth == 0 => {
                statements.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }

        index += 1;
    }

    if start < body.len() {
        statements.push(&body[start..]);
    }

    statements
}

fn find_header_end(source: &str, from: usize) -> Option<usize> {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;

    for (index, ch) in source[from..].char_indices() {
        let absolute = from + index;

        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string || in_char => escaped = true,
            '"' if !in_char => in_string = !in_string,
            '\'' if !in_string => in_char = !in_char,
            ';' | '{' if !in_string && !in_char => return Some(absolute),
            _ => {}
        }
    }

    None
}

fn find_matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;

    for (index, ch) in source[open..].char_indices() {
        let absolute = open + index;

        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string || in_char => escaped = true,
            '"' if !in_char => in_string = !in_string,
            '\'' if !in_string => in_char = !in_char,
            '{' if !in_string && !in_char => depth += 1,
            '}' if !in_string && !in_char => {
                depth -= 1;
                if depth == 0 {
                    return Some(absolute);
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_property_header(header: &str, location: SourceLocation) -> JppResult<Property> {
    let tokens: Vec<&str> = header.split_whitespace().collect();
    if tokens.len() < 3 {
        return Err(
            Diagnostic::new("JPP1000", "Property declaration is incomplete.", location)
                .with_suggestion("Use a form such as 'prop get set String firstName;'."),
        );
    }

    let mut getter = false;
    let mut setter = false;
    let mut once = false;
    let mut final_field = false;
    let mut index = 0;

    while index < tokens.len() {
        match tokens[index] {
            "get" => getter = true,
            "set" => setter = true,
            "once" => once = true,
            "final" => final_field = true,
            _ => break,
        }
        index += 1;
    }

    if !getter && !setter {
        return Err(Diagnostic::new(
            "JPP1004",
            "Property must declare at least 'get' or 'set'.",
            location,
        ));
    }

    if once && !getter {
        return Err(Diagnostic::new(
            "JPP1005",
            "Once-only properties must be readable.",
            location,
        ));
    }

    if final_field && setter {
        return Err(Diagnostic::new(
            "JPP1006",
            "Final properties cannot declare a setter.",
            location,
        ));
    }

    if final_field && once {
        return Err(Diagnostic::new(
            "JPP1007",
            "A property cannot be both 'final' and 'once'.",
            location,
        ));
    }

    if tokens.len() - index < 2 {
        return Err(Diagnostic::new(
            "JPP1000",
            "Property declaration must include a type and name.",
            location,
        ));
    }

    let name = tokens[tokens.len() - 1].to_string();
    let ty = tokens[index..tokens.len() - 1].join(" ");

    Ok(Property {
        name,
        ty,
        getter,
        setter,
        once,
        final_field,
        getter_body: None,
        setter_body: None,
        location,
        indent: String::new(),
    })
}

fn apply_property_body(
    property: &mut Property,
    body: &str,
    location: SourceLocation,
) -> JppResult<()> {
    let trimmed = body.trim();

    if property.setter {
        if let Some(set_start) = find_set_block(trimmed) {
            let open = set_start + trimmed[set_start..].find('{').unwrap();
            let close = find_matching_brace(trimmed, open).ok_or_else(|| {
                Diagnostic::new(
                    "JPP1008",
                    "Property set block is not closed.",
                    location.clone(),
                )
            })?;
            property.setter_body = Some(trim_body_edges(&trimmed[open + 1..close]));
            return Ok(());
        }
    }

    if property.getter && !property.setter {
        property.getter_body = Some(trim_body_edges(body));
        return Ok(());
    }

    Err(Diagnostic::new(
        "JPP1009",
        format!("Property '{}' has a body JPP does not understand.", property.name),
        location,
    )
    .with_suggestion("Use a getter body on read-only properties or a 'set { ... }' block on settable properties."))
}

fn find_set_block(body: &str) -> Option<usize> {
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.starts_with("set ") || trimmed.starts_with("set{") || trimmed == "set\n" {
            return Some(offset + line.len() - trimmed.len());
        }
        offset += line.len();
    }
    None
}

fn trim_body_edges(body: &str) -> String {
    let lines = body.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map(|index| index + 1)
        .unwrap_or(start);

    lines[start..end].join("\n")
}

fn find_class_name(source: &str) -> Option<String> {
    let mut previous = "";
    for token in source.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_') {
        if previous == "class" && !token.is_empty() {
            return Some(token.to_string());
        }
        if !token.is_empty() {
            previous = token;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use jpp_model::Segment;

    #[test]
    fn parses_property_islands_and_preserves_java() {
        let parsed = parse_source(
            None,
            "public class Customer {\n    prop get set String firstName;\n}\n",
        )
        .unwrap();

        assert_eq!(parsed.class_name.as_deref(), Some("Customer"));
        assert_eq!(parsed.segments.len(), 3);

        match &parsed.segments[1] {
            Segment::Property(prop) => {
                assert_eq!(prop.name, "firstName");
                assert_eq!(prop.ty, "String");
                assert!(prop.getter);
                assert!(prop.setter);
                assert_eq!(prop.indent, "    ");
            }
            other => panic!("expected property, got {other:?}"),
        }
    }

    #[test]
    fn parses_set_block() {
        let parsed = parse_source(
            None,
            "class C {\n    prop get set String email {\n        set {\n            value = value.trim();\n        }\n    }\n}\n",
        )
        .unwrap();

        let Segment::Property(prop) = &parsed.segments[1] else {
            panic!("expected property");
        };

        assert!(prop.setter_body.as_ref().unwrap().contains("value.trim()"));
    }

    #[test]
    fn parses_mapper_islands() {
        let parsed = parse_source(
            None,
            "class C {\n    mapper CustomerSummary summary {\n        displayName = fullName();\n        referrerName = referrer?.fullName() ?: \"\";\n    }\n}\n",
        )
        .unwrap();

        let Segment::Mapper(mapper) = &parsed.segments[1] else {
            panic!("expected mapper");
        };

        assert_eq!(mapper.target_type, "CustomerSummary");
        assert_eq!(mapper.method_name, "summary");
        assert_eq!(mapper.assignments.len(), 2);
        assert_eq!(mapper.assignments[0].target_property, "displayName");
        assert_eq!(
            mapper.assignments[1].expression,
            "referrer?.fullName() ?: \"\""
        );
    }
}

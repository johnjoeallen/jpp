package io.github.jpp.maven;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.stream.Stream;

final class JppEngine {
    private JppEngine() {
    }

    static void generate(Path input, Path output) throws JppException {
        List<Path> files = findJppFiles(input);
        for (Path file : files) {
            try {
                String source = Files.readString(file, StandardCharsets.UTF_8);
                ParsedFile parsed = parseSource(file, source);
                String java = generateJava(parsed);
                String relative = input.toFile().isFile()
                    ? replaceExtension(file.getFileName().toString(), ".java")
                    : replaceExtension(input.relativize(file).toString(), ".java");
                Path destination = output.resolve(relative);
                Path parent = destination.getParent();
                if (parent != null) {
                    Files.createDirectories(parent);
                }
                Files.writeString(destination, java, StandardCharsets.UTF_8);
            } catch (IOException ex) {
                throw new JppException("Could not process " + file + ": " + ex.getMessage(), ex);
            }
        }
    }

    private static List<Path> findJppFiles(Path input) throws JppException {
        List<Path> files = new ArrayList<>();

        try {
            if (Files.isRegularFile(input)) {
                if (input.getFileName().toString().endsWith(".jpp")) {
                    files.add(input);
                }
            } else if (Files.exists(input)) {
                try (Stream<Path> stream = Files.walk(input)) {
                    stream.filter(Files::isRegularFile)
                        .filter(path -> path.getFileName().toString().endsWith(".jpp"))
                        .forEach(files::add);
                }
            }
        } catch (IOException ex) {
            throw new JppException("Could not read input path " + input + ": " + ex.getMessage(), ex);
        }

        Collections.sort(files);
        return files;
    }

    private static ParsedFile parseSource(Path path, String source) throws JppException {
        return new Parser(path, source).parse();
    }

    private static String generateJava(ParsedFile file) throws JppException {
        if (file.className == null) {
            throw new JppException("Could not find a Java class declaration for generated methods.");
        }

        List<Property> finalProperties = new ArrayList<>();
        for (Segment segment : file.segments) {
            if (segment instanceof Property) {
                Property property = (Property) segment;
                if (property.finalField) {
                    finalProperties.add(property);
                }
            }
        }

        StringBuilder output = new StringBuilder();
        boolean[] emittedConstructor = new boolean[] { false };
        for (Segment segment : file.segments) {
            if (segment instanceof JavaSegment) {
                output.append(rewriteNullSafeAccess(((JavaSegment) segment).text));
            } else if (segment instanceof Property) {
                Property property = (Property) segment;
                output.append(generateProperty(file.className, property, finalProperties, emittedConstructor));
            } else if (segment instanceof Mapper) {
                output.append(generateMapper((Mapper) segment));
            }
        }

        return output.toString();
    }

    private static String generateMapper(Mapper mapper) {
        String indent = mapper.indent;
        StringBuilder output = new StringBuilder();
        output.append(indent).append("public ").append(mapper.targetType).append(" ")
            .append(mapper.methodName).append("() {\n");
        output.append(indent).append("    ").append(mapper.targetType).append(" target = new ")
            .append(mapper.targetType).append("();\n\n");
        for (MapperAssignment assignment : mapper.assignments) {
            output.append(indent).append("    target.").append(assignment.targetProperty)
                .append("(").append(rewriteNullSafeAccess(assignment.expression)).append(");\n");
        }
        output.append("\n").append(indent).append("    return target;\n");
        output.append(indent).append("}\n");
        return output.toString();
    }

    private static String generateProperty(String className, Property property, List<Property> finalProperties, boolean[] emittedConstructor) {
        StringBuilder output = new StringBuilder();
        String indent = property.indent;

        if (property.hasBackingField()) {
            if (property.finalField) {
                output.append(indent).append("private final ").append(property.ty).append(" ")
                    .append(property.name).append(";\n");
            } else if (!property.isCalculated()) {
                output.append(indent).append("private ").append(property.ty).append(" ")
                    .append(property.name).append(";\n");
            }
        }

        if (property.finalField && !emittedConstructor[0]) {
            output.append("\n");
            output.append(generateConstructor(className, finalProperties, indent));
            emittedConstructor[0] = true;
        }

        if (property.getter) {
            output.append("\n");
            output.append(generateGetter(property, indent));
        }

        if (property.setter || property.once) {
            output.append("\n");
            output.append(generateSetter(className, property, indent));
        }

        return output.toString();
    }

    private static String generateConstructor(String className, List<Property> properties, String indent) {
        StringBuilder output = new StringBuilder();
        output.append(indent).append("public ").append(className).append("(\n");
        for (int i = 0; i < properties.size(); i++) {
            Property property = properties.get(i);
            String comma = (i + 1 == properties.size()) ? "" : ",";
            output.append(indent).append("    ").append(property.ty).append(" ").append(property.name)
                .append(comma).append("\n");
        }
        output.append(indent).append(") {\n");
        for (Property property : properties) {
            output.append(indent).append("    this.").append(property.name).append(" = ")
                .append(property.name).append(";\n");
        }
        output.append(indent).append("}\n");
        return output.toString();
    }

    private static String generateGetter(Property property, String indent) {
        StringBuilder output = new StringBuilder();
        output.append(indent).append("public ").append(property.ty).append(" ")
            .append(property.name).append("() {\n");
        if (property.getterBody != null) {
            output.append(indentBody(rewriteNullSafeAccess(property.getterBody), indent, "    "));
        } else {
            output.append(indent).append("    return this.").append(property.name).append(";\n");
        }
        output.append(indent).append("}\n");
        return output.toString();
    }

    private static String generateSetter(String className, Property property, String indent) {
        StringBuilder output = new StringBuilder();
        String synchronizedKeyword = property.once ? " synchronized" : "";
        output.append(indent).append("public").append(synchronizedKeyword).append(" ")
            .append(className).append(" ").append(property.name).append("(")
            .append(property.ty).append(" value) {\n");

        if (property.once) {
            output.append(generateNullCheck(indent, "value", property.name)).append('\n');
            output.append(indent).append("    if (this.").append(property.name).append(" != null) {\n");
            output.append(indent).append("        throw new IllegalStateException(\n");
            output.append(indent).append("            \"Property '").append(property.name).append("' is once-only\"\n");
            output.append(indent).append("        );\n");
            output.append(indent).append("    }\n\n");
        }

        if (property.setterBody != null) {
            output.append(indentBody(rewriteNullSafeAccess(property.setterBody), indent, "    "));
            output.append('\n');
        }

        output.append(indent).append("    this.").append(property.name).append(" = value;\n\n");
        output.append(indent).append("    return this;\n");
        output.append(indent).append("}\n");
        return output.toString();
    }

    private static String generateNullCheck(String indent, String variable, String propertyName) {
        return indent + "    if (" + variable + " == null) {\n"
            + indent + "        throw new NullPointerException(\n"
            + indent + "            \"Property '" + propertyName + "' cannot be null\"\n"
            + indent + "        );\n"
            + indent + "    }\n";
    }

    private static String rewriteNullSafeAccess(String source) {
        StringBuilder output = new StringBuilder(source.length());
        int index = 0;
        while (index < source.length()) {
            int operator = findNullSafeOperator(source, index);
            if (operator >= 0) {
                NullSafeRewrite rewrite = parseNullSafeAccess(source, operator);
                if (rewrite != null) {
                    output.append(source, index, rewrite.receiverStart);
                    output.append(generateNullSafeAccess(rewrite));
                    index = rewrite.end;
                    continue;
                }
                output.append(source, index, operator + 1);
                index = operator + 1;
            } else {
                output.append(source.substring(index));
                break;
            }
        }
        return output.toString();
    }

    private static String generateNullSafeAccess(NullSafeRewrite rewrite) {
        if (rewrite.fallback != null) {
            return "java.util.Optional.ofNullable(" + rewrite.receiver + ").map(__jpp_value -> __jpp_value."
                + rewrite.member + ").orElse(" + rewrite.fallback + ")";
        }
        return "(" + rewrite.receiver + " == null ? null : " + rewrite.receiver + "." + rewrite.member + ")";
    }

    private static int findNullSafeOperator(String source, int from) {
        ScanState state = ScanState.CODE;
        for (int index = from; index + 1 < source.length(); index++) {
            char ch = source.charAt(index);
            char next = source.charAt(index + 1);
            switch (state) {
                case CODE:
                    if (ch == '"') {
                        state = ScanState.STRING;
                    } else if (ch == '\'') {
                        state = ScanState.CHAR;
                    } else if (ch == '/' && next == '/') {
                        state = ScanState.LINE_COMMENT;
                        index++;
                    } else if (ch == '/' && next == '*') {
                        state = ScanState.BLOCK_COMMENT;
                        index++;
                    } else if (ch == '?' && next == '.') {
                        return index;
                    }
                    break;
                case STRING:
                    if (ch == '\\') {
                        index++;
                    } else if (ch == '"') {
                        state = ScanState.CODE;
                    }
                    break;
                case CHAR:
                    if (ch == '\\') {
                        index++;
                    } else if (ch == '\'') {
                        state = ScanState.CODE;
                    }
                    break;
                case LINE_COMMENT:
                    if (ch == '\n') {
                        state = ScanState.CODE;
                    }
                    break;
                case BLOCK_COMMENT:
                    if (ch == '*' && next == '/') {
                        state = ScanState.CODE;
                        index++;
                    }
                    break;
            }
        }
        return -1;
    }

    private static NullSafeRewrite parseNullSafeAccess(String source, int operator) {
        int receiverEnd = skipWhitespaceBack(source, operator);
        int receiverStart = findReceiverStart(source, receiverEnd);
        if (receiverStart < 0) {
            return null;
        }
        String receiver = source.substring(receiverStart, receiverEnd);
        int memberStart = operator + 2;
        int memberEnd = findMemberEnd(source, memberStart);
        if (memberEnd < 0) {
            return null;
        }
        String member = source.substring(memberStart, memberEnd);
        FallbackResult fallback = parseNullSafeFallback(source, memberEnd);
        return new NullSafeRewrite(receiverStart, receiver, member, fallback.fallback, fallback.end);
    }

    private static FallbackResult parseNullSafeFallback(String source, int memberEnd) {
        int fallbackOperator = skipWhitespaceForward(source, memberEnd);
        if (fallbackOperator + 1 >= source.length() || !source.startsWith("?:", fallbackOperator)) {
            return new FallbackResult(null, memberEnd);
        }
        int fallbackStart = skipWhitespaceForward(source, fallbackOperator + 2);
        int fallbackEnd = findFallbackEnd(source, fallbackStart);
        int trimmedEnd = trimWhitespaceBack(source, fallbackStart, fallbackEnd);
        return new FallbackResult(source.substring(fallbackStart, trimmedEnd), fallbackEnd);
    }

    private static int skipWhitespaceForward(String source, int index) {
        while (index < source.length() && Character.isWhitespace(source.charAt(index))) {
            index++;
        }
        return index;
    }

    private static int skipWhitespaceBack(String source, int index) {
        while (index > 0 && Character.isWhitespace(source.charAt(index - 1))) {
            index--;
        }
        return index;
    }

    private static int trimWhitespaceBack(String source, int start, int end) {
        while (end > start && Character.isWhitespace(source.charAt(end - 1))) {
            end--;
        }
        return end;
    }

    private static int findFallbackEnd(String source, int start) {
        int parenDepth = 0;
        int bracketDepth = 0;
        int braceDepth = 0;
        ScanState state = ScanState.CODE;
        for (int index = start; index < source.length(); index++) {
            char ch = source.charAt(index);
            switch (state) {
                case CODE:
                    if ((ch == ';' || ch == '\n') && parenDepth == 0 && bracketDepth == 0 && braceDepth == 0) {
                        return index;
                    }
                    if (ch == '(') {
                        parenDepth++;
                    } else if (ch == ')' && parenDepth > 0) {
                        parenDepth--;
                    } else if (ch == '[') {
                        bracketDepth++;
                    } else if (ch == ']' && bracketDepth > 0) {
                        bracketDepth--;
                    } else if (ch == '{') {
                        braceDepth++;
                    } else if (ch == '}' && braceDepth > 0) {
                        braceDepth--;
                    } else if (ch == '"') {
                        state = ScanState.STRING;
                    } else if (ch == '\'') {
                        state = ScanState.CHAR;
                    }
                    break;
                case STRING:
                    if (ch == '\\') {
                        index++;
                    } else if (ch == '"') {
                        state = ScanState.CODE;
                    }
                    break;
                case CHAR:
                    if (ch == '\\') {
                        index++;
                    } else if (ch == '\'') {
                        state = ScanState.CODE;
                    }
                    break;
                case LINE_COMMENT:
                case BLOCK_COMMENT:
                    break;
            }
        }
        return source.length();
    }

    private static int findReceiverStart(String source, int end) {
        int index = end;
        while (index > 0) {
            char ch = source.charAt(index - 1);
            if (Character.isLetterOrDigit(ch) || ch == '_' || ch == '.') {
                index--;
            } else {
                break;
            }
        }
        return index == end ? -1 : index;
    }

    private static int findMemberEnd(String source, int start) {
        int index = readIdentifier(source, start);
        if (index < 0) {
            return -1;
        }
        while (index < source.length()) {
            char ch = source.charAt(index);
            if (ch == '(') {
                index = readBalanced(source, index, '(', ')');
            } else if (ch == '[') {
                index = readBalanced(source, index, '[', ']');
            } else if (ch == '.') {
                index = readIdentifier(source, index + 1);
            } else {
                return index;
            }
            if (index < 0) {
                return -1;
            }
        }
        return index;
    }

    private static int readIdentifier(String source, int start) {
        if (start >= source.length()) {
            return -1;
        }
        char first = source.charAt(start);
        if (!Character.isLetter(first) && first != '_') {
            return -1;
        }
        int index = start + 1;
        while (index < source.length()) {
            char ch = source.charAt(index);
            if (Character.isLetterOrDigit(ch) || ch == '_') {
                index++;
            } else {
                break;
            }
        }
        return index;
    }

    private static int readBalanced(String source, int start, char open, char close) {
        int depth = 0;
        ScanState state = ScanState.CODE;
        for (int index = start; index < source.length(); index++) {
            char ch = source.charAt(index);
            switch (state) {
                case CODE:
                    if (ch == open) {
                        depth++;
                    } else if (ch == close) {
                        depth--;
                        if (depth == 0) {
                            return index + 1;
                        }
                    } else if (ch == '"') {
                        state = ScanState.STRING;
                    } else if (ch == '\'') {
                        state = ScanState.CHAR;
                    }
                    break;
                case STRING:
                    if (ch == '\\') {
                        index++;
                    } else if (ch == '"') {
                        state = ScanState.CODE;
                    }
                    break;
                case CHAR:
                    if (ch == '\\') {
                        index++;
                    } else if (ch == '\'') {
                        state = ScanState.CODE;
                    }
                    break;
                case LINE_COMMENT:
                case BLOCK_COMMENT:
                    break;
            }
        }
        return -1;
    }

    private static String indentBody(String body, String baseIndent, String extraIndent) {
        StringBuilder output = new StringBuilder();
        int commonIndent = commonIndent(body);
        String[] lines = body.split("\n", -1);
        for (String line : lines) {
            if (line.trim().isEmpty()) {
                output.append('\n');
            } else {
                output.append(baseIndent).append(extraIndent);
                int start = Math.min(commonIndent, line.length());
                output.append(line.substring(start).replaceFirst("\\s+$", ""));
                output.append('\n');
            }
        }
        return output.toString();
    }

    private static int commonIndent(String body) {
        int min = Integer.MAX_VALUE;
        String[] lines = body.split("\n", -1);
        for (String line : lines) {
            if (!line.trim().isEmpty()) {
                int indent = 0;
                while (indent < line.length()) {
                    char ch = line.charAt(indent);
                    if (ch == ' ' || ch == '\t') {
                        indent++;
                    } else {
                        break;
                    }
                }
                min = Math.min(min, indent);
            }
        }
        return min == Integer.MAX_VALUE ? 0 : min;
    }

    private static String replaceExtension(String fileName, String newExtension) {
        int lastDot = fileName.lastIndexOf('.');
        String base = lastDot >= 0 ? fileName.substring(0, lastDot) : fileName;
        return base + newExtension;
    }

    private static final class Parser {
        private final Path path;
        private final String source;
        private int cursor;
        private final List<Segment> segments = new ArrayList<>();

        private Parser(Path path, String source) {
            this.path = path;
            this.source = source;
        }

        private ParsedFile parse() throws JppException {
            while (true) {
                ExtensionIsland islandStart = findExtensionIsland(source, cursor);
                if (islandStart == null) {
                    break;
                }
                if (islandStart.start > cursor) {
                    segments.add(new JavaSegment(source.substring(cursor, islandStart.start)));
                }

                if (islandStart.kind == IslandKind.PROPERTY) {
                    PropertyIsland island = parsePropertyAt(islandStart.start);
                    cursor = island.end;
                    segments.add(island.property);
                } else {
                    MapperIsland island = parseMapperAt(islandStart.start);
                    cursor = island.end;
                    segments.add(island.mapper);
                }
            }

            if (cursor < source.length()) {
                segments.add(new JavaSegment(source.substring(cursor)));
            }

            return new ParsedFile(path, source, findClassName(source), segments);
        }

        private PropertyIsland parsePropertyAt(int start) throws JppException {
            int afterIndent = start;
            while (afterIndent < source.length()) {
                char ch = source.charAt(afterIndent);
                if (ch == ' ' || ch == '\t') {
                    afterIndent++;
                } else {
                    break;
                }
            }
            String indent = source.substring(start, afterIndent);
            int headerStart = afterIndent + "prop".length();
            int headerEnd = findHeaderEnd(source, headerStart);
            if (headerEnd < 0) {
                throw new JppException("Unterminated property declaration.");
            }

            String header = source.substring(headerStart, headerEnd).trim();
            Property parsed = parsePropertyHeader(header);
            parsed.indent = indent;

            int end;
            char marker = source.charAt(headerEnd);
            if (marker == ';') {
                end = headerEnd + 1;
            } else {
                int bodyEnd = findMatchingBrace(source, headerEnd);
                if (bodyEnd < 0) {
                    throw new JppException("Property '" + parsed.name + "' has an unclosed body.");
                }
                String body = source.substring(headerEnd + 1, bodyEnd);
                applyPropertyBody(parsed, body);
                end = bodyEnd + 1;
            }

            return new PropertyIsland(parsed, end);
        }

        private MapperIsland parseMapperAt(int start) throws JppException {
            int afterIndent = start;
            while (afterIndent < source.length()) {
                char ch = source.charAt(afterIndent);
                if (ch == ' ' || ch == '\t') {
                    afterIndent++;
                } else {
                    break;
                }
            }
            String indent = source.substring(start, afterIndent);
            int headerStart = afterIndent + "mapper".length();
            int headerEnd = findHeaderEnd(source, headerStart);
            if (headerEnd < 0) {
                throw new JppException("Unterminated mapper declaration.");
            }
            if (source.charAt(headerEnd) != '{') {
                throw new JppException("Mapper declarations require a body.");
            }

            String header = source.substring(headerStart, headerEnd).trim();
            Mapper mapper = parseMapperHeader(header);
            mapper.indent = indent;

            int bodyEnd = findMatchingBrace(source, headerEnd);
            if (bodyEnd < 0) {
                throw new JppException("Mapper '" + mapper.methodName + "' has an unclosed body.");
            }
            String body = source.substring(headerEnd + 1, bodyEnd);
            mapper.assignments = parseMapperBody(body);
            return new MapperIsland(mapper, bodyEnd + 1);
        }
    }

    private static ExtensionIsland findExtensionIsland(String source, int from) {
        int index = from;
        while (index < source.length()) {
            int lineEnd = source.indexOf('\n', index);
            if (lineEnd < 0) {
                lineEnd = source.length();
            }
            String line = source.substring(index, lineEnd);
            String trimmed = trimStartSpaces(line);
            if (trimmed.startsWith("prop ") || trimmed.startsWith("prop\t")) {
                return new ExtensionIsland(index, IslandKind.PROPERTY);
            }
            if (trimmed.startsWith("mapper ") || trimmed.startsWith("mapper\t")) {
                return new ExtensionIsland(index, IslandKind.MAPPER);
            }
            index = lineEnd + 1;
        }
        return null;
    }

    private static Mapper parseMapperHeader(String header) throws JppException {
        String[] tokens = header.trim().split("\\s+");
        if (tokens.length != 2) {
            throw new JppException("Mapper declaration must include a target type and method name.");
        }
        return new Mapper(tokens[0], tokens[1]);
    }

    private static List<MapperAssignment> parseMapperBody(String body) throws JppException {
        List<MapperAssignment> assignments = new ArrayList<>();
        for (String statement : splitMapperStatements(body)) {
            String trimmed = statement.trim();
            if (trimmed.isEmpty()) {
                continue;
            }
            int eq = trimmed.indexOf('=');
            if (eq < 0) {
                throw new JppException("Mapper assignment '" + trimmed + "' must use '='.");
            }
            String target = trimmed.substring(0, eq).trim();
            String expression = trimmed.substring(eq + 1).trim();
            if (target.isEmpty() || expression.isEmpty()) {
                throw new JppException("Mapper assignment must include a target property and expression.");
            }
            assignments.add(new MapperAssignment(target, expression));
        }
        if (assignments.isEmpty()) {
            throw new JppException("Mapper body must contain at least one assignment.");
        }
        return assignments;
    }

    private static List<String> splitMapperStatements(String body) {
        List<String> statements = new ArrayList<>();
        int start = 0;
        int index = 0;
        int depth = 0;
        boolean inString = false;
        boolean inChar = false;
        boolean escaped = false;

        while (index < body.length()) {
            char ch = body.charAt(index);
            if (escaped) {
                escaped = false;
                index++;
                continue;
            }

            if (inString || inChar) {
                if (ch == '\\') {
                    escaped = true;
                } else if (inString && ch == '"') {
                    inString = false;
                } else if (inChar && ch == '\'') {
                    inChar = false;
                }
            } else {
                if (ch == '"') {
                    inString = true;
                } else if (ch == '\'') {
                    inChar = true;
                } else if (ch == '(' || ch == '[' || ch == '{') {
                    depth++;
                } else if (ch == ')' || ch == ']' || ch == '}') {
                    if (depth > 0) {
                        depth--;
                    }
                } else if (ch == ';' && depth == 0) {
                    statements.add(body.substring(start, index));
                    start = index + 1;
                }
            }

            index++;
        }

        if (start < body.length()) {
            statements.add(body.substring(start));
        }
        return statements;
    }

    private static int findHeaderEnd(String source, int from) {
        boolean inString = false;
        boolean inChar = false;
        boolean escaped = false;
        for (int index = from; index < source.length(); index++) {
            char ch = source.charAt(index);
            if (escaped) {
                escaped = false;
                continue;
            }
            if (ch == '\\' && (inString || inChar)) {
                escaped = true;
            } else if (ch == '"' && !inChar) {
                inString = !inString;
            } else if (ch == '\'' && !inString) {
                inChar = !inChar;
            } else if ((ch == ';' || ch == '{') && !inString && !inChar) {
                return index;
            }
        }
        return -1;
    }

    private static int findMatchingBrace(String source, int open) {
        int depth = 0;
        boolean inString = false;
        boolean inChar = false;
        boolean escaped = false;
        for (int index = open; index < source.length(); index++) {
            char ch = source.charAt(index);
            if (escaped) {
                escaped = false;
                continue;
            }
            if (ch == '\\' && (inString || inChar)) {
                escaped = true;
            } else if (ch == '"' && !inChar) {
                inString = !inString;
            } else if (ch == '\'' && !inString) {
                inChar = !inChar;
            } else if (ch == '{' && !inString && !inChar) {
                depth++;
            } else if (ch == '}' && !inString && !inChar) {
                depth--;
                if (depth == 0) {
                    return index;
                }
            }
        }
        return -1;
    }

    private static Property parsePropertyHeader(String header) throws JppException {
        String[] tokens = header.trim().split("\\s+");
        if (tokens.length < 3) {
            throw new JppException("Property declaration is incomplete.");
        }

        boolean getter = false;
        boolean setter = false;
        boolean once = false;
        boolean finalField = false;
        int index = 0;
        while (index < tokens.length) {
            String token = tokens[index];
            if ("get".equals(token)) {
                getter = true;
            } else if ("set".equals(token)) {
                setter = true;
            } else if ("once".equals(token)) {
                once = true;
            } else if ("final".equals(token)) {
                finalField = true;
            } else {
                break;
            }
            index++;
        }

        if (!getter && !setter) {
            throw new JppException("Property must declare at least 'get' or 'set'.");
        }
        if (once && !getter) {
            throw new JppException("Once-only properties must be readable.");
        }
        if (finalField && setter) {
            throw new JppException("Final properties cannot declare a setter.");
        }
        if (finalField && once) {
            throw new JppException("A property cannot be both 'final' and 'once'.");
        }
        if (tokens.length - index < 2) {
            throw new JppException("Property declaration must include a type and name.");
        }

        String name = tokens[tokens.length - 1];
        StringBuilder ty = new StringBuilder();
        for (int i = index; i < tokens.length - 1; i++) {
            if (i > index) {
                ty.append(' ');
            }
            ty.append(tokens[i]);
        }

        return new Property(name, ty.toString(), getter, setter, once, finalField);
    }

    private static void applyPropertyBody(Property property, String body) throws JppException {
        String trimmed = body.trim();

        if (property.setter) {
            int setStart = findSetBlock(trimmed);
            if (setStart >= 0) {
                int open = trimmed.indexOf('{', setStart);
                int close = findMatchingBrace(trimmed, open);
                if (close < 0) {
                    throw new JppException("Property set block is not closed.");
                }
                property.setterBody = trimBodyEdges(trimmed.substring(open + 1, close));
                return;
            }
        }

        if (property.getter && !property.setter) {
            property.getterBody = trimBodyEdges(body);
            return;
        }

        throw new JppException("Property '" + property.name + "' has a body JPP does not understand.");
    }

    private static int findSetBlock(String body) {
        int offset = 0;
        String[] lines = body.split("\n", -1);
        for (String line : lines) {
            String trimmed = trimStartSpaces(line);
            if (trimmed.startsWith("set ") || trimmed.startsWith("set{") || "set".equals(trimmed)) {
                return offset + line.length() - trimmed.length();
            }
            offset += line.length() + 1;
        }
        return -1;
    }

    private static String trimBodyEdges(String body) {
        String[] lines = body.split("\n", -1);
        int start = 0;
        while (start < lines.length && lines[start].trim().isEmpty()) {
            start++;
        }
        int end = lines.length;
        while (end > start && lines[end - 1].trim().isEmpty()) {
            end--;
        }
        StringBuilder output = new StringBuilder();
        for (int i = start; i < end; i++) {
            if (i > start) {
                output.append('\n');
            }
            output.append(lines[i]);
        }
        return output.toString();
    }

    private static String trimStartSpaces(String value) {
        int index = 0;
        while (index < value.length()) {
            char ch = value.charAt(index);
            if (ch == ' ' || ch == '\t') {
                index++;
            } else {
                break;
            }
        }
        return value.substring(index);
    }

    private static String findClassName(String source) {
        String previous = "";
        String[] tokens = source.split("[^A-Za-z0-9_]+");
        for (String token : tokens) {
            if ("class".equals(previous) && !token.isEmpty()) {
                return token;
            }
            if (!token.isEmpty()) {
                previous = token;
            }
        }
        return null;
    }

    private static final class ParsedFile {
        final Path path;
        final String source;
        final String className;
        final List<Segment> segments;

        ParsedFile(Path path, String source, String className, List<Segment> segments) {
            this.path = path;
            this.source = source;
            this.className = className;
            this.segments = segments;
        }
    }

    private interface Segment {
    }

    private static final class JavaSegment implements Segment {
        final String text;

        JavaSegment(String text) {
            this.text = text;
        }
    }

    private static final class Property implements Segment {
        final String name;
        final String ty;
        final boolean getter;
        final boolean setter;
        final boolean once;
        final boolean finalField;
        String getterBody;
        String setterBody;
        String indent = "";

        Property(String name, String ty, boolean getter, boolean setter, boolean once, boolean finalField) {
            this.name = name;
            this.ty = ty;
            this.getter = getter;
            this.setter = setter;
            this.once = once;
            this.finalField = finalField;
        }

        boolean hasBackingField() {
            return setter || once || finalField || getterBody == null;
        }

        boolean isCalculated() {
            return getter && !setter && !once && !finalField && getterBody != null;
        }
    }

    private static final class Mapper implements Segment {
        final String targetType;
        final String methodName;
        List<MapperAssignment> assignments = new ArrayList<>();
        String indent = "";

        Mapper(String targetType, String methodName) {
            this.targetType = targetType;
            this.methodName = methodName;
        }
    }

    private static final class MapperAssignment {
        final String targetProperty;
        final String expression;

        MapperAssignment(String targetProperty, String expression) {
            this.targetProperty = targetProperty;
            this.expression = expression;
        }
    }

    private enum ScanState {
        CODE,
        STRING,
        CHAR,
        LINE_COMMENT,
        BLOCK_COMMENT
    }

    private enum IslandKind {
        PROPERTY,
        MAPPER
    }

    private static final class ExtensionIsland {
        final int start;
        final IslandKind kind;

        ExtensionIsland(int start, IslandKind kind) {
            this.start = start;
            this.kind = kind;
        }
    }

    private static final class PropertyIsland {
        final Property property;
        final int end;

        PropertyIsland(Property property, int end) {
            this.property = property;
            this.end = end;
        }
    }

    private static final class MapperIsland {
        final Mapper mapper;
        final int end;

        MapperIsland(Mapper mapper, int end) {
            this.mapper = mapper;
            this.end = end;
        }
    }

    private static final class NullSafeRewrite {
        final int receiverStart;
        final String receiver;
        final String member;
        final String fallback;
        final int end;

        NullSafeRewrite(int receiverStart, String receiver, String member, String fallback, int end) {
            this.receiverStart = receiverStart;
            this.receiver = receiver;
            this.member = member;
            this.fallback = fallback;
            this.end = end;
        }
    }

    private static final class FallbackResult {
        final String fallback;
        final int end;

        FallbackResult(String fallback, int end) {
            this.fallback = fallback;
            this.end = end;
        }
    }
}

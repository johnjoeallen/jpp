package io.github.jpp.maven;

final class JppException extends Exception {
    JppException(String message) {
        super(message);
    }

    JppException(String message, Throwable cause) {
        super(message, cause);
    }
}

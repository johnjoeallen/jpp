#!/usr/bin/env bash
set -euo pipefail

mvn install
mvn -f examples/pom.xml compile

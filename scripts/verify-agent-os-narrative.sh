#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_pattern() {
  local file="$1"
  local pattern="$2"
  if ! grep -Fq "$pattern" "$file"; then
    echo "missing required narrative anchor in $file: $pattern" >&2
    exit 1
  fi
}

require_absent() {
  local file="$1"
  local pattern="$2"
  if grep -Fq "$pattern" "$file"; then
    echo "forbidden narrative drift in $file: $pattern" >&2
    exit 1
  fi
}

require_pattern README.md "runtime-centered Enterprise Agent OS"
require_pattern README.md "Manager Runtime and Managed Runtime"
require_pattern README.md "Environment Scheduling Layer"
require_pattern README.md "Ontology Action Contract Layer"
require_pattern README.md "K Agent"

require_pattern docs/architecture.md "Environment Scheduling Layer"
require_pattern docs/architecture.md "K Agent does not own ManagerPlan"
require_pattern docs/architecture.md "Ontology Action Contract defines business objects"

require_pattern docs/stage2-stage3-roadmap.md "Full Agent OS Implementation Phases"
require_pattern docs/stage2-stage3-roadmap.md "Environment Scheduling + K Agent"
require_pattern docs/stage2-stage3-roadmap.md "Ontology Action Contract"

require_pattern docs/agent-remote-computer-plan.md "Full Agent OS Alignment"
require_pattern docs/agent-remote-computer-plan.md "K Agent Guardrail"
require_pattern docs/agent-remote-computer-plan.md "approved execution envelope"

require_absent README.md "Ontology is the product center"
require_absent README.md "Work Surfaces -> Collaboration -> Manager Agent / Work Coordination -> Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent README.md "Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent README.md "Enterprise Data Foundation"
require_absent README.md "Semantic layers are the next productization surface"
require_absent docs/architecture.md "Remote Computer is the top-level product object"
require_absent docs/architecture.md "Work Surfaces -> Collaboration -> Manager Agent / Work Coordination -> Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent docs/architecture.md "Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent docs/architecture.md "Enterprise Data Foundation"
require_absent docs/architecture.md "## Semantic Layer"
require_absent docs/agent-remote-computer-plan.md "K Agent owns Policy"
require_absent docs/agent-remote-computer-plan.md "Work Surfaces -> Collaboration -> Manager Agent / Work Coordination -> Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent docs/agent-remote-computer-plan.md "Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent docs/agent-remote-computer-plan.md "Enterprise Data Foundation"
require_absent docs/stage2-stage3-roadmap.md "Work Surfaces -> Collaboration -> Manager Agent / Work Coordination -> Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent docs/stage2-stage3-roadmap.md "Managed Runtime -> Semantic Layer -> Data Foundation"
require_absent docs/stage2-stage3-roadmap.md "Enterprise Data Foundation"
require_absent docs/stage2-stage3-roadmap.md "Semantic Layer / Ontology Service"

echo "agent_os_narrative=ok"

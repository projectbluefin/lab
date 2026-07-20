# Release evidence

This document explains where release-trust evidence is defined. It does not
record the current release state; current state is generated from repository
inputs and published test results.

## Canonical sources

- Release verdict definition: [`adr/0002-release-verdict-definition.md`](adr/0002-release-verdict-definition.md)
- Dashboard data contracts: [`reference/page-contracts.md`](reference/page-contracts.md)
- Workflow parameter contracts: [`reference/WORKFLOWS.md`](reference/WORKFLOWS.md)
- Test procedures: [`testing.md`](testing.md)

## Evidence requirements

A release claim should identify:

1. The artifact or immutable digest under test.
2. The workflow or test suite that produced the evidence.
3. The publication time and result source.
4. The verification rule used to interpret the result.

Do not edit generated result files by hand. Fix the producing workflow or
source data and regenerate the artifact.

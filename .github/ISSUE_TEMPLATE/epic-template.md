---
name: Epic Template
about: Epic template with integrated release checklist
title: "[EPIC]"
labels: ''
assignees: ''

---

# Epic Template: [Epic Name] v[Version]

## Epic Overview

**Release Version:** v[X.Y.Z]  
**Target Release Date:** [YYYY-MM-DD]  
**Epic Lead:** @[username]  
**Technical Owner:** @[username]  
**Business Owner:** @[username]  

### Epic Description

[Provide a clear, concise description of what this epic delivers and why]

### Success Criteria

- [ ] [Specific measurable outcome 1]
- [ ] [Specific measurable outcome 2]
- [ ] [Specific measurable outcome 3]

---

## Pre-Implementation Checklist

> **Note**: This section should reference completed IP process rather than duplicate work. Strike through items not applicable to this release with reasoning.

### Architecture Pipeline Completion

- [ ] **Improvement Proposal Approved** _(reference IP-YYYY-MM-DD: [Title] - issue #xxx)_
  - [ ] Strategic review complete (internal) OR Community consensus achieved (external)
  - [ ] Technical feasibility confirmed
  - [ ] All IP comments resolved
  - [ ] Final approval documented

### Implementation Prerequisites  

- [ ] **Technical Dependencies Resolved** _(create dependency issues #xxx or reference existing)_
- [ ] **Resource Allocation Confirmed** _(team assignments and timeline approved)_
- [ ] **Integration Planning Complete** _(downstream impact analysis from IP)_

---

## Implementation Tracking

> **Instructions**: Create GitHub issues for each applicable item below. Link the issue numbers and check when completed.

### Core Development

- [ ] **Feature Development Tasks** _(break down into specific implementation issues)_
  - [ ] [Feature 1 implementation] _(issue #xxx)_
  - [ ] [Feature 2 implementation] _(issue #xxx)_
  - [ ] [Feature N implementation] _(issue #xxx)_

### Documentation Tasks
>
> Create separate issues for each documentation type that requires updates

- [ ] **API Documentation** _(issue #xxx or ~not applicable~)_
- [ ] **CLI Documentation** _(issue #xxx or ~not applicable~)_
- [ ] **Installation Guides** _(issue #xxx or ~not applicable~)_
- [ ] **Architecture Decision Records (ADRs)** _(issue #xxx for each new ADR)_
- [ ] **User Guides** _(issue #xxx or ~not applicable~)_
- [ ] **Developer Integration Guides** _(issue #xxx or ~not applicable~)_
- [ ] **Migration Guides** _(issue #xxx or ~not applicable~)_

### Infrastructure & DevOps

- [ ] **Infrastructure Updates** _(issue #xxx for each infra change)_
- [ ] **CI/CD Pipeline Updates** _(issue #xxx or ~not applicable~)_
- [ ] **Deployment Scripts** _(issue #xxx or ~not applicable~)_
- [ ] **Monitoring & Observability** _(issue #xxx or ~not applicable~)_

### Quality Assurance

- [ ] **Testing Strategy** _(issue #xxx defining test approach)_
  - [ ] Unit Tests _(part of feature issues or separate #xxx)_
  - [ ] Integration Tests _(issue #xxx)_
  - [ ] End-to-End Tests _(issue #xxx)_
  - [ ] Performance Tests _(issue #xxx or ~not applicable~)_
- [ ] **Security Review** _(issue #xxx or ~not applicable~)_
- [ ] **Audit Preparation** _(issue #xxx or ~not applicable for this release~)_

### Release Engineering

- [ ] **Release Artifacts** _(tracked automatically via CI/CD)_
  - [ ] Binary builds _(automated)_
  - [ ] Package distributions _(automated)_  
  - [ ] Container images _(automated)_
- [ ] **Release Notes** _(issue #xxx)_
- [ ] **Changelog Updates** _(issue #xxx)_
- [ ] **Version Tagging** _(automated via release process)_

---

## Pre-Release Gates

> These items must be completed before release deployment

### Code Freeze Criteria

- [ ] All implementation issues closed
- [ ] All documentation issues closed  
- [ ] All infrastructure issues resolved
- [ ] Release candidate tagged

### Testing & Validation

- [ ] **Testing Completion** _(all testing issues closed)_
- [ ] **Security Review Passed** _(issue #xxx closed or ~not required~)_
- [ ] **Performance Validation** _(issue #xxx closed or ~not required~)_
- [ ] **Integration Testing Passed** _(issue #xxx closed)_

### Release Readiness

- [ ] **Release Notes Finalized** _(issue #xxx closed)_
- [ ] **Documentation Updated** _(all doc issues closed)_
- [ ] **Deployment Scripts Ready** _(issue #xxx closed or ~not applicable~)_
- [ ] **Rollback Plan Documented** _(issue #xxx or ~not applicable~)_

---

## Post-Release Handover

> These items are completed after successful release deployment

### Handover Tasks

- [ ] **DevRel Handover** _(create handover issue #xxx)_
  - [ ] Developer documentation review
  - [ ] Integration examples updated
  - [ ] Community communication sent
- [ ] **Product Handover** _(create handover issue #xxx)_
  - [ ] Feature validation completed
  - [ ] User feedback collection setup
  - [ ] Metrics dashboard updated
- [ ] **Marketing Handover** _(create handover issue #xxx)_
  - [ ] Release announcement published
  - [ ] Marketing materials updated
  - [ ] Partner communications sent

### Post-Release Monitoring

- [ ] **Release Health Check** _(issue #xxx for 48-hour monitoring)_
- [ ] **User Feedback Collection** _(issue #xxx)_
- [ ] **Performance Monitoring** _(issue #xxx or automated)_
- [ ] **Issue Triage Setup** _(issue #xxx for post-release support)_

---

## Epic Completion Criteria

**Epic is considered complete when:**

1. ✅ All pre-release gates passed
1. ✅ Release successfully deployed to production  
1. ✅ All handover tasks completed
1. ✅ Post-release monitoring established
1. ✅ This epic marked as closed

---

## Notes & Context

### Key Decisions Made

- [Document major architectural or product decisions made during epic]

### Assumptions & Constraints  

- [List any assumptions that could affect the epic]
- [Note any constraints that influenced the approach]

### Dependencies

- **Blocked by:** [List epics/issues this depends on]
- **Blocks:** [List epics/issues that depend on this]

### Risks & Mitigation

- **Risk:** [Description] | **Mitigation:** [Approach]

---

## Related Links

- **Improvement Proposal:** [Link to approved IP issue]
- **Strategic RFC:** [Link to Notion RFC if internal origin]
- **Community Discussion:** [Link to GitHub Discussion if external origin]
- **Technical RFC:** [Link to GitHub RFC if applicable]
- **Design Documents:** [Links to detailed design docs]
- **Previous Release:** [Link to previous epic]
- **Next Planned Release:** [Link to next epic if known]

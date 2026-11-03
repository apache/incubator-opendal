# OpenDAL Specifications

Specifications define the portable behavior OpenDAL promises: the observable
contracts that hold across every supporting service. They are living
documents; implementations, API documentation, and behavior tests must remain
consistent with them as OpenDAL evolves. Some promises apply only when the
service advertises the corresponding capability, and an unsupported request
fails with an explicit error. Behavior outside a specification's promises is
outside the portable contract, and callers must not rely on it holding across
services.

Specifications state the contract. Behavior tests under `core/tests/behavior`
provide executable conformance coverage for the specified contracts. Public
API documentation explains the observable behavior, capability requirements,
parameter meaning, and relevant errors at the API site, then links to the
specification for the complete cross-operation contract.

Accepted RFCs are immutable historical documents. They record the design that
reached consensus, including its original motivation and trade-offs.
Specifications describe the current contract, even when later implementation
work or a newer RFC changes the accepted design.

Editorial corrections and changes that codify already implemented behavior use
the normal pull request process and include evidence for that behavior. A
substantial contract change requires a new RFC. The resulting implementation,
behavior tests, API documentation, and specification change land together.

## Active specifications

- [Conditional operations](https://opendal.apache.org/docs/specifications/conditional-operations/)
  define portable predicate, error, and atomicity semantics, including the
  behavior when the file does not exist.
- [Metadata](https://opendal.apache.org/docs/specifications/metadata/) defines
  owned metadata, compact construction limits, and result completion semantics.

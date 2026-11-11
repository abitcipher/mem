# Contributing

MEM is source-available under the terms in [LICENSE.md](LICENSE.md). Contributions are welcome,
on the condition below.

## Contributor agreement

By submitting a contribution (code, documentation, tests or anything else) to this project, you
agree that:

1. **You may give it.** The contribution is your own work, or you have the right to submit it,
   and your employer (if it has rights in your work) allows it.
2. **You keep your copyright.** You are not assigning ownership.
3. **License to the maintainer.** You grant the project's licensor (named in the Required Notice
   of `LICENSE.md`) a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to
   use, reproduce, modify, distribute and sublicense your contribution, as part of the project or
   otherwise, under the project's current license terms or any other terms the licensor chooses.
4. **Patents.** You grant the licensor, and everyone who receives the project from the licensor,
   a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to any patent claims
   you can license that your contribution, alone or combined with the project, would infringe.
5. **No warranty.** You provide the contribution as is.

State your agreement in your first pull request with the line:

> I agree to the MEM contributor agreement in CONTRIBUTING.md.

## Before you open a pull request

- One concern per pull request, in small reviewable steps.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets` and
  `cargo test --workspace` pass; in `app/`, `flutter analyze` and `flutter test` pass.
- New dependencies pass `cargo deny check` and `cargo vet`. No AGPL or GPL dependencies.
- Changes to the wire format update `spec/*.cddl` and the test vectors.
- Do not weaken a test in `tests/invariants/` to make a change pass.
- Security problems go through [SECURITY.md](SECURITY.md), not public issues.

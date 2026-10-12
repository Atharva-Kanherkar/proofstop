# Repository foundation: acceptance contract

## Functional behavior

- Create a standalone public GitHub repository named `proofstop` under
  `Atharva-Kanherkar`, with a local checkout outside the Kairo repository.
- Put the research premise, proposed architecture, implementation milestones,
  experimental protocol, decision criteria, and related work in `README.md`.
- Identify the repository as a research proposal with no implemented runtime or
  measured savings. Do not invent installation commands or benchmark results.
- Separate online acceptance checks from held-out evaluation and include the
  cost of unsuccessful probes, cache operations, and cancellation.
- Include an MIT license and a minimal ignore file for local research artifacts.
- Preserve the Kairo working tree. Publish no credentials or private workloads.
- Use plain professional language without em dashes or attribution trailers.

## Unit tests

Not applicable: this change contains documentation and repository metadata only.
Future runtime tests are specified in the README roadmap.

## Integration and functional checks

- Inspect the complete staged diff against this contract.
- Check that every local Markdown link resolves and external research links are
  drawn from sources inspected during this session.
- Check for whitespace errors, em dashes, and accidental credential patterns.

## Smoke checks

- Confirm the GitHub repository is public and its default branch is `main`.
- Confirm the pushed commit matches local `HEAD` and `README.md` is retrievable.

## End-to-end checks

Not applicable: no executable application or deployment exists yet.

## Manual checks

- Read the rendered structure through the README source, including headings,
  tables, links, code fences, and the proposed state-machine diagram.
- Verify that the first experiment is bounded, reproducible, and explicitly
  insufficient to substantiate a general efficiency claim.
- Verify that GPU rental is future experimental work, not an action taken by
  this documentation change.

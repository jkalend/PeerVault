# R0 evidence bundle template

Every retained run lives at
`evidence/peervault/R0/<profile-id>/<run-id>/` and is never overwritten. The
exporter reads only sanitized inputs, writes the inventory, then verifies it.

## Required members

| Member | Source | Purpose |
|---|---|---|
| `report.json` | `evidence.json` emitted by the harness | Exact backend/profile/command record |
| `profile.json` | report profile or derived unqualified profile | Profile identity and limits |
| `profile-input.json` | exact `--profile` input, when named | Binds the selected profile bytes |
| `source-inputs.json` | report `source_inputs` | Hashes of product build inputs and fixture goldens |
| `planning-inputs.json` | `docs/planning-inputs.json` | Planning-source identities |
| `fixture-a.json`, `fixture-b.json` | checked-in goldens | Independent fixture oracles |
| `commands.md` | `docs/r0.md` | Command references and limits at export time |
| `manifest.json` | exporter | Append-only inventory and run manifest |

## Manifest fields

```json
{
  "schema_version": 1,
  "gate": "R0",
  "run_id": "<harness run id>",
  "profile_id": "<profile id>",
  "gate_status": "partial | fail | unknown",
  "baseline_result": "pass | fail",
  "inputs": {"source": "source-inputs.json", "planning": "planning-inputs.json"},
  "backend": {"backend": "restic | kopia", "backend_version": "...", "executable_sha256": "...", "backend_lock_sha256": "..."},
  "tool": {"harness_version": "...", "harness_sha256": "..."},
  "environment": {"os": "...", "arch": "...", "clock_quality": "...", "profile_sha256": "..."},
  "budgets": {"command_timeout_seconds": 300, "output_limit_bytes_per_stream": 4194304},
  "assertions": [],
  "restored_files": [],
  "faults": [],
  "topology": null,
  "failures": [],
  "untested_cases": [],
  "artifacts": [],
  "reviewer": {"status": "unreviewed", "identity": null},
  "supersedes": []
}
```

## Rules

- `gate_status` must never be `pass`; R0 closes only by the backend/profile ADR,
  not by a single report.
- Every artifact hash is recorded in `manifest.json` and re-checked by
  `scripts/evidence.py verify`.
- Missing, changed or unlisted members invalidate the bundle.
- Scored profiles require no `missing_fields`, an observed `clock_probe`, and
  the exact profile input; smoke profiles remain explicitly partial.
- Raw repositories, caches, backend config, secrets and private paths stay out of
  the registry. Hashes identify bytes, not truth or recoverability.

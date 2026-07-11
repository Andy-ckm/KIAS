# Release Process

KIAS releases must be reproducible from a reviewed commit and must not contain credentials, personal data, private organization identifiers, internal state, or unreviewed binary material.

## Release prerequisites

Before creating a tag:

1. All required checks on `main` are green.
2. Security and privacy scans have no unresolved high/critical findings.
3. `CHANGELOG.md` describes user-visible and breaking changes.
4. Version numbers and lockfiles are consistent.
5. Dependencies and licenses have been reviewed.
6. Upgrade, rollback, and data-migration implications are documented.
7. No release blocker in `docs/project-status.md` is incorrectly claimed as complete.

## Versioning

Use semantic versions:

- patch: compatible fixes and hardening;
- minor before 1.0: features and documented breaking changes;
- major after 1.0: incompatible public-contract changes.

Create an annotated, signed tag when maintainer signing is available:

```bash
git switch main
git pull --ff-only
git status --short

git tag -s vX.Y.Z -m "KIAS vX.Y.Z"
git push origin vX.Y.Z
```

The release workflow builds from the immutable tag with the lockfile, creates checksums, generates a signed build-provenance attestation, and publishes release notes.

## Verification

Download the archive and checksum file from the release page, then verify:

```bash
sha256sum --check SHA256SUMS

gh attestation verify kias-*.tar.gz \
  --repo Andy-ckm/KIAS
```

Extract and inspect the version/help output before deployment:

```bash
tar -xzf kias-*.tar.gz
./kias --help
```

## Emergency security releases

For a confirmed vulnerability:

1. coordinate privately under `SECURITY.md`;
2. prepare the smallest safe fix and regression test;
3. rotate or revoke affected credentials immediately;
4. backport only when the supported-version policy requires it;
5. publish a security advisory and patched release after users can upgrade;
6. avoid publishing exploit details before a fix is available;
7. record follow-up prevention work in the public issue tracker when safe.

## Post-release checks

- verify release artifacts and attestations from a clean environment;
- confirm generated notes and checksums;
- confirm no workflow logs or artifacts contain sensitive data;
- update documentation and examples to the released version;
- monitor reports and dependency advisories.

A release artifact is not a production-readiness certification. Deployers must review the threat model and environment-specific controls.

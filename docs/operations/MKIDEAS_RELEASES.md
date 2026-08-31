# MK Ideas Buzz internal release preparation

The fork cannot rely on upstream private build systems. The manual
`mkideas-internal-artifacts.yml` workflow is fork-owned preparation and has no
release, package-write, tag-write, or deployment permission. It is disabled
unless the repository variable `MKIDEAS_INTERNAL_RELEASES_ENABLED` is explicitly
set to `true`, and every run requires the exact confirmation phrase.

The workflow prepares short-lived artifacts only:

- unsigned Windows NSIS installer;
- unsigned macOS DMG;
- debug-signed Android APK for local/internal testing;
- no-codesign iOS application bundle;
- SHA-256 manifest and a CycloneDX source-dependency inventory.

It does not create a GitHub Release, update an app, notarize, upload to a store,
publish a container, or distribute artifacts to partners. The dependency SBOM
is source-scoped; a final release must also generate and verify a packaged-binary
SBOM and provenance attestation.

## Credentials and gates still required

- Final Windows application identifier, Authenticode certificate, timestamp
  authority policy, and private updater signing key.
- Final macOS bundle identifier, Apple Developer team, Developer ID certificate,
  notarization credentials, hardened runtime/entitlement review, and updater key.
- Final iOS bundle identifier, provisioning profile, signing certificate,
  TestFlight/App Store Connect access, privacy declarations, and push profile.
- Final Android application ID, upload/app-signing keys, Play Console internal
  track access, privacy declarations, and Firebase configuration if push is used.
- Fork-owned container registry, immutable relay/agent/push digests, vulnerability
  scan, SBOM, signatures, and provenance.
- Explicit authorization to spend runner minutes, enable the workflow, sign,
  distribute, upload, or publish anything.

Before internal distribution, verify install/upgrade/uninstall, identity and key
storage, relay URL defaults, five-area workflow, deep links, media, offline state,
revocation, crash reporting policy, artifact hashes, and signature status on the
exact candidate bytes.


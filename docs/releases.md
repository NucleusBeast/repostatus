# Building releases

The `Release builds` GitHub Actions workflow builds four archives:

- `RepoStatus-macos-arm64.zip`: signed and notarized Apple Silicon app.
- `RepoStatus-macos-x64.zip`: signed and notarized Intel Mac app.
- `RepoStatus-windows-arm64.zip`: Windows ARM64 executable.
- `RepoStatus-windows-x64.zip`: Windows x64 executable.

Windows executables are unsigned. All platforms require Git to be installed.

## Repository secrets

In GitHub, open Settings → Secrets and variables → Actions and add:

| Secret | Value |
| --- | --- |
| `MACOS_CERTIFICATE_BASE64` | Base64-encoded Developer ID Application `.p12`, including its private key |
| `MACOS_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` |
| `MACOS_SIGNING_IDENTITY` | Full `Developer ID Application: Your Name (TEAMID)` certificate name |
| `APPLE_ID` | Apple account email used for notarization |
| `APPLE_TEAM_ID` | Developer Program Team ID from Membership details |
| `APPLE_APP_PASSWORD` | App-specific password generated for that Apple account |

To copy the encoded certificate on macOS:

```sh
base64 -i "/path/to/certificate.p12" | pbcopy
```

Keep certificates and passwords out of the repository. The signing script imports
the certificate into a temporary runner keychain and deletes it after signing.
Changing the Apple account's main password revokes app-specific passwords; update
the secret if that happens.

## First test run

Push the workflow to the default branch, then open Actions → Release builds →
Run workflow. This runs tests, builds all four targets, and signs/notarizes the
Mac apps. Download the archives from the completed run's Artifacts section.
Manual runs do not create a GitHub release. Test the downloaded apps on their
respective platforms before publishing.

## Versioned release

Set the package version in `Cargo.toml` and update `Cargo.lock`, commit the
changes, then push a matching tag. For the current version:

```sh
git tag v0.1.0
git push origin v0.1.0
```

Tags must match `v` followed by the package version exactly. Once all four builds
succeed, Actions creates a draft GitHub release and attaches the archives. Review
its notes and publish it in GitHub when ready. Reruns can replace draft assets;
the workflow refuses to replace assets on an already published release.

## Notarization failures

The signing step checks for Apple's `Accepted` status and prints the notary log
when Apple rejects the submission. No Mac artifact is uploaded until the ticket
has been stapled and signature/Gatekeeper checks pass. If Apple's processing
exceeds the 45-minute wait, the job fails; inspect the submission status before
retrying.

Stapling, ticket validation, and Gatekeeper assessment each get up to five
attempts, waiting 10, 20, 30, and 40 seconds between attempts. Apple's ticket
delivery service can drop a connection even after notarization reports
`Accepted` (for example, `stapler validate` may report network error `-1005`
and exit 68). The workflow still requires every check to pass; persistent
failures stop the release. An isolated network failure does not imply that the
Intel build or signing credentials are invalid.

References: [Apple notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)
and [GitHub certificate installation](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications).

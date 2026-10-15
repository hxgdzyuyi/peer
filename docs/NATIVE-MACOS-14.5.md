# macOS 14.5 deployment target

The user installed the Apple Silicon artifact built from `6981b97406f440ab843b904605fb927691d2553b`
on macOS 14.5 and encountered the system-version gate. Both `scripts/bundle-macos.sh`
and native CI explicitly selected 15.0; the generated Info.plist also said 15.0.
That artifact is not compatible with 14.5, regardless of the successful CI build.

This correction retains the locked GPUI/Longbridge dependencies. The independent
source audit (reported by the user, also spot-checked in the locally downloaded
0.3.9 crates) found version checks around NSWindowRestorationOptions, the
pre-15 window path and 15.3 full-screen APIs. gpui-pre-apple's Metal compilation
uses a 10.15.7 target. No mandatory unguarded macOS 15 call was identified in
that review; this is not an exhaustive runtime compatibility proof.

`scripts/macos-target.env` now supplies 14.5 to Cargo/rustc, native dependency
build scripts and Info.plist. Packaging uses an isolated deployment-specific
Cargo target directory. Native CI and the manual artifact workflow restore no
compiled cache, so all Rust and native dependencies rebuild. The runner remains
macOS 15; the build host version does not determine the deployment target.

After extracting the actual ZIP, `verify-macos-target.py` checks every packaged
Mach-O image: arm64 present, minimum load-command versions at most 14.5 (the
main executable must equal 14.5), plist agreement, and strict signature verification.
It records complete load commands, image hashes and dynamic dependencies. Current
packaging permits only OS-provided `/System/Library/` and `/usr/lib/` dependencies;
unexpected embedded/rpath dependencies fail the build for explicit review. OS
framework binaries are not redistributed, and their version-specific API
availability cannot be proved by checking a newer build host's framework files.
The artifact includes `macos-target-report.json` alongside `Peer-native-macos.zip`.

Local Linux checks: shell syntax, Python compilation and load-command parser cases
for LC_BUILD_VERSION / LC_VERSION_MIN_MACOSX and rejection ordering of 15.0 > 14.5.
Actual Apple build, dependency report and extracted-signature results must come
from this correction's CI run. **Real macOS 14.5 launch and interaction remain
unverified until the user tests the rebuilt package.** No OS upgrade is required
or recommended as a workaround. No Release is published.

The pending highlighting work is deliberately excluded from this compatibility
commit. Original Library video frame-by-frame review remains blocked by the
previous 403 scope_violation; no alternate retrieval is attempted.

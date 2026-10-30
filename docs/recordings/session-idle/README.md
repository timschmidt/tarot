# Opt-in session idle reporting

October 30, 2026. Unreleased source implements authenticated-session idle
reporting behind `--session-idle`, with protected physical panel-off readiness.
The normal launcher does not enable it. The phone remains on signed Tarot57;
no package, phone preference, service policy or kernel sleep test changes in
this component. T016 and the full development plan remain incomplete.
[Contract and remaining native work](../../SESSION_IDLE.md).

## Accepted host scope

Rust 1.98.1 and minimum Rust 1.89.0 each pass 278 workspace tests, with nine
explicitly ignored. Strict owned-code host/AArch64-musl DRM Clippy and the
minimum-Rust AArch64 DRM workspace/all-target check pass. Both frozen binaries
pass the same fourteen-case private-manager matrix and the existing protected
sleep-delay replay. All 473 frozen compiler inputs and 155 QA files match the
final source ledger. Dependencies and preference schema are unchanged.

The new matrix drives the same publisher used by the compositor. It exercises
real private D-Bus calls/properties and independent gdbus reads, actual caller
PID requests, false-before-readiness, active-session changes, external property
correction, no healthy polling, late/rejected/timed-out writes, unsupported
manager actions/short delay, six invalid-session identities, owner release and
reacquire, whole-broker replacement and shutdown. The nested compositor's new
wiring remains unready: a synthetic lock supplies no physical panel-off proof.
It clears its hint on exit. No system bus, physical audio, Bluetooth, real
Suspend call or native PAM/KMS acceptance is involved.

The protected sleep-delay regression retains real private FD handoff/lifetime,
protected nested submission, ignored foreign preparation signals, blocked unlock
during preparation/rearm, busy/denied rearm, service/broker replacement and
restoration of the original app and cached clipboard. This does not represent
physical KMS or kernel sleep.

## Persistent-hint limit

A delayed acknowledgement for old readiness is discarded, and current readiness
is rechecked before a successful status is published. An already-applied hint
can still have existed briefly; this is not a lease protocol. Denial and timeout
report unknown state. Reconnect starts by explicitly clearing the previous hint
and confirming readback before considering readiness again.

One asynchronous fixture intentionally applies a timed-out true write **after
the client exits**. The test passes only when the last client status reports
unknown at shutdown, independent readback proves the persisted true value, and
a later connection clears it. Thus the green private test does not certify a
production failure/recovery contract. Native manager behavior, managed-session
teardown, late-write recovery and actual global-inhibitor enforcement remain
required before default enablement.

The compositor attempts its bounded false clear before dismantling protected
rendering and sleep coordination. Startup/reconnect false writes, session
validation and fresh property reads use the existing guarded bus transport;
no manager configuration is mutated. Private `IdleAction=suspend` is only a
fixture property; the fixture never performs an idle action or kernel sleep.

## Retained failures and artifacts

The initial private fixture exports `GetSessionByPid` through automatic method
naming. The real API requires `GetSessionByPID`. Independent introspection
reproduces the mismatch; an explicit zbus name corrects the fixture. That run
fails before any hint write and remains retained. The unchanged publisher then
passes the full matrix, followed by the frozen current/minimum acceptance runs.

An initial workspace run is denied EPERM while filling an owned private Unix
socket. A private X server also cannot bind under the sandbox. The same tests
run with permitted private sockets outside the sandbox and pass. An unavailable
`xvfb-run` command is replaced by the repository's existing owned-display helper.
All failed logs remain separate from acceptance; no test assertion is relaxed.

Ignored `artifacts/m4/session-idle/` retains the frozen compiler/QA sources,
current/minimum binaries, complete private event journals/readbacks, protected
sleep regressions, compiler/lint logs, naming proof, source manifest and
independent verifier. The public [evidence index](evidence.json) records hashes
and scope. This source has not produced or installed a new signed APK. The next
device delivery requires a new clean package revision and its native audit.

The earlier [USB kernel-control failure](../audio-kernel/README.md) remains
unresolved. Continue T016 with native idle-hint qualification while the actual
manager action stays ignore, then policy controls/packaging and genuine automatic
sleep/wake, all-UID block and power/reliability tests. Existing M0–M8 requirements,
including deferred hardware/theme work, retain their full scope.

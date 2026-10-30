# Polkit authentication implementation

October 30, 2026. M4 requires a standard Polkit authentication agent so native
Linux applications and system controls can request authorization. The Rust agent now registers for its actual local elogind session and presents
a compositor-owned authentication dialog using the shared keyboard. Private-bus
acceptance covers the real agent object, synthetic helper and rendered dialog.
Revision 52 is installed and passes actual phone enrollment and system-authority
grant acceptance. Normal Tarot remains the default, with diagnostics off.
[Exact evidence and remaining gates](recordings/polkit/README.md).

## Authority boundary

The standard agent implements BeginAuthentication and CancelAuthentication and
registers with the authority for a subject. BeginAuthentication supplies an
opaque cookie, action/message and permitted identities; it remains pending until
authentication finishes. Dismissal returns the standard Cancelled error. The
privileged helper reports the authenticated identity to the authority, which
owns the policy decision. A local “allow” button or a helper-looking string is
not sufficient evidence of a granted system authorization.
[Agent interface](https://www.freedesktop.org/software/polkit/docs/latest/eggdbus-interface-org.freedesktop.PolicyKit1.AuthenticationAgent.html),
[authority interface](https://www.freedesktop.org/software/polkit/docs/latest/eggdbus-interface-org.freedesktop.PolicyKit1.Authority.html).

The phone has Polkit 127 with elogind and the installed root-owned setuid helper
at `/usr/lib/polkit-1/polkit-agent-helper-1`. The Rust integration uses that
standard Linux component for privileged PAM work. It does not introduce another
setuid Tarot program or replace the system's PAM/authorization policy.

## Helper and presentation

`src/polkit/helper.rs` owns one child and private nonblocking pipes. Its production
entry verifies the installed helper's ownership/permissions, resolves the offered
UID to an account and uses a fixed executable path. The cookie is sent through
stdin; responses never enter argv or environment. Input buffers have zeroizing
owners, and event Debug output omits prompt/info/error text.

The conversation handles echo-off/echo-on prompts, informational/error messages,
C-escaped UTF-8 text, success and failure. Replies match a particular question;
old, duplicate, oversized or line-injecting responses cannot answer the next
question. A 510-byte response leaves room for the newline and terminator in the
Linux-PAM helper's 512-byte read. It rejects unsupported/truncated output and
success followed by an unsuccessful exit. It bounds line/output sizes, question
count and total conversation time. Cancellation/drop kills and reaps the owned
child. Child creation, NSS lookup and cleanup run on a separate authentication worker.
Only one conversation may run, including cancellation/child cleanup.
[Polkit 127 conversation](https://github.com/polkit-org/polkit/blob/127/src/polkitagent/polkitagentsession.c),
[PAM helper](https://github.com/polkit-org/polkit/blob/127/src/polkitagent/polkitagenthelper-pam.c),
[Linux-PAM response bound](https://github.com/linux-pam/linux-pam/blob/v1.7.1/libpam/include/security/_pam_types.h).

`tarot-core::authorization` keeps authority cookies out of the presentation model.
Its response owner is bounded and cannot be serialized. Request/turn identity
changes erase entry. Only identities from the current offer can be selected;
submission moves the model to Checking. Protection/dismissal retires a request,
and a late snapshot cannot reopen it or recover its input. Request Debug omits
message/info text. The shell connects this model to protection, modal focus and the shared trusted
keyboard. Each request and question changes the keyboard context and retires
held contacts. Private entry uses no TextEdit, clipboard, undo or history.
Authentication rejection clears the entry and keyboard and presents a static
error with Close; success and cancellation restore the originating app.

Real private child-pipe tests cover two questions, cookie/argv separation,
maximum-length framing, Unicode metadata, normal rejection, malformed/truncated
output, stale replies, cancellation and timeout cleanup. Pure state tests cover
identity substitution, prompt replacement and retired-request behavior. They use
synthetic responses and do not invoke the installed privileged helper or prove a
successful real Polkit grant.

## Session and caller validation

The system-bus supervisor enrolls a `unix-session` subject with the Session.Id
returned for the compositor's actual PID. It requires the current UID, a local
Wayland session, and the verified login1 exporter. Production trusts UID 0 for
login1. For Polkit it trusts UID 0 or the dedicated NSS `polkitd` account, provided
that account differs from the GUI user. The installed phone's authority actually
runs as UID 102 (`polkitd`); requiring a root exporter would reject this valid
configuration. Root ownership remains required for the installed setuid helper.

BeginAuthentication must come from the current, enrolled unique authority
exporter. Admission freshly checks the login1 owner, PID/session, UID, Id,
Active, Remote, Type and LockedHint. Locked, inactive, foreign, overlapping and
unsupported requests are rejected. Cookies stay in zeroizing backend owners;
only offered `unix-user` identities can be selected. A sole identity proceeds
automatically; multiple identities require an explicit choice.

One event-driven supervisor owns enrollment and scoped owner/session subscriptions.
Authority/login1 replacement, session replacement, inactive/locked signals and
bus loss cancel live requests. Reconnection creates a new epoch without reusing
request IDs. Lock protection cancels immediately on the compositor thread;
dismissal, helper failure and bounded deadlines also end the request. An inactive
signal retires input even if the subsequent session read already observes active
again. Cancellation cannot revoke a grant already accepted by the authority.

All D-Bus, NSS and PAM operations stay off rendering. Routine diagnostic status
contains availability, epoch, request/turn IDs, phase and echo mode, and static
errors; it omits cookies, responses, identities, action/reason and PAM text.
Protected diagnostics retain the existing lock-only redaction. A diagnostic
helper override requires an explicit private platform bus and a nonprivileged,
current-user-owned fixture; it cannot select the installed setuid helper.

## Acceptance and remaining integration

The isolated rendered fixture checks enrollment, touch-keyboard entry, rejection,
cancel/Close, multiple PAM questions, stale replies/cookies, explicit choice of
another offered account, spoofed callers, overlapping requests, rapid inactive
transitions, LockedHint, same-path session replacement, service owner replacement,
whole broker loss/restart, malformed/oversized/hung helpers, lock cancellation and
rejection while protected. A synthetic grant requires a separate fixture helper
response to its fake authority; an agent void reply alone never grants it. No
host privileged helper or real password is used. These cases do not establish a
real system authorization.

The ordinary-user native requester ran in Tarot's actual graphical PAM cgroup.
Its narrowly scoped temporary `auth_self` action had no privileged mechanism and
no retained grant. The real system authority granted the correct password and a
fresh request after locking. It denied the wrong password, UI Cancel, requester
cancellation, killed helper, unsubmitted-input lock, a request while locked, and
locking after a valid response was queued to a stopped helper. Actual credentials
went only through the rendered keyboard, guarded by process/request/turn,
echo-off prompt, unlocked modal and portrait presentation checks.

The test removed its policy and verified the action disappeared from authority
enumeration. The diagnostic PAM cgroup, input device and control socket were
removed; exact preferences, platform policies, audio and backlight were restored.
A separate audit verifies installed/loaded revision-52 hashes, normal default
Tarot, actual session/UID, Panfrost, services and diagnostics off. The package adds
`polkit-elogind` as an explicit runtime dependency; the pre-existing service,
helper and policy packages were unchanged. Native daemon restart and broader
policy behavior remain unverified. Interactive NetworkManager callers still
need separate integration and acceptance before enabling auth-required controls.

Current Rust 1.98.1 and minimum 1.89.0 each pass all 251 workspace tests,
including private helper-process, pure request-state, modal focus/retirement
and dedicated daemon-principal cases. Strict host and aarch64-musl/DRM lint
pass, along with the shared Wi-Fi credential and Bluetooth regressions. These checks are recorded separately from
private rendered acceptance and native acceptance. Cross-built package creation
alone does not run target unit tests or prove real authority grants.

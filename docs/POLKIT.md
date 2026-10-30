# Polkit authentication implementation

October 30, 2026. M4 requires a standard Polkit authentication agent so native
Linux applications and system controls can request authorization. The full
agent remains unfinished. The current code implements a tested helper
conversation and presentation-state foundation; it does not register an agent,
open a shell dialog or grant a real authorization. Installed phone revision 51
is unchanged by this work.

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
at `/usr/lib/polkit-1/polkit-agent-helper-1`. The Rust integration will use that
standard Linux component for privileged PAM work. It does not introduce another
setuid Tarot program or replace the system's PAM/authorization policy.

## Implemented foundation

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
child. Child creation, NSS lookup and cleanup belong on an authentication worker,
not the compositor thread.
[Polkit 127 conversation](https://github.com/polkit-org/polkit/blob/127/src/polkitagent/polkitagentsession.c),
[PAM helper](https://github.com/polkit-org/polkit/blob/127/src/polkitagent/polkitagenthelper-pam.c),
[Linux-PAM response bound](https://github.com/linux-pam/linux-pam/blob/v1.7.1/libpam/include/security/_pam_types.h).

`tarot-core::authorization` keeps authority cookies out of the presentation model.
Its response owner is bounded and cannot be serialized. Request/turn identity
changes erase entry. Only identities from the current offer can be selected;
submission moves the model to Checking. Protection/dismissal retires a request,
and a late snapshot cannot reopen it or recover its input. Request Debug omits
message/info text. This model still needs to be connected to compositor
protection, the trusted keyboard and the actual authority.

Real private child-pipe tests cover two questions, cookie/argv separation,
maximum-length framing, Unicode metadata, normal rejection, malformed/truncated
output, stale replies, cancellation and timeout cleanup. Pure state tests cover
identity substitution, prompt replacement and retired-request behavior. They use
synthetic responses and do not invoke the installed privileged helper or prove a
successful real Polkit grant.

## Remaining integration and acceptance

1. Enroll the compositor's actual active local elogind session with the current
   root-owned authority. Validate the calling exporter and session on admission;
   handle authority/login1 loss and new epochs without reusing request tokens.
   Reject unsolicited/spoofed and overlapping requests.
2. Run the helper on a bounded worker. Bind identity choice, every conversation
   turn, completion and cancellation to that live authority request. Cancel on
   protection, dismissal, deadline and service/session loss. Distinguish ending
   an unfinished request from revoking a grant already accepted by the authority.
3. Present a compositor-owned authentication modal with action/reason/identity,
   the shared trusted keyboard, masked private entry and explicit cancellation.
   Preserve the originating app while excluding its input; retire held contacts
   and fields when the request/turn changes. Routine status must omit cookies,
   responses and PAM conversation text.
4. Exercise real private D-Bus fixtures for caller validation, enrollment,
   responses and service/subject replacement, plus rendered typing/cancellation
   and protected-frame cases. A fixture's synthetic SUCCESS cannot establish
   authorization by the real system authority.
5. Verify an ordinary-user real authorization challenge on the phone, with a
   narrowly scoped temporary test action that has no privileged mechanism.
   Require actual authority results for correct/wrong credentials, cancellation,
   lock, service/helper failures and cleanup. Remove the test action and temporary
   resources, then independently verify normal Tarot and unchanged policies.
   Only after that evidence should interactive platform callers be enabled.

Validation checkpoint: current Rust 1.98.1 and minimum Rust 1.89.0 each pass all
249 workspace tests, including five private helper-process cases and three
request-state cases. Strict host and aarch64-musl/DRM workspace lint pass. There
is no new dependency or package revision. Real authority enrollment, credential
acceptance and shell-dialog behavior are not covered by these checks.

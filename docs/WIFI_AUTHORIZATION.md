# Wi-Fi authorization

October 30, 2026. Explicit Wi-Fi actions can now request authentication through
Tarot's session-bound Polkit agent. NetworkManager still decides whether each
operation is authorized. This extends the revision-52 agent; it does not grant
permissions in the shell or complete M4.

## Permission and operation boundaries

NetworkManager's `yes`, `auth` and `no` permissions remain distinct. `Yes` can
request an operation directly. `Authentication` enables an explicit control only
while Tarot has a verified active enrollment and the session is unprotected.
`No` stays denied. The agent's readiness flag is an admission hint, never proof
of a grant. The backend refreshes inventory, checks opaque row/profile tokens and
the unique NetworkManager owner, then sends the operation to that mechanism.
Background reads do not request authentication or retrieve stored secrets.
[NetworkManager API](https://www.networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.html).

One bounded write worker can wait for human authentication while a separate
short-timeout connection continues inventory, owner and secret-agent updates.
The write connection has a 400-second bound: new-profile activation may require
successive network-control and profile-creation conversations, each bounded to
180 seconds by the agent. State reads retain their five-second timeout. Both
connections must belong to the same broker. Shutdown closes both connections.
An uncertain write is never retried automatically against a replacement daemon;
admission stays closed until the old worker has actually ended.

A successful method reply starts a separate state-observation deadline.
Scanning completes only when `LastScan` advances; connection, disconnection and
radio actions must reach their corresponding observed states. The method reply
alone cannot mark a scan or connection complete.
[Wireless API](https://www.networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Device.Wireless.html).

The installed NetworkManager version is 1.52.2. Its scan handler explicitly allows
interactive authorization. Its add-and-activate path authorizes network control
before asking settings to create the profile. These paths were checked against
the matching upstream source:
[scan](https://github.com/NetworkManager/NetworkManager/blob/1.52.2/src/core/devices/wifi/nm-device-wifi.c),
[activation](https://github.com/NetworkManager/NetworkManager/blob/1.52.2/src/core/nm-manager.c),
[authority call](https://github.com/NetworkManager/NetworkManager/blob/1.52.2/src/core/nm-auth-manager.c).

## Returning from authentication

A settings-originated prompt retains only its page, entry and navigation history.
It cancels previous private input and restores the settings page with a fresh
menu revision after success, cancellation or closing a static failure. Native
requesters retain their existing return behavior. Home, launcher navigation or
locking discards the saved destination. Old requests, turns, credentials and held
contacts cannot be reused when a second authorization question appears.

Agent readiness is revoked immediately on protection, session deactivation,
service loss and shutdown. Registration in an inactive session does not enable
authentication-required controls. Existing NetworkManager secret-agent replies
remain separate from new-operation admission and can answer an already pending
connection request.

## Executed host acceptance

Current Rust 1.98.1 and minimum Rust 1.89.0 each pass 256 workspace tests, with
nine explicitly ignored tests. Strict host and aarch64-musl/DRM lint pass for
Tarot's workspace. Both frozen compiler builds pass the rendered integration:

- An explicit scan remains pending beyond the previous five-second timeout;
  inventory updates continue while its password dialog is open.
- Actual shared touch-keyboard entry restores Wi-Fi settings. A method reply
  still waits for the fixture's separate scan-completion observation.
- Wrong responses, Cancel, Home and locking deny the operation and retire input.
- Radio, disconnect, saved activation and temporary personal-profile creation
  follow mechanism decisions. New creation handles two successive prompts;
  the first response cannot answer the second.
- A live agent cannot upgrade `No`; inactive enrollment cannot request `auth`;
  `Yes` works without an active agent.
- NetworkManager, authority and whole-broker replacement retire old requests and
  identities, recover without restarting Tarot, and require a fresh action.
- Exiting with an unanswered authorization completes within the six-second
  shutdown limit (under 0.03 seconds in both accepted rendered runs).

The existing private Wi-Fi credential and rendered authentication-agent suites
also pass. The fake authority grants only after a separate synthetic helper
response; the agent's void reply cannot grant anything. These tests use an owned
private broker, Xvfb and a nonprivileged helper. They neither invoke the host's
setuid helper nor establish a real system authorization or hardware radio result.

Reproduce after building `tarot`, `network-probe` and `lock-probe`:

```sh
python3 tests/network_authorization_replay.py \
  --output artifacts/network-authorization-private
```

Use `--xvfb /path/to/Xvfb` and `--binary /path/to/tarot` as needed. The example
fixtures must be in the binary's adjacent `examples` directory. Every run needs
a new output directory. Reports preserve source/binary hashes and failures.
Local evidence is under ignored `artifacts/m4/network-authorization/`.

## Native acceptance and remaining gates

Signed revision 53 passes real RequestScan/PAM acceptance on the PinePhone Pro.
A temporary rule required nonretained `auth_self` only for the independently
verified GUI PID, user and active local session. It added no grant. Correct
passwords, including a fresh request after unlocking, completed real scans.
Wrong-password entry, Cancel and locking returned mechanism errors. Credentials
went only through the guarded rendered keyboard and installed standard helper.

The rule was removed and original GUI permissions returned. Exact preferences,
policies, profile files, active connection, audio controls and backlight matched
the baseline. NetworkManager and the system bus were not restarted. A separate
audit verifies normal default Tarot53, diagnostics off, actual session/UID,
loaded payloads, Panfrost and cleanup. All 650 archive members and 614 frozen
inputs verify locally. [Evidence and artifacts](recordings/network-authorization/README.md).
The test's use of subject fields and
`AUTH_SELF` follows the
[Polkit rules API](https://polkit.pages.freedesktop.org/polkit/polkit.8.html) and
[version-127 subject construction](https://github.com/polkit-org/polkit/blob/127/src/polkitbackend/polkitbackendduktapeauthority.c).

Native daemon/broker restart, authorization of radio/profile changes,
other authorization policies, hidden/enterprise/VPN setup, profile
management and release reliability remain open. Physical Bluetooth tests remain
deferred at the owner's request.

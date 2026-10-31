# Wi-Fi adapter idle cost

October 31, 2026. Measuring what an idle Waydroid container costs found a
larger, constant cost in Tarot itself. Revision 71 removes it.

## The problem

With the screen locked and dark and nothing happening, polkitd used about 74
CPU ticks a minute (about 1.2% of one core), with dbus-daemon and
NetworkManager close behind. A ten-second capture of calls to Polkit and
NetworkManager showed the source:

| Caller | Calls in 10 s |
| --- | --- |
| NetworkManager → Polkit `CheckAuthorization` | 68 |
| Tarot → `Properties.GetAll` | 36 |
| Tarot → `GetPermissions`, `GetDevices`, `GetAllAccessPoints`, `GetSettings` | 4 each |

Tarot's Wi-Fi adapter reread its whole inventory on every NetworkManager
signal, including signal-strength and statistics updates, about every 2.5 s.
Each reread called `GetPermissions`, which makes NetworkManager run around 17
Polkit checks. `Server::watch()` recorded whether the Wi-Fi page was open, but
the worker never read that flag. Waydroid's bridge and veth devices made each
reread larger, which is why the cost doubled with Android frozen.

## The change

- **Coalescing.** With the Wi-Fi page closed and no call, operation, secret
  prompt or retry in flight, signals within 10 s of the last reread are
  coalesced into one. Opening the page, an action or a stop ends the wait at
  once; nothing waits while the page is open.
- **Cached permissions.** `GetPermissions` runs on the first read, after the
  daemon is replaced, when the Wi-Fi page opens, and when NetworkManager emits
  `CheckPermissions` (which it does after a Polkit authority change). The
  private-bus NetworkManager fixture now emits `CheckPermissions` when its
  permission changes, as the real daemon does.

The private-bus network, credentials and authorization replays pass, including
their permission-change cases.

## On the phone (signed revision 71)

Package `2034116c3862baf726e7af5377b5258b48261a99a56876cb4ff1518366a76641`,
compositor `81e7220107c5a0caa78f2992aea4710b3d5a69d6d0687c0fc9131fb01794ac85`,
installed with a normal-session restart.

- Ten-second capture: one inventory reread, no `GetPermissions` and no Polkit
  checks (revision 70: four rereads and 68 checks).
- Sixty-second profile with the Waydroid container frozen and the screen dark:

| Ticks per minute, or interrupts per second | r70 | r71 |
| --- | --- | --- |
| polkitd | 143 | not among busy processes |
| dbus-daemon | 53 | 13 |
| NetworkManager | 49 | 16 |
| Tarot | 15 | 10 |
| Timer interrupts/s | 161 | 82 |
| Function-call IPIs/s | 112 | 65 |

A revision 71 sample with Waydroid stopped was taken right after login, with
the screen on and the orientation sensor active, and is not comparable.

## Limits

- CPU ticks and interrupts are proxies. The phone was on USB power with a full
  battery, so no energy figure is claimed; an unplugged measurement remains.
- The status bar's Wi-Fi state can lag by up to 10 s while the Wi-Fi page is
  closed.

Evidence: `artifacts/m6a/idle-cost/` and `artifacts/m4/network-idle/`.

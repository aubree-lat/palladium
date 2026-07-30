# Palladium

A Discord client that runs on the **system webview via Tauri** instead of bundling
Electron, with your choice of **Vencord** or **Equicord** as the client mod and a
**native Rust implementation of arRPC** for Discord Rich Presence.

The whole app is **5.9 MB**. For comparison, Equibop on the same machine occupies
314 MB — 103 MB of which is a bundled Node runtime purely to run arRPC.

> [!IMPORTANT]
> **Voice needs a patched WebKitGTK.** Every distribution ships WebKitGTK with
> `ENABLE_WEB_RTC` off, so `RTCPeerConnection` does not exist at all and Discord
> refuses voice outright. This is not a user-agent problem and cannot be fixed from
> inside the app.
>
> ```bash
> npm run setup:librice   # ICE backend (unpackaged on Arch, so it is built)
> npm run setup:webrtc    # WebKitGTK with WebRTC, into ~/.local/opt
> npm run start:webrtc    # run Palladium against it
> ```
>
> Both build into a private prefix loaded by path, so the system libraries are
> untouched and no root is needed. Budget a few hours for the WebKit compile.
>
> **This gets you the full WebRTC API and Opus, and SDP negotiation completes —
> but Discord voice still does not connect.** ICE gathers only `host` candidates
> and never sends a connectivity check, so it stalls at "Checking route". That is
> an upstream WebKitGTK/ICE problem, not something Palladium can reach. See
> [Why Discord voice still fails](#why-discord-voice-still-fails).

> [!WARNING]
> This is a hobby project and still rough in places. It logs in, chats, uploads
> files, themes both itself and Discord, and drives Rich Presence — but it is not
> as polished as [Vesktop](https://github.com/Vencord/Vesktop) or
> [Equibop](https://github.com/Equicord/Equibop).

MIT licensed — do whatever you like with it.

---

## Why this works

The interesting constraint is that Tauri has no Electron preload script, so the
usual Vesktop/Equibop injection strategy is unavailable. The way around it:

Both Vencord and Equicord publish **two** builds on their release pages.

| Build | Files | Needs Electron? |
|---|---|---|
| Desktop | `renderer.js`, `preload.js`, `patcher.js` | **Yes** — `renderer.js` talks to a `VencordNative` bridge that `preload.js` exposes over Electron IPC |
| Browser | `browser.js`, `browser.css` | **No** — it defines its own `window.VencordNative`, backed by `localStorage` and IndexedDB |

Palladium uses the **browser build**. It is fully self-contained, so it only needs
to be evaluated before Discord's own bundle runs — which is exactly what Tauri's
`initialization_script` does. No preload, no asar, no patching.

```
┌─────────────────────────────────────────┐
│ Tauri (Rust)                            │
│                                         │
│  mods.rs ──► downloads browser.js/css   │
│              (ETag-cached)              │
│                 │                       │
│  inject.rs ─────┴──► initialization_    │
│                      script             │
│                         │               │
│  arrpc/ ──► discord-ipc-N (unix socket) │
│         └─► ws://127.0.0.1:1337         │
└─────────────────────────┼───────────────┘
                          ▼
              ┌───────────────────────┐
              │ WebKitGTK / WebView2  │
              │  discord.com          │
              │  + Vencord/Equicord   │
              └───────────────────────┘
```

## arRPC, in Rust

Rich Presence only works because Discord's official client listens on a local IPC
socket. Third-party clients have to provide that socket themselves.
[arRPC](https://github.com/OpenAsar/arrpc) does this in Node; Palladium
reimplements it in Rust so there is no bundled runtime and no sidecar to babysit.

Two halves, in `src-tauri/src/arrpc/`:

- **`ipc.rs`** serves `discord-ipc-0` … `discord-ipc-9` (unix socket, or a named
  pipe on Windows) and speaks Discord's framed protocol:
  `[i32le type][i32le length][JSON]`. It claims the lowest slot nobody else is
  serving, so it coexists with a real Discord install rather than fighting it.
- **`bridge.rs`** rebroadcasts activity over a websocket on port 1337 — precisely
  what the stock **arRPC plugin already shipped in both Vencord and Equicord**
  connects to. No patched plugin required; just enable *WebRichPresence (arRPC)*
  in your mod's settings.

Activity translation matches upstream: seconds-vs-milliseconds timestamp
normalisation, and splitting `buttons` into labels on the activity plus URLs in
`metadata.button_urls`.

The bridge port is claimed **before** the IPC listener starts. If something else
already holds 1337 it is almost certainly another arRPC (Vesktop, Equibop and the
Discord flatpak all ship one), so Palladium logs a clear warning and steps aside
rather than competing for game connections.

Upstream's third transport — **process scanning**, which guesses at running games
by name — is deliberately not implemented. It is the least reliable part of arRPC
and needs a maintained game database to be worth anything.

## Making WebKitGTK behave

wry hands you a fairly bare WebKitWebView, and several defaults have to be changed
before it works as a Discord client. All of this lives in `src-tauri/src/webkit.rs`.

| Problem | Cause | Fix |
|---|---|---|
| "Unsupported Browser", no voice UI | `enable-media-stream` and `enable-webrtc` default to **off**, so `navigator.mediaDevices` does not exist | Enable them on the native `WebKitSettings` — and *before* the page loads, since WebKit installs JS globals at context creation. The webview is therefore created on `about:blank`, tuned, then navigated. |
| Mic prompt never resolves, empty device lists | Unhandled `permission-request` signals are denied | Grant `UserMedia`, `DeviceInfo` and `Notification`; deny geolocation and pointer lock |
| "Upload a File" does nothing | wry never handles `run-file-chooser` | Present a `GtkFileChooserNative`, honouring the input's `accept` filter |
| Dragging a file onto chat does nothing | Tauri's drag-drop handler swallows the drop before the page sees it | `disable_drag_drop_handler()` |

WebKit also hides some APIs behind runtime *feature flags*, reachable only through
`webkit_settings_set_feature_enabled`, which the Rust bindings do not wrap — so
`webkit.rs` declares that C API by hand and enables the WebRTC-related flags.

## Startup

A small splash window is shown while the mod bundle is fetched, then the Discord
window is created and the splash closes. Discord itself then takes several seconds
to boot; the window is painted `#0d001a` rather than black while that happens.

The branch URLs point at `/channels/@me` rather than `/app`. `/app` bounces to it
as a *fresh document load*, which parsed the mod bundle twice and doubled startup
time; going direct halves it.

## Settings

Settings live in their own window, opened from the tray (**palladium settings**).
It covers the client mod, theme, Discord theming, arRPC, branch, window backend,
mod caching, tray behaviour, importing from other clients, and a couple of
maintenance actions.

An earlier version injected a section into Discord's own settings sidebar instead.
It connected fine but the sidebar entry never reliably appeared — matching
Discord's content-hashed class names to find the insertion point is guesswork that
breaks between client builds. A local window is both more dependable and, because
it removed the need for remote IPC, materially safer.

On a first launch a dedicated setup window asks which client mod to use rather
than guessing. The tray is otherwise navigation only: open discord, palladium
settings, reload, quit.

## Running it

Requires Rust, Node, and the Tauri v2 system dependencies (`webkit2gtk-4.1`,
`libsoup-3.0`, `gtk3` on Linux).

```bash
npm install
npm run dev      # or: cd src-tauri && cargo run
npm run build    # deb / rpm / AppImage
```

| Variable | Effect |
|---|---|
| `RUST_LOG=palladium=debug` | Verbose logging, including document title changes |
| `PALLADIUM_URL=<url>` | Open a different URL instead of Discord (debugging) |
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | Workaround for blank windows on some Linux GPU drivers |

## Building for Windows from Linux

```bash
npm run setup:windows   # one-time; installs the cross toolchain
npm run build:windows
```

Produces:

| Artifact | Size |
|---|---|
| `src-tauri/target/x86_64-pc-windows-msvc/release/palladium.exe` | ~5.9 MB (PE32+ GUI) |
| `.../bundle/nsis/Palladium_0.1.0_x64-setup.exe` | ~2.2 MB |

`scripts/setup-windows-cross.sh` installs `cargo-xwin` (which fetches the MSVC CRT
and Windows SDK) plus the Rust target. `ring`, pulled in by rustls, compiles C, so
a plain `cargo check --target x86_64-pc-windows-msvc` fails without a real MSVC
toolchain — cargo-xwin is what supplies `lib.exe` and friends.

Bundling the NSIS installer needs three workarounds, all handled by the setup
script. Tauri warns that cross-platform bundling is experimental, and this is what
that means in practice:

1. **NSIS is not in Arch's repos** and Tauri only downloads its own plugin, so the
   script fetches the official NSIS distribution into `~/.cache/tauri/NSIS`.
2. **Tauri resolves `makensis.exe` through `PATH`** on a non-Windows host, and
   Linux cannot exec a PE binary. A shell shim named `makensis.exe` stands in and
   forwards to wine. It targets `Bin/makensis.exe` directly, because the stock root
   `makensis.exe` is only a launcher stub that derives its child from its own
   filename — renaming it breaks it.
3. **Tauri writes host-native Unix paths into the generated `installer.nsi`**,
   which wine cannot resolve, and NSIS reads a leading `/` as a switch. The shim
   rewrites quoted absolute paths onto wine's `Z:` drive and converts path
   arguments with `winepath`.

The installer is **unsigned** — signing only works from a Windows host unless you
configure `bundler > windows > sign_command`. SmartScreen will warn on first run.

The app needs the **WebView2 runtime**, which ships with Windows 11 and current
Windows 10. The installer bundles `NSISdl.dll` to fetch the bootstrapper if it is
missing; the bare `.exe` does not, so prefer the installer on older machines.

> Cross-compiled binaries have not been run on real Windows. The named-pipe arRPC
> transport compiles for `x86_64-pc-windows-msvc` but is untested at runtime. For
> release builds, a `windows-latest` CI runner remains the trustworthy option.

## Configuration

Settings live in the panel above; the file is just where they land.
`~/.config/palladium/config.json`:

```json
{
  "client_mod": "equicord",      // "vencord" | "equicord" | "none"
  "arrpc_enabled": true,
  "arrpc_bridge_port": 1337,     // the stock plugin hardcodes 1337
  "always_update_mod": false,    // ignore the ETag cache and refetch each launch
  "minimize_to_tray": true,
  "discord_branch": "stable",    // "stable" | "ptb" | "canary"
  "linux_backend": "auto",       // "auto" | "wayland" | "x11"
  "theme": "palladium",          // see Themes
  "theme_discord": false,        // also restyle Discord itself
  "zoom": 1.0
}
```

Settings from an older install under `~/.config/tauricord` are migrated
automatically on first launch, as is the webview profile — renaming the app changed
its bundle identifier, which would otherwise have orphaned the Discord login and
the client mod's localStorage.

Switching the client mod rebuilds the Discord window in place — the injection
script is fixed at webview creation, so a plain reload is not enough.

Mod bundles are cached in `~/.local/share/palladium/mods/` and refreshed with HTTP
ETags, so a warm launch costs one conditional request per asset and no GitHub API
quota. If the network is unavailable, the cached copy is used rather than failing.

## Security notes

- **The Discord window is granted no Tauri IPC at all.** There is no remote
  capability; `src-tauri/capabilities/default.json` is scoped to Palladium's own
  local windows (splash, setup, settings). Commands are declared in `build.rs` via
  `AppManifest`, so each is an explicit, auditable grant.
- Settings used to be injected into Discord's own sidebar, which needed a remote
  IPC grant. Moving them to a local window removed that need entirely, so the
  remote capability was deleted rather than left lying around.
- Everything the page still needs from the host goes through the token-guarded
  localhost endpoint instead, which exposes exactly three narrow routes and cannot
  reach the command surface.
- Mod CSS is embedded via `serde_json` string encoding, so stylesheet content
  cannot break out into script.
- Invite codes arriving over RPC come from untrusted local processes and are
  whitelist-validated before ever reaching a `location.assign`.
- The RPC `READY` payload reports arRPC's stand-in user rather than the real
  logged-in account — the RPC socket has no business handing out your identity.

## Voice support

Out of the box, voice does not work — and it is **not** a user-agent problem.
Palladium presents a full Chrome user agent, verified against both the HTTP
request header and `navigator.userAgent`, exactly as Vesktop does.

The real cause, found by probing a bare `WebKit2.WebView` with `enable-webrtc`
set in a secure context (`scripts/webkit-probe.py`):

```
RTCPeerConnection=undefined  RTCRtpSender=undefined  RTCDataChannel=undefined
mediaDevices=object          isSecureContext=true
```

`mediaDevices` is present, so the settings applied — but every WebRTC API is
missing. Reading WebKit's own build config explains why:

```cmake
# Source/cmake/OptionsGTK.cmake
WEBKIT_OPTION_DEFAULT_PORT_VALUE(ENABLE_MEDIA_STREAM PRIVATE ON)
WEBKIT_OPTION_DEFAULT_PORT_VALUE(ENABLE_WEB_RTC     PRIVATE ${ENABLE_EXPERIMENTAL_FEATURES})
```

`MEDIA_STREAM` is on by default, which is why `getUserMedia` works. `WEB_RTC` is
gated behind experimental features, which are **off** in release builds — so no
distribution ships it. This affects **any** wry- or Tauri-based client on Linux,
not just this one; they all share the engine.

### Fixing it

```bash
npm run setup:webrtc
```

`scripts/build-webkit-webrtc.sh` builds WebKitGTK 2.52.5 with
`-D ENABLE_WEB_RTC=ON`, mirroring Arch's PKGBUILD flags otherwise. Two details
matter:

- **`-D USE_LIBRICE=OFF`.** The GTK port forces `USE_LIBRICE=ON`
  (`OptionsGTK.cmake:150`), pulling in a Rust ICE library that is packaged in
  neither Arch's repos nor the AUR. Turning it off routes ICE through GStreamer's
  libnice agent (`libgstwebrtcnice`), which distributions do ship.
- **A private prefix** (`~/.local/opt/webkit-webrtc`), loaded via
  `LD_LIBRARY_PATH`. Replacing the system `libwebkit2gtk-4.1` would put every
  dependent package at risk — `lutris`, `evolution-data-server`, `gnome-boxes`,
  `cloudflare-warp-bin`. Nothing system-wide is modified, and no `sudo` is needed
  to install.

Needs `gperf unifdef ruby ruby-stdlib` plus cmake/ninja/lld, roughly 20 GB of
disk, and 1–3 hours of compile. The script verifies `ENABLE_WEB_RTC` actually
stuck in `CMakeCache.txt` and aborts if not, so a bad flag fails in seconds rather
than after the full build.

### The result

Same probe, same WebKit version, only the library path differs:

| | distro build | patched build |
|---|---|---|
| `RTCPeerConnection` | `undefined` | **`function`** |
| `RTCRtpSender` | `undefined` | **`function`** |
| `RTCDataChannel` | `undefined` | **`function`** |
| Opus advertised | error | **`true`** |

Opus matters specifically because it is the codec Discord uses for voice. Inside
the Tauri webview the runtime feature flags go from **1 flipped to 8**, since
`webkit.rs` finally has WebRTC flags available to enable.

Launch with `npm run start:webrtc`.

### librice is required, not optional

If WebKit is built with `-D USE_LIBRICE=OFF`, Discord reaches "Checking route" and
then loops disconnect/reconnect forever, with the log repeating:

```
libnice-WARNING: Could not find component 1 in stream 1
```

WebKitGTK 2.52 has two ICE implementations, and the GTK port **defaults to librice**:

```cmake
# Source/cmake/GStreamerDefinitions.cmake — global default
WEBKIT_OPTION_DEFINE(USE_LIBRICE "Whether to enable support for librice" PRIVATE OFF)
# Source/cmake/OptionsGTK.cmake:150 — but the GTK port opts in
WEBKIT_OPTION_DEFAULT_PORT_VALUE(USE_LIBRICE PRIVATE ON)
```

Turning it off to dodge an unpackaged dependency falls back to the legacy libnice
path, which fails. Media, codecs and the whole API surface still work — only route
negotiation breaks, which is exactly what "Checking route" is.

[librice](https://github.com/ystreet/librice) is a sans-IO ICE implementation in
Rust. It is packaged in neither Arch's repos nor the AUR, so it has to be built,
which is what `setup:librice` does — it exposes the `rice-io` and `rice-proto`
pkg-config modules WebKit looks for, via
[cargo-c](https://github.com/lu-zero/cargo-c):

```bash
npm run setup:librice   # cargo-c + librice into ~/.local/opt/webkit-webrtc
npm run setup:webrtc    # rebuild WebKitGTK; USE_LIBRICE=ON is the default
npm run start:webrtc
```

It installs into the **same prefix** as the patched WebKitGTK, so one
`LD_LIBRARY_PATH` covers everything at runtime, and needs no root.
`build-webkit-webrtc.sh` refuses to start if pkg-config cannot see the two
modules, rather than silently producing another build with broken voice.

To confirm the swap took, the rebuilt library should reference librice and not
libnice at all:

```bash
L=~/.local/opt/webkit-webrtc/lib/libwebkit2gtk-4.1.so.0.21.9
strings -a "$L" | grep -c rice_agent      # non-zero
strings -a "$L" | grep -c nice_agent_new  # 0
LD_LIBRARY_PATH=~/.local/opt/webkit-webrtc/lib ldd "$L" | grep rice
```

### Why Discord voice still fails

Everything up to the final connectivity step works. Discord creates an offer, sets
a local description (`signalingState => have-local-offer`), sends the SDP to its
RTC server, and applies the answer. The break is ICE, and it is reproducible
without Discord at all using `scripts/ice-probe.py`, which builds a plain
`RTCPeerConnection` against a public STUN server:

```
GATHER_COMPLETE total=24 {"host":24}
first=candidate:0 1 UDP 2128617727 192.168.1.250 36908 typ host
```

24 candidates, **every one of them `host`**, and no `srflx` despite a STUN server
being configured. Meanwhile a plain UDP STUN binding request from the same machine
succeeds in 5ms and correctly returns the public address. So UDP works, the network
works, and the STUN server answers — **librice simply never performs the STUN
transaction**, and never sends connectivity checks. ICE therefore reaches
`checking` and stays there until Discord times out and retries.

Both ICE backends available in WebKitGTK 2.52 are broken here, differently:

| backend | failure |
|---|---|
| `USE_LIBRICE=OFF` (libnice) | repeating `Could not find component 1 in stream 1` |
| `USE_LIBRICE=ON` (librice) | host candidates only, no STUN, no connectivity checks |

That makes voice an upstream problem, not an application one. Nothing in Palladium
sits between the ICE agent and the network.

One genuine application-level bug *was* found and fixed along the way:
`RTCRtpSender.setParameters` is unimplemented in WebKitGTK and threw
`NotSupportedError`, killing Discord's voice setup before negotiation
(`UnifiedConnection.setTransceiverEncodingProperty: setParameters failed`).
`src-tauri/src/voice_shim.js` catches that, strips the properties WebKit cannot
apply (`maxBitrate`, `scaleResolutionDownBy`, `maxFramerate`) and resolves instead
of rejecting. That took the error count to zero and let negotiation complete — it
just is not sufficient while ICE cannot check.

#### Diagnosing it yourself

```bash
# does this WebKit gather anything beyond host candidates?
GDK_BACKEND=x11 LD_LIBRARY_PATH=~/.local/opt/webkit-webrtc/lib \
  GI_TYPELIB_PATH=~/.local/opt/webkit-webrtc/lib/girepository-1.0 \
  python3 scripts/ice-probe.py

# watch Discord's own console and every request/status code
PALLADIUM_CONSOLE=1 PALLADIUM_NETLOG=1 npm run start:webrtc
```

`PALLADIUM_CONSOLE=1` pipes the page console (including Discord's own errors) to
stdout, and `PALLADIUM_NETLOG=1` logs every resource load with its status code.
Both were what turned this from guesswork into a diagnosis.

### Optional voice-quality plugins

With librice in place WebKit asks for two GStreamer elements that Arch splits out
of `gst-plugins-rs`:

```
GStreamer element audiornnoise not found. Please install it
gst-plugins-rs is not installed, RTP bandwidth estimation now disabled
```

```bash
sudo pacman -S --needed gst-plugin-rsaudiofx gst-plugin-rsrtp
```

`rsaudiofx` provides `audiornnoise` (RNNoise suppression, what Discord's noise
suppression toggle drives) and `rsrtp` provides `rtpgccbwe` (congestion control, so
bitrate adapts instead of being fixed). Neither is required to connect — voice
works without them, just without noise suppression or adaptive bitrate.

> [!NOTE]
> WebKitGTK's GStreamer WebRTC backend is upstream-*experimental*. The APIs being
> present is necessary but not sufficient — expect rougher edges than Chromium,
> particularly around screenshare.

## Themes

Seven themes, selectable in settings, applied instantly:

| | |
|---|---|
| `palladium` | deep purple gradient (default) |
| `oxygen` | white and grey, black text |
| `hydrogen` | black and grey, white text |
| `plutonium` | navy and blue, white text |
| `uranium` | green, white text |
| `iron` | dark grey and black, white text |
| `gallium` | pitch black AMOLED, white outlines and text |

They are plain CSS custom property sets in `src/theme.css`, keyed off
`:root[data-theme="..."]`, so adding one is a single block.

**"apply to discord"** is an optional toggle that restyles Discord itself to
match, by overriding its colour tokens (`--background-primary`, `--text-normal`,
`--bg-base-primary`, the `--brand-*` family and so on) from `src-tauri/src/theme.rs`.
It is injected as a constructed stylesheet so it does not touch your QuickCSS, and
it re-applies live without a reload. Gallium additionally draws white outlines on
panels for the AMOLED look.

Discord's token names shift between client builds, so this is best-effort — if a
surface stays unthemed, its token needs adding to `theme.rs`.

## Importing from Vesktop or Equibop

Offered on first run and from settings. Reads:

- `~/.config/{vesktop,equibop}/settings.json` — branch, tray behaviour, arRPC
  (handling both the `arRPC` and inverted `arRPCDisabled` spellings)
- `~/.config/{vesktop,equibop}/settings/settings.json` — the Vencord/Equicord
  config, seeded into `localStorage.VencordSettings` before the mod loads, so
  enabled plugins and themes carry across
- `settings/quickCss.css` if present

The seed is written to disk and consumed on the next launch, because the settings
have to be in localStorage *before* the mod boots.

## The local endpoint

Discord's CSP blocks a lot of what a client needs, but its `connect-src` allows
`http://127.0.0.1:*` — that is how the real client talks to its own RPC. Palladium
uses that same door for three things, served by one token-guarded HTTP server on a
random port (`src-tauri/src/proxy.rs`):

| route | why |
|---|---|
| `?url=…` | forwards requests Discord's CSP blocked, so plugin APIs like ReviewDB, `api.vencord.dev` and `badges.equicord.org` work |
| `/clipboard-image` | WebKitGTK never exposes clipboard images to the DOM — pasting a screenshot yields `types=[] files=0 items=0` while text works fine. This reads the GTK clipboard natively and hands back a PNG |
| `/zoom` | Ctrl +/-/0, applied with WebKit's native zoom and persisted |

Three CSP directives matter, and each needs a different trick:

| directive | blocks | approach |
|---|---|---|
| `connect-src` | plugin API calls | allows `http://127.0.0.1:*`, so requests are re-issued through the endpoint |
| `img-src` | third-party images (badges, avatars) | does **not** allow localhost, but does allow `blob:` — so a failed image is fetched through the endpoint and swapped to an object URL, cached per URL |
| `script-src` | remote scripts | not worked around; the client mod is injected as a privileged user script instead, which CSP does not police |

Third-party **WebSockets** remain blocked: `connect-src` permits only Discord's own
`wss://` hosts plus `ws://127.0.0.1:*`, and relaying arbitrary WebSocket traffic is
a much bigger job than request forwarding. Plugins that open their own sockets to
non-Discord hosts will still fail.

Every request must carry a 32-character random per-launch token in the path;
without it the server returns 403, so no other local process can use it as a relay.
The request forwarder only ever retries **after** a failure, so Discord's own
traffic goes out natively and untouched.

This is deliberately not Tauri IPC. The Discord webview is granted **no** IPC
surface at all — see [Security notes](#security-notes).

## Known limitations

### Web build vs desktop build

`browser.js` runs the mod in `IS_WEB` mode. Plugins gated behind
`IS_DISCORD_DESKTOP` / `IS_VESKTOP` will not appear, settings and themes live in
IndexedDB instead of on disk, and the in-app updater is disabled.

Separately, Discord gates screenshare-with-audio and global push-to-talk on the
presence of `window.DiscordNative` — again not on the user agent. Vesktop and
Equibop get those because an Electron preload provides that object.

Both have the same remedy, and it is now the obvious next milestone: implement
`DiscordNative` and `VencordNative` shims backed by Tauri commands, and switch to
the **desktop** build (`renderer.js` + `renderer.css`). This was pointless while
WebRTC was unavailable, since the desktop build's headline wins are voice
features — with a patched WebKitGTK it is worth doing.

## If you want to fork this

`Palladium` was only ever a working title. Renaming touches `package.json`,
`src-tauri/Cargo.toml`, `tauri.conf.json` (`productName`, `identifier`) and the
`palladium_lib` crate name.

The parts most worth salvaging, in rough order of how much work they represent:

- `scripts/build-webkit-webrtc.sh` + `scripts/webkit-probe.py` — getting WebRTC
  into WebKitGTK at all. Useful to **any** Linux project on wry, Tauri, WebKitGTK
  or WPE that needs `RTCPeerConnection`, not just Discord clients.
- `src-tauri/src/arrpc/` — a self-contained Rust arRPC. No Node, no sidecar, and
  the stock Vencord/Equicord plugin connects to it unmodified. Worth extracting on
  its own: Equibop currently ships **103 MB of bundled Node runtime** for the same
  job.
- `src-tauri/src/webkit.rs` — the WebKitGTK fixes for media, permissions and the
  file chooser. Useful to any Tauri app that needs `getUserMedia` or uploads.
- `scripts/setup-windows-cross.sh` — cross-compiling a Tauri app *and* bundling its
  NSIS installer from Linux, including the wine workarounds.
- `src-tauri/src/inject.rs` + `mods.rs` — injecting a client mod's browser build
  into a plain system webview.

## License

MIT — see [LICENSE](LICENSE). Do whatever you want with it.

Vencord and Equicord are GPL-3.0 but are **not** redistributed here; their browser
builds are downloaded from their own release pages at runtime.

## Credits

- [Vencord](https://github.com/Vendicated/Vencord) and [Vesktop](https://github.com/Vencord/Vesktop)
- [Equicord](https://github.com/Equicord/Equicord) and [Equibop](https://github.com/Equicord/Equibop)
- [arRPC](https://github.com/OpenAsar/arrpc) by OpenAsar — the protocol work this reimplements

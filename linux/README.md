<div align="center">

<img src="src-tauri/icons/128x128.png" width="96" alt="Coucou icon">

# Coucou for Linux

**Mochi lives on the edge of your screen — top, bottom, left or right — and keeps an eye on your Claude Code sessions.**

Approve Claude Code permissions, watch your session work, drop a file, chat with Claude — without leaving what you're doing.

</div>

---

Built for **Wayland** (KDE Plasma, Sway, Hyprland, …). Under X11 it falls back to a
plain always-on-top window, which works but is not the target.

This is the Windows port's frontend (`../windows`) with a Linux backend: Tauri 2, a
Rust core, and Mochi drawn on a canvas. Only **Claude Code** is wired up for now;
the other integrations that came with the port (Stripe, n8n, GitHub, …) are still
there and stay off until you add a key.

## Install

You need Rust, Node 20+ and the system libraries. On **Arch / CachyOS**:

```sh
sudo pacman -S --needed base-devel rust nodejs npm pkgconf \
  webkit2gtk-4.1 gtk3 gtk-layer-shell libayatana-appindicator dbus
```

On Debian / Ubuntu:

```sh
sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
  libgtk-layer-shell-dev libayatana-appindicator3-dev libdbus-1-dev libssl-dev
```

Then:

```sh
cd linux
scripts/install.sh
```

It builds and installs into `~/.local` — no root. `scripts/install.sh --uninstall`
removes it again.

## Using it

Start **Coucou** from your launcher, then **Settings… → Claude Code → Install hooks…**
(the tray icon has *Settings…*, or click the gear in the island). You see the exact
diff of what will change in `~/.claude/settings.json` and the path of the dated
backup that will be taken. Nothing is written until you click, your own hooks are
never touched, and uninstalling removes only Coucou's entries.

| What you do | What happens |
|---|---|
| Move the pointer into the hidden island's edge | Mochi peeks out |
| Click the small island | It opens |
| Click Mochi | It gets annoyed. Three times in a row and it goes dizzy |
| Rest the pointer on Mochi for two seconds | Hearts |
| Drag a file onto the island | Mochi turns into a box, swallows it, then offers to answer questions about it |
| `Esc` | Closes the island |

A Claude Code permission request opens the island with **Deny / Allow**; a finished
session shows what it did.

### Which edge

**Settings… → General → Island lives on** moves it to the top, bottom, left or right
of the screen, always centred along that edge, and it moves right away. On the
left and right the island is the same horizontal shape, growing out of the edge.

Which monitor it uses is the compositor's call (on Plasma, the active one). It stays
clear of panels instead of drawing over them, so a bottom island sits on top of the
Plasma panel rather than under it.

### What is different from macOS and Windows

Wayland gives an app no global cursor position, so Mochi's eyes follow the pointer
only while it is over the island, not across the whole screen. The pointer reaches the
island through ordinary mouse events, which is also why a hidden island costs no CPU.

## Claude Code

The relay is a tiny executable, `coucou-hook`, copied to
`~/.local/share/coucou/bin/` at launch and talking to Coucou over a Unix socket in
`$XDG_RUNTIME_DIR` (mode 0600). It is given 300 ms to reach Coucou and exits cleanly
if the app is closed, slow or crashed — **a Claude Code session is never blocked or
slowed down by Coucou.** If nobody answers a permission request in time, Coucou stays
quiet and Claude Code asks in the terminal as usual. It works from any terminal.

## Keys and privacy

The Anthropic API key (for the chat) and any integration key live in your keyring
through the Secret Service — KWallet on Plasma — never on disk and never in the
interface. No telemetry. The only network requests Coucou makes are to the services
you configure yourself.

Files: settings in `~/.config/coucou/`; relay, inbox and log in
`~/.local/share/coucou/`.

## Troubleshooting

- **Blank island, or nothing appears** (often NVIDIA): start it with
  `WEBKIT_DISABLE_DMABUF_RENDERER=1 coucou`.
- **Log:** `~/.local/share/coucou/coucou.log` says whether the island got a layer-shell
  surface and whether the relay is listening.
- **`Install hooks…` says the relay is missing:** restart Coucou, or check the log line
  `coucou-hook not found`.

## Develop

```sh
npm install
npm run dev        # the frontend alone, in a browser, with the mouse as the cursor
npm run tauri dev  # the whole app
cargo test         # relay + hook installer tests
```

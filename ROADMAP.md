# Roadmap

Ideas that are **not** in the current release. PRs welcome.

## Next up

- **Process that owns the port** — show `node.exe` / `python` / Docker next to `:5173` so a stale process is obvious.
- **More default ports** — Storybook `6006`, Expo/Metro `8081`, Vite preview `4173`, webpack `8888`.
- **Pin a port** — keep the QR locked on one server when several are active.
- **HTTPS in the QR** — emit `https://` for mkcert / `vite --https` instead of always `http://`.
- **`--host` copy command** — on LOCAL ONLY, copy the exact dev-server flag for the detected framework.
- **Path presets** — chips for `/admin`, `/api`, `?debug=true` instead of a single custom path field.
- **Optional public tunnel** — Cloudflare Tunnel or ngrok when the phone is not on the same Wi-Fi. Default stays LAN-only / offline.
- **Global hotkey** — e.g. `Ctrl+Shift+Q` to restore the window from the tray.

## Maybe later

- Network device scanner (out of scope — different product).
- QR visual customization.
- Light theme (the neon brutalist look is the identity).

## Shipped

- Auto-scan + QR with LAN IP
- Framework detection, custom ports/paths, tray, autostart
- LAN IP heuristic (skip VPN/WSL) and **manual interface picker**
- Native OS notification when a new port comes online
- Open the URL in the desktop browser
- Windows / macOS / Linux installers via GitHub Actions

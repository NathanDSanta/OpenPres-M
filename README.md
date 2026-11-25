# OpenPres Manager

The interface of [OpenPres](https://github.com/NathanDSanta/OpenPres): songs and slides, the
Bible, screens, audio, the stream, translation and the settings. It runs as its own desktop
program on the operators' computers and finds the OpenPres servers on the network; the server
also serves it to browsers and iPads, and shows it in its own windows.

One of the OpenPres repositories: the server is [OpenPres](https://github.com/NathanDSanta/OpenPres),
the Stream Deck plugin [OpenPres-SD](https://github.com/NathanDSanta/OpenPres-SD), the viewer
app [OpenPres-V](https://github.com/NathanDSanta/OpenPres-V) and the Translation app
[OpenPres-T](https://github.com/NathanDSanta/OpenPres-T).

## Development

React and TypeScript, built with Vite: `npm ci`, then `npm run dev` (with a server running on
this computer) or `npm run build`. The desktop program is a small Tauri 2 shell in
`src-tauri/` (Rust): `npm run tauri dev` opens it. It finds servers (mDNS), starts the one on
this computer (the server's own program, with `--server`), keeps its logs in
`OpenPres/Manager/logs` and updates itself from this repository's releases; the beta program
is the choice of the server it managed last.

Work on a branch named `feature/…`, `fix/…`, `docs/…`, `chore/…` or `ci/…`, and open the pull
request into `develop` with a title like `feat: …` or `fix: …`. The checks (types and build;
the desktop program's tests and clippy) run on every pull request. Betas and stable releases:
[docs/RELEASING.md](docs/RELEASING.md).

Each release publishes the desktop program for Windows, macOS and Linux
(`OpenPres-Manager-<version>-<computer>…`), its update file (`latest.json`) and the web build
(`OpenPres-Manager-web-<version>.zip`), which the server bundles. The installed program
updates itself; it needs the `TAURI_SIGNING_PRIVATE_KEY` secret (the same key as the server's)
to sign its updates. `compatibility.json` says which servers this Manager works with
(`oldest_server`).

The code may not be reformatted with Prettier: match the surrounding style by hand.

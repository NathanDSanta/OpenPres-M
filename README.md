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
this computer) or `npm run build`. Work on a branch named `feature/…`, `fix/…`, `docs/…`,
`chore/…` or `ci/…`, and open the pull request into `develop` with a title like `feat: …` or
`fix: …`. The checks (types and build) run on every pull request. Betas and stable releases:
[docs/RELEASING.md](docs/RELEASING.md).

Each release publishes the web build (`OpenPres-Manager-web-<version>.zip`), which the server
bundles. `compatibility.json` says which servers this Manager works with (`oldest_server`).

The code may not be reformatted with Prettier: match the surrounding style by hand.

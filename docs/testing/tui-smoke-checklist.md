# TUI Smoke Checklist

This is a manual checklist for quickly validating the Convergence TUI after changes.

Prereqs:
- A workspace with `.converge/` initialized.
- Remote configured via `login --url ... --token ... --repo ...`.

## Root

- [ ] Local root shows Status by default.
- [ ] `Tab` toggles to remote root dashboard.
- [ ] `/` shows root command palette.

## Auth

- [ ] Header shows `user@server` when token is valid.
- [ ] When token is missing/invalid, header shows auth guidance and remote dashboard shows an auth-required panel.

## Inbox

- [ ] `inbox` opens and lists publications.
- [ ] `fetch` from inbox fetches selected snap.
- [ ] `bundle` from inbox creates a bundle from selected publication.

## Bundles

- [ ] `bundles` opens and lists bundles.
- [ ] `approve` works.
- [ ] `promote` works (or errors with actionable guidance).
- [ ] `release <channel>` creates a release from the selected bundle.
- [ ] `superpositions` opens for bundles with conflicts.

## Releases

- [ ] `releases` opens and lists channels.
- [ ] `fetch` fetches the selected channel into the local store.

## Lanes

- [ ] `lanes` opens and lists lane heads.
- [ ] `fetch` fetches the selected lane head.

## Local workflow

- [ ] Local `snap` creates a snap.
- [ ] `publish` works.
- [ ] `diff` works (optional: check CLI output separately).

# Release Process

- run `npm version patch --no-git-tag-version`, make pullrequest "rofl-client/ts: Release 0.1.2", merge
- checkout merged commit
- `git tag --sign --message="Typescript ROFL Client 0.1.2" rofl-client/ts/v0.1.2`
- `git push origin tag rofl-client/ts/v0.1.2`
- see https://github.com/oasisprotocol/oasis-sdk/actions/workflows/release-rofl-client-ts.yml

# Security policy

## Reporting a vulnerability

Please do not open a public issue for vulnerabilities involving credentials,
private account data, local file exposure, command execution, or network data
leaks. Use GitHub's private vulnerability reporting for this repository.

Include:

- affected version or commit;
- reproduction steps using synthetic data;
- impact and expected behavior;
- any proposed mitigation.

Never include a real `MUSIC_U` cookie, private listening history, account UID,
or unredacted backend logs in a report.

## Credential handling

- Login cookies are user-local runtime state and must never be committed.
- `NETEASE_COOKIE` is read from the process environment.
- Persisted media URLs and query tokens are scrubbed from caches.
- Logs redact URL query strings, nested JSON URLs, and `MUSIC_U` values.
- Shell and backend child processes are terminated when the host exits.

## Supported versions

Security fixes are applied to the latest commit on the default branch.

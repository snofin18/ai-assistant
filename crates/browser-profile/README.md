# assistant-browser-profile

> Dedicated browser-profile lifecycle and the hard no-copy-user-profile invariant.

## Responsibilities

- Own a separate `--user-data-dir` per automation profile.
- Provide a stable window-title suffix.
- Track whether a first human login is required.
- Refuse any operation that would copy the user's normal browser profile.

## Boundaries

- no browser launch, CDP transport, policy decision, or credential handling;
- no network, filesystem persistence, or browser process lifecycle;
- no identifier generation from randomness.

## Invariants

1. A profile always pins its own `--user-data-dir`.
2. A profile always refuses user-profile copying.
3. Profile ids must be one bounded path component; empty, control, separator,
   drive-prefix, and dot-segment ids are rejected before they become state.

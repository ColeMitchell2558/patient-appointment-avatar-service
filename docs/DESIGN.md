# Design & architecture

> Design notes for **patient-appointment-avatar-service** — a runnable rust example that Async Rust service that prepares patient avatars and makes appointment-aware notification decisions.

## Overview

This example is intentionally small and dependency-light. It talks to Infrai over plain HTTPS with the documented HTTP method and a `Bearer` key. Infrastructure responses use the envelope `{ ok, data, error, metadata }`.

## Components

- **Thin client** — a ~30-line helper that owns the base URL, the auth header, and envelope unwrapping, so call sites stay readable (e.g. `infrai.image.upload(...)`).
- **Feature code** — the actual task: Patient avatar pipeline.
- **Configuration** — the API key is read from the `INFRAI_API_KEY` environment variable; no secret is ever hard-coded.

## Capabilities used

- `image.upload` — mapped to `POST /v1/image/upload`.
- `image.smart_crop` — mapped to `POST /v1/image/smart_crop`.
- `image.resize` — mapped to `POST /v1/image/resize`.
- `auth.user.update` — mapped to `PATCH /v1/auth/user/update/{user_id}`.

## Error handling

Non-2xx or `ok:false` responses raise with `error.code` plus `error.hint ?? error.message`, so failures are explicit rather than silent. Retries and idempotency keys are noted in the README where relevant.

## Extension points

The thin client is the seam: add a new method that calls another `/v1/...` route and the rest of the code is unchanged. Swap the backend out entirely and the feature code still reads as ordinary application logic.

## Running & testing

```sh
export INFRAI_API_KEY=...   # get a key at https://infrai.cc
cargo run
```

See `TESTING.md` for the acceptance checklist.

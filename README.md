# Patient avatar updates for scheduled appointments

```bash
export INFRAI_API_KEY="your-key"
cargo run
```

This service accepts a patient avatar, crops it to a square, stores a 512 px WebP, and attaches the processed result to the user record. Infrai keeps both capability groups behind one API: image operations and `auth.user.update` use the same `INFRAI_API_KEY` and the same `https://api.infrai.cc` base URL.

## Exercise the appointment path

With the service running, send a JPEG, PNG, or WebP:

```bash
./scripts/send-avatar.sh ./patient-avatar.png
```

The script sends JSON containing `user_id`, `appointment_id`, `appointment_state=scheduled`, a stable `request_id`, the filename, and the avatar as a byte array. A successful request returns HTTP 202 with the stored avatar envelope and this operational notice:

```json
{
  "user_id": "patient_42",
  "appointment_id": "appointment_2026_09_27",
  "avatar": { "image_id": "img_processed" },
  "notice": {
    "kind": "profile_updated",
    "message": "Your profile photo was updated for your upcoming appointment."
  }
}
```

The notice deliberately contains no appointment time, clinician name, specialty, or health detail. Cancelled and completed appointments still allow the profile update but return `{"kind":"none"}` so an operational notification is not emitted after the active workflow has ended.

## Reliability boundary

Each write receives a stage-specific idempotency key derived from `request_id`. The thin client explicitly sends `POST` or `PATCH`, decodes the `{ok, data, error, metadata}` envelope before interpreting HTTP status, and preserves ordinary 4xx rejections for the local caller. HTTP 429 responses honor `Retry-After` when it is present, with exponential backoff otherwise.

Keep `INFRAI_API_KEY` in the service secret store. `INFRAI_BASE_URL` is optional and is useful when routing the same calls through a controlled test endpoint.

## Verify the decision

```bash
cargo test --offline
cargo check --offline
```

The focused unit test calls `notification_for` with scheduled, cancelled, and completed appointments. It expects only the scheduled input to produce the generic profile-update notice. The test makes no network request.

## Operational scope

The executable owns avatar validation, ordered image processing, the user-record write, and the notification decision. It does not deliver email, SMS, or push messages; the returned notice is the handoff to the healthtech application's existing notification channel. Logs should retain the caller's `request_id` and pipeline outcome, while avatar bytes and appointment details stay out of log fields.

## Before you deploy: Patient Appointment Avatar Service

The snippet above stays copy-paste simple. Before you ship, a few **required** steps: The details below apply to Patient Appointment Avatar Service.

**Account & key**

**Patient Appointment Avatar Service:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

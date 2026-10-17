# One-button conversation companion

The foreground GTK client connects this device's existing microphone controls
and audio player to Telecodex's integrating conversation. Native sessions,
routing, questions and work remain owned by that controller. There is no second
session owner or local model client.

Build the ordinary UI-enabled binary and launch it explicitly:

```sh
cargo build --locked
daimon-integrators companion /absolute/private/companion.json
```

The selected device must first be enrolled through Telecodex's existing
`dashboard.companion` command. See the owning
[enrollment contract](https://github.com/AlterMundi/telecodex/blob/main/docs/interaction-companion.md).
Keep the credential file on the device under its authorized user, mode 0600.
The local configuration is also an owner-only regular JSON file:

```json
{
  "schema": "daimon-integrators.companion-config/v1",
  "credential_file": "/absolute/private/device.json",
  "state_dir": "/absolute/private/conversation",
  "integrator_binary": "/usr/local/bin/daimon-integrators",
  "playback_command": ["pw-play"]
}
```

The existing panel must be running under the same selected user. Capture uses
its existing bounded audio mode and output-specific `stop-capture` command.
Python 3 and PipeWire's `pw-play` are runtime dependencies; the Python helper is
embedded in the binary. No Python package installation is needed on the device.
The playback command is an owner-selected argument array, never shell text or
content supplied by a remote session.

Press Talk to start recording, then press Send voice to stop the selected
capture and send its finalized clip. Hold for 900 ms to retain the recording
locally without sending and start another message. Closing stops only this
client's selected capture and player; work continues in its native sessions.
Opening the client does not capture, synthesize or play media. The first explicit
button gesture arms queued speech; if a question is already pending it first
plays that question. After that, a recording gesture takes priority over other
queued questions. Local microphone observation runs independently of remote
requests, so a stalled connection cannot make an old sample appear current.

Question playback is ordered and source-linked across all parts. A completed
question or worker result supplies an immutable contextual reference for the next
clip. Barge-in uses the currently audible supported source rather than an older
question. Starting a new playback clears the previous focus. Integrating replies
stay in the integrating native history and do not create unsupported device hints.

The reference uses `context_only: true`: the server retains the original and
lets the integrating model decide whether this is an answer, another task or a
clarification. It does not automatically answer or choose that source's session.
`question_context` inspects the reported question and `answer_question` consumes
only the exact retained original if that question is still current. An explicit
`#Topic` continues to follow the controller's shortcut. Older dashboards refuse
the `context:` wire prefix rather than silently falling back to a direct answer.
Worker audio includes the certified session label on the first part when the
selected server has Telecodex PR154. Real semantic quality and audible clarification
for unrouted originals still require complete receiving qualification.

An interrupted audio render never causes a speculative repeat. Barge-in stops
only the owned player before starting the microphone. Paused/unavailable sources
remain retained separately from completed playback; receiving audio or a player
exiting does not prove that a human heard it. The selected source is revalidated
before and after playback and during remote refresh.

Each original clip gets a durable UUID before capture, with immutable enrollment,
entry binding, reply reference and finalized byte hash. An uncertain POST remains
inspectable under that original UUID. Reconnection polls its receipt and does not
resend it. A Retry gesture sends only the same immutable clip and identity.
A new capture may proceed while earlier transcription is still preparing.
Saved local recordings and uncertain originals are not deleted on close. Only
one foreground client may own a local profile; its lease lasts through helper
shutdown. An enrollment change cannot silently reassign previous recordings.

## Automated qualification

```sh
python3 -B -m unittest discover -s tests -p 'test_companion*.py'
cargo fmt --check
cargo test --locked --no-default-features
cargo clippy --locked --all-targets -- -D warnings
```

The transport/runtime tests exercise actual TLS/private files, lost receipts,
immutable retries, independent in-flight recordings, source changes, synthesis
barge-in and delayed capture/player startup. They use no physical sensors or
model inference.

`tests/companion_native_ui.py` runs the actual GTK binary with Xvfb/XTest,
FFmpeg-generated synthetic audio, a TLS dashboard, actual Telecodex controller
and SQLite, enrolled credentials and synthetic App Server/ASR/speech peers.
Run it with a Python environment containing the Telecodex dashboard dependencies:

```sh
python3 -B tests/companion_native_ui.py \
  --telecodex-source /absolute/telecodex \
  --telecodex-binary /absolute/qualified-telecodex \
  --binary target/debug/daimon-integrators \
  --output /absolute/private/qualification
```

It requires Xvfb, libX11, libXtst, OpenSSL and FFmpeg. It never contacts production
or opens physical devices. Native question response, concurrent result handling,
long-hold retention and closing a selected capture are separate from real
Wayland/device installation, microphone quality, audible comprehension and
physical button acceptance. Those receiving observations remain required.

# Recording controls

In **Settings → General**, choose a shortcut and a recording mode:

- **Hold to talk** (the default, including upgrades): press and hold the shortcut, then release it to stop and transcribe.
- **Press to start / stop**: press and release once to start, then press again to stop and transcribe. Changing tabs or apps does not stop recording. Automatic paste uses the focused app when transcription finishes, as before.
- The shortcut row shows the saved shortcut and a single **Change** button. Additional choices appear only while editing; cancelling keeps the previous shortcut.
- **Use Fn** is available inside **Change** on macOS. It selects the standalone Fn/Globe key without requiring a webview to recognize a modifier-only key. Other shortcut combinations remain available through **Change**. Fn is reserved for dictation while selected; choose a different shortcut if you need it as a function-layer key while dictating.

Updates preserve every valid saved shortcut, recording mode and automatic-paste preference. Older settings without a recording mode keep Hold; Fn is selected only by an explicit user action.

Esc cancels microphone recording without transcribing. The existing duration limit applies to both modes. Shortcut and mode edits are rejected while recording or while an input event is being processed. Provider choice, credentials, cloud entitlements and cleanup settings are unchanged.

On Mac, microphone recordings shorter than 350 ms are discarded. Remaining recordings pass through a small offline voice detector before provider selection, temporary-file creation or ASR. The detector uses a fresh state per recording, 16 ms frames, at least 96 ms of probable voice and 64 ms of consecutive confident voice. Bounded amplification affects only detector input, not the original audio. Non-speech produces the translated **No speech detected** message without clipboard/history changes, cleanup or cloud transcription usage. Imported files retain their separate pipeline. The pinned Earshot dependency is compiled only on macOS; no model download or network service is required for detection.

Fn requires the existing macOS Accessibility permission. If macOS still opens emoji or system dictation when Fn/Globe is pressed, select **Do Nothing** for that key under **System Settings → Keyboard**. Dictámelo does not change this system preference automatically. Keyboard utilities and secure-input fields may intercept global shortcuts.

## Implementation

Native shortcut edges, Escape and duration deadlines enter one ordered worker. Microphone startup and stop are awaited in that worker, so a quick release or second press cannot overtake startup. Transcription runs separately, keeping cancellation responsive. The recording plan captures the mode at the same time as the provider/privacy settings. Repeated key-down events are ignored until a matching release; stale duration deadlines cannot stop a later recording.

macOS Fn uses a Core Graphics event tap on its own run-loop thread. It filters physical virtual key code 63 and the SecondaryFn flag; arrows and function keys carrying the same flag are not mistaken for the Fn key. It passes unrelated events through, releases an already-held Fn even if another modifier is down, and cancels safely if macOS disables the tap. Escape is observed both inside and outside the app. The shared mode logic remains usable by the Windows shortcut adapter; Windows runtime validation is deferred.

## Installed-app regression matrix

Use a downloaded local model, disable cloud cleanup and use a disposable text document when testing microphone/paste behavior. Retain the user's original settings and history.

1. Upgrade with an existing shortcut: Hold and the original shortcut remain selected.
2. Select Fn, quit and reopen: selection persists, with no credential changes.
3. Hold Fn: one recording starts; release transcribes once. Try a very quick tap too.
4. Toggle Fn: release leaves recording active; press again stops once. Repeat across apps/tabs.
5. Repeat both modes with a regular multi-key shortcut. Holding a toggle shortcut must not alternate start/stop on key repeat.
6. Cancel with Esc while Settings is focused and while another app is focused. A following release must not transcribe cancelled audio.
7. Let the duration limit stop a toggle recording, then start another. A previous timer must not stop the new recording.
8. Verify Fn+other modifier presses and arrow/function keys do not trigger a standalone Fn edge. Check any installed keyboard-remapping utility separately.
9. Verify shortcut/mode controls are unavailable during recording and failed shortcut registration leaves the previous shortcut usable.

Report physical-key tests separately from synthetic key events, mocked UI and reducer tests. Passing Mac tests does not imply Windows hardware coverage.

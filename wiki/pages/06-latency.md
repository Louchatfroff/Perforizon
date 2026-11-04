---
title: Latency
section: Use
summary: The input-to-screen counter, present modes, frame latency, network batching and what was not done.
---

# Latency

Perforizon cares about input lag as much as frame rate. This page covers the tools it has for measuring and lowering it. All the options are listed in [Settings](05-settings.md).

## The input-to-screen counter

Turn on `latencyHud` (Advanced Graphics, or `horizon-ctl graphics latencyHud true`). It shows the input-to-screen delay, average and worst of the last 240 frames, in the PvP HUD corner of the ping counter, and under `input_latency_ms` in `horizon-ctl status`. See [PvP](07-pvp.md) for the HUD corners.

How it works: the oldest input event not yet shown (mouse motion, key, button) is stamped when it arrives. The renderer times it at `queue.present` (the `Present` figure) and when the GPU reports the submission done (the `Input` figure, read at the next submit, so up to one frame late). It does not include the display's own scanout.

It never changes a frame, and the auto-tune never tunes it.

## Present modes

`presentMode` is used only with VSync off.

| Value | Meaning |
| --- | --- |
| `auto` (default) | Presents with Immediate and one frame of queue; shown as Auto (VSync) in the menu. |
| `fifo` | Classic vsynced queue. |
| `mailbox` | Latest frame replaces the waiting one. The auto-tune also tries it (VSync is off while it runs). |
| `immediate` | Present at once. |

## Frame latency

`frameLatency` is the number of frames queued ahead, 0 to 3. The default 0 means auto: 1, or 2 on Fifo. The auto-tune also tries 2.

## How the auto-tune picks

`presentMode` and `frameLatency` change no pixels, so they run in 100% parity too. The offscreen benchmarks have no swapchain and cannot see them; the in-game auto-tune can, because it runs in a real window.

For these two steps the rule is not "beats the first value by 3%". Every candidate within 3% of the best score is a tie, and the tie goes to the first value in the list, which is the lower-latency one. A higher-latency value is kept only when it is clearly faster. See [Auto-tune](04-auto-tune.md).

## Network: netLowLatency

`netLowLatency` (off by default) holds small network packets and sends them as one train, at the start of each frame and the end of each tick on the client, and after each message on a LAN host. The bytes on the wire are the same and protocol 777 is unchanged. `TCP_NODELAY` is already set on every game connection, as vanilla does.

On loopback a burst of three packets plus the answer takes about 15 to 17 microseconds either way: batching saves system calls and TCP segments, not round-trip time. The auto-tune cannot measure a network and never toggles it.

## Why late input sampling was not done

The idea is to read the newest mouse input just before the frame is submitted. It was not added because winit hands input over only between event-loop callbacks, so nothing new can be read between `apply_mouse_look` (right after the frame's sleep and ticks) and the submit. The frame limiter also sleeps at the end of a frame, before the next input is read.

## Related

- Hit prediction, crystal and anchor optimizers: [PvP](07-pvp.md).
- Pipeline cache and other start-up options: [Settings](05-settings.md).
- If the game is capped at a low frame rate in the background: [Troubleshooting](13-troubleshooting.md).

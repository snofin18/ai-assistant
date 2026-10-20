# SPIKE-G CDP harness

This directory contains a one-off Stage 0 spike. It is not product code.

The runner uses only the Python standard library and a dedicated browser profile.
It never attaches to the user's normal Edge or Chrome profile.

Run the local self-check:

```powershell
python spikes/spike-g-cdp/run_spike.py --self-check
```

Run the real Edge and Chrome experiments:

```powershell
python spikes/spike-g-cdp/run_spike.py --browsers edge,chrome
```

Generated evidence is written to `spikes/spike-g-cdp/artifacts/`.

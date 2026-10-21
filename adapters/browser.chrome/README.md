# com.google.chrome Adapter v0

This directory is the declarative Chrome adapter pack. It contains data only.
Runtime execution, selector calibration, real-machine measurement, and the
T5.1/T5.2/T5.3 workflows are owned by later Stage 1c cards.

## Files

- `adapter.toml`: app identity, channel, profile, health, and safety defaults.
- `app_map.json`: command table, shell UI map, workflows, known pitfalls,
  site blacklist, and state machine.
- `selectors/targets.json`: provisional shell selector declarations.
- `tools/tools.json`: CDP and shell tool declarations with postconditions.
- `interrupts/interrupts.json`: cookie banner, login wall, and captcha handling.

## Safety Boundaries

- Visible text is never the primary selector.
- Site blacklisting is enforced for banking, payment, password change, and
  captcha domains.
- Cookie banners, login walls, and captcha challenges always require a human.
- Form filling never submits automatically.
- Downloads must go to a dedicated shadow directory and be verified.

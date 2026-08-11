# Interaction Contract

```text
stopped -> send synthetic follow-up -> fresh process
running -> confirm? no -> unchanged
running -> confirm? yes + no queued input -> queue synthetic follow-up
running -> confirm? yes + queued input -> preserve queued input
```

The action is disabled while send/queue mutations are in flight.

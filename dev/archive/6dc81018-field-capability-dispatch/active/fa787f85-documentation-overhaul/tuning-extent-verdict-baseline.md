# Validator verdicts before and after the declaration-locator change

Each committed tuning-extent campaign, validated in publication mode against the main checkout.

```
commit=6652402912ede6f5e20399438edb1d1dd253b2e0
## gf2-a83583e0-20260930t230000z-2728298
$ python3 dev/scripts/validate-tuning-extent-campaign.py --stage /tmp/gf2-a83583e0-20260930t230000z-2728298 --publication /home/vkaskivuo/Projects/gf2
GF2_TUNING_VALIDATION={"schema":"tuning-extent-campaign-publication-v1","campaign_id":"gf2-a83583e0-20260930t230000z-2728298","status":"published","entries":12970}
exit=0
## gf2-dbd8787d-20261001t230000z-2601601
$ python3 dev/scripts/validate-tuning-extent-campaign.py --stage /tmp/gf2-dbd8787d-20261001t230000z-2601601 --publication /home/vkaskivuo/Projects/gf2
GF2_TUNING_VALIDATION={"schema":"tuning-extent-campaign-publication-v1","campaign_id":"gf2-dbd8787d-20261001t230000z-2601601","status":"published","entries":13669}
exit=0
```

## After the change

The changed validator (`2399b5cc7`) checks the same `/tmp` stages and the main checkout at `c711a1fff`, which holds the same published destinations and archives as `66524029` (the intervening commits touch neither); verdicts and entry counts are identical.

```
validator=.agents/worktrees/agent-8bd873f7/dev/scripts/validate-tuning-extent-campaign.py at 2399b5cc7326580d37960f9323bd55dad344ad55; checkout=c711a1fffba082e7b4d913189b43f76f06c093ff
## gf2-a83583e0-20260930t230000z-2728298
$ python3 .agents/worktrees/agent-8bd873f7/dev/scripts/validate-tuning-extent-campaign.py --stage /tmp/gf2-a83583e0-20260930t230000z-2728298 --publication /home/vkaskivuo/Projects/gf2
GF2_TUNING_VALIDATION={"schema":"tuning-extent-campaign-publication-v1","campaign_id":"gf2-a83583e0-20260930t230000z-2728298","status":"published","entries":12970}
exit=0
## gf2-dbd8787d-20261001t230000z-2601601
$ python3 .agents/worktrees/agent-8bd873f7/dev/scripts/validate-tuning-extent-campaign.py --stage /tmp/gf2-dbd8787d-20261001t230000z-2601601 --publication /home/vkaskivuo/Projects/gf2
GF2_TUNING_VALIDATION={"schema":"tuning-extent-campaign-publication-v1","campaign_id":"gf2-dbd8787d-20261001t230000z-2601601","status":"published","entries":13669}
exit=0
```

## With entry a83583e0 under the epic directory

The validator of the checkout at `a0ebe2884`, where the declaration of a83583e0 lies under `dev/active/6dc81018-field-capability-dispatch/a83583e0/`, checks the same `/tmp` stages and the main checkout at `ef6fbdd25`; verdicts and entry counts are identical.

```
validator=.agents/worktrees/agent-b23c364a/dev/scripts/validate-tuning-extent-campaign.py at a0ebe288401f2dc5ae3cb0cf10194c4f74c08f34; checkout=ef6fbdd25a1a27db7eb6bae695916f8aac79b77d
## gf2-a83583e0-20260930t230000z-2728298
$ python3 .agents/worktrees/agent-b23c364a/dev/scripts/validate-tuning-extent-campaign.py --stage /tmp/gf2-a83583e0-20260930t230000z-2728298 --publication /home/vkaskivuo/Projects/gf2
GF2_TUNING_VALIDATION={"schema":"tuning-extent-campaign-publication-v1","campaign_id":"gf2-a83583e0-20260930t230000z-2728298","status":"published","entries":12970}
exit=0
## gf2-dbd8787d-20261001t230000z-2601601
$ python3 .agents/worktrees/agent-b23c364a/dev/scripts/validate-tuning-extent-campaign.py --stage /tmp/gf2-dbd8787d-20261001t230000z-2601601 --publication /home/vkaskivuo/Projects/gf2
GF2_TUNING_VALIDATION={"schema":"tuning-extent-campaign-publication-v1","campaign_id":"gf2-dbd8787d-20261001t230000z-2601601","status":"published","entries":13669}
exit=0
```

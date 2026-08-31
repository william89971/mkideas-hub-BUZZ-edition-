# Synthetic Command Center migration bundle

This bundle is deliberately fictional. It exercises the pure migration
contracts without connecting to the old Command Center, a Buzz relay, or media
storage.

Coverage includes:

- one admin and two partner membership mappings;
- a DNC guest with topics, tags, a human note, research, and outreach;
- an interview with questions and timestamped transcript segments;
- versioned content with a clip and a pending approval;
- a failed agent run, task, attention/comment, activity, and audit record.

The old schema has no binary attachment table. `legacy_external_references.json`
shows the HTTPS references that a real exporter preserves but never fetches.
The small WebVTT file is a fictional, locally supplied attachment that exercises
hash verification and interview association; a real run requires separately
approved local files under a bounded attachment root.

No IDs, names, addresses, messages, or transcript text in this directory are
real.

# MK Ideas Buzz repository rename procedure

This is documentation only. The current GitHub slug remains unchanged until a
separate rename authorization.

1. Confirm the proposed clean slug is available.
2. Inventory open pull requests, branch protections, Actions variables/secrets,
   release assets, package identities, deployment webhooks, app-store links, and
   documentation URLs.
3. Keep the local checkout directory and branches unchanged.
4. Rename through GitHub settings only after authorization.
5. Update local `origin` to the renamed MK fork.
6. Preserve `buzz-upstream` for the upstream Buzz source.
7. Preserve `command-center-reference` as fetch-only with push disabled.
8. Verify GitHub redirects, fetch, push dry-run, CI, artifact workflows, release
   metadata, and external webhooks.
9. Do not force-push, recreate branches, publish releases, or combine the rename
   with production deployment.


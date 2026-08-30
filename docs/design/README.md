# MK Ideas V0 design evidence

`mkideas-v0-concept.png` is the generated design direction used before
implementation. The four `mkideas-v0-*.png` files are deterministic desktop
captures produced by `desktop/tests/e2e/mkideas-v0.spec.ts`.

Implementation-to-concept comparison:

1. Both preserve the permanent five-area navigation and keep Search universal.
2. Both use a warm editorial canvas, high-contrast serif hierarchy, restrained
   red accents, and compact operational metadata rather than generic SaaS cards.
3. The concept's review queue became Today’s explicit human gate with signed
   Approve/Reject actions and an “AI cannot clear this list” invariant.
4. The concept's mobile guest capture maps to the Flutter People bottom sheet;
   desktop uses the same fields and signed event contract.
5. The implementation goes beyond the visual concept by exposing signer,
   version, provenance, transcript/clip timestamps, and Team record references.

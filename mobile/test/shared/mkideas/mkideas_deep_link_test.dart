import 'package:buzz/shared/deeplink/deep_link.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const community = 'hub.mkideas.org';
  const entityId = '11111111-1111-4111-8111-111111111111';
  const eventId =
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

  test('builds and parses the canonical MK Ideas entity link', () {
    final link = buildMkIdeasEntityLink(
      community: community,
      kind: 30803,
      entityId: entityId,
    );

    expect(
      parseBuzzDeepLink(Uri.parse(link)),
      const MkIdeasDeepLink(
        community: community,
        kind: 30803,
        entityId: entityId,
      ),
    );
  });

  test('preserves an explicitly requested historical revision', () {
    final link = buildMkIdeasEntityLink(
      community: community,
      kind: 30804,
      entityId: entityId,
      eventId: eventId,
    );

    expect(parseMkIdeasDeepLink(Uri.parse(link))?.eventId, eventId);
  });

  test('rejects unknown kinds, duplicate parameters, and invalid UUIDs', () {
    final invalid = [
      'buzz://mkideas/entity?community=$community&kind=30900&d=$entityId',
      'buzz://mkideas/entity?community=$community&kind=30803&kind=30804&d=$entityId',
      'buzz://mkideas/entity?community=$community&kind=30803&d=guest-1',
      'buzz://mkideas/entity?community=$community&kind=30803&d=$entityId&extra=1',
      'buzz://mkideas/entity?community=localhost:70000&kind=30803&d=$entityId',
    ];

    for (final value in invalid) {
      expect(parseMkIdeasDeepLink(Uri.parse(value)), isNull, reason: value);
    }
  });
}

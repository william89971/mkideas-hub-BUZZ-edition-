import test from 'node:test';
import assert from 'node:assert/strict';
import { validateDeployment } from './preflight.mjs';

function valid() {
  const image = `ghcr.io/team/relay@sha256:${'a'.repeat(64)}`;
  return { services: {
    relay: { image, environment: {
      BUZZ_REQUIRE_AUTH_TOKEN: 'true', BUZZ_REQUIRE_RELAY_MEMBERSHIP: 'true',
      RELAY_OWNER_PUBKEY: 'b'.repeat(64), RELAY_URL: 'wss://hub.company.test',
      BUZZ_AUTO_MIGRATE: 'false', BUZZ_MK_DEVICE_GRANTS: 'audit',
    } },
    restic: { image, environment: { RESTIC_REPOSITORY: 's3:https://backup.company.test/bucket' } },
    prometheus: { image, ports: [{host_ip: '127.0.0.1', published: '9090'}] },
  } };
}

test('accepts closed access, immutable images, and private monitoring', () => {
  assert.deepEqual(validateDeployment(valid()), []);
});
test('rejects exposed internal services and unreviewed images', () => {
  const config = valid();
  config.services.prometheus.ports[0].host_ip = '0.0.0.0';
  config.services.relay.image = 'relay:latest';
  assert.equal(validateDeployment(config).length, 2);
});
test('rejects placeholder hosting and premature enforcement', () => {
  const config = valid();
  config.services.relay.environment.RELAY_URL = 'wss://hub.example.invalid';
  config.services.relay.environment.BUZZ_MK_DEVICE_GRANTS = 'enforce';
  config.services.restic.environment.RESTIC_REPOSITORY = '/local-backup';
  assert.equal(validateDeployment(config).length, 3);
});

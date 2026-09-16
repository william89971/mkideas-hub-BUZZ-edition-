import { spawnSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

// Never print rendered configuration or secret contents.
export function validateDeployment(config) {
  const errors = [];
  const services = config.services ?? {};
  for (const [name, service] of Object.entries(services)) {
    if (service.profiles?.some((profile) => ['agents', 'push', 'pairing'].includes(profile))) continue;
    if (!/@sha256:[a-f0-9]{64}$/.test(service.image ?? '') || /example\.invalid|REPLACE/i.test(service.image)) {
      errors.push(`${name}: pin a real reviewed image digest`);
    }
    for (const port of service.ports ?? []) {
      if (name !== 'caddy' && !['127.0.0.1', '::1'].includes(port.host_ip)) {
        errors.push(`${name}: published ports must bind to loopback`);
      }
    }
  }
  const env = services.relay?.environment ?? {};
  if (env.BUZZ_REQUIRE_AUTH_TOKEN !== 'true' || env.BUZZ_REQUIRE_RELAY_MEMBERSHIP !== 'true') {
    errors.push('relay: closed membership and authenticated HTTP are required');
  }
  if (!/^[a-f0-9]{64}$/.test(env.RELAY_OWNER_PUBKEY ?? '')) errors.push('relay: configure the owner public key');
  if (!/^wss:\/\/[^/]+\/?$/.test(env.RELAY_URL ?? '') || /example|invalid|localhost/.test(env.RELAY_URL ?? '')) {
    errors.push('relay: configure the real public wss domain');
  }
  if (env.BUZZ_AUTO_MIGRATE !== 'false') errors.push('relay: run reviewed migrations separately before startup');
  if (env.BUZZ_MK_DEVICE_GRANTS === 'enforce' && env.BUZZ_MK_DEVICE_GRANTS_ENROLLMENT_COMPLETE !== 'true') {
    errors.push('relay: device enforcement requires a completed enrollment and recovery rehearsal');
  }
  const repository = services.restic?.environment?.RESTIC_REPOSITORY ?? '';
  if (!/^(s3:https:\/\/|sftp:|rest:https:\/\/)/.test(repository) || /example|invalid|REPLACE/.test(repository)) {
    errors.push('backup: configure a real encrypted off-server repository');
  }
  return errors;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const cwd = fileURLToPath(new URL('../', import.meta.url));
  const result = spawnSync('docker', ['compose', '--env-file', '.env', '-f', 'compose.yml', '--profile', '*', 'config', '--format', 'json'], { cwd, encoding: 'utf8' });
  if (result.status !== 0) {
    console.error('Preflight failed: Docker Compose could not resolve .env and compose.yml. No configuration values were printed.');
    process.exit(1);
  }
  const config = JSON.parse(result.stdout);
  const errors = validateDeployment(config);
  const active = Object.values(config.services).filter((s) => !s.profiles || s.profiles.some((p) => ['maintenance', 'monitoring'].includes(p)));
  const needed = new Set(active.flatMap((s) => (s.secrets ?? []).map((v) => typeof v === 'string' ? v : v.source)));
  for (const name of needed) {
    try {
      const stats = statSync(config.secrets[name].file);
      if (!stats.isFile() || stats.size === 0 || (stats.mode & 0o077) !== 0) errors.push(`${name}: secret must be a nonempty private file (chmod 600)`);
    } catch { errors.push(`${name}: required secret file is missing`); }
  }
  const alerts = readFileSync(resolve(cwd, 'alertmanager.yml'), 'utf8');
  if (!/\b(webhook_configs|email_configs|slack_configs|pagerduty_configs|opsgenie_configs):/.test(alerts)) {
    errors.push('monitoring: configure a real alert receiver and test its delivery');
  }
  if (errors.length) {
    console.error(errors.map((error) => `- ${error}`).join('\n'));
    process.exit(1);
  }
  console.log('Static deployment preflight passed. DNS/TLS, restore, and alert-delivery acceptance still require live verification.');
}

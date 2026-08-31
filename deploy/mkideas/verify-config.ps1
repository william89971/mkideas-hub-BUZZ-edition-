$ErrorActionPreference = 'Stop'

$deployDir = $PSScriptRoot
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
  throw 'Docker is required for Compose validation.'
}

$envFile = Join-Path $deployDir '.env.validation'
$secretDir = Join-Path $deployDir '.validation-secrets'
try {
  New-Item -ItemType Directory -Force -Path $secretDir | Out-Null
  $secretNames = @(
    'postgres_password', 'redis_password', 's3_access_key', 's3_secret_key',
    'relay_private_key', 'agent_private_key', 'restic_password',
    'backup_s3_access_key', 'backup_s3_secret_key', 'push_grant_keys',
    'push_token_keys', 'apns_identity.pem', 'app_attest_root.pem'
  )
  foreach ($name in $secretNames) {
    Set-Content -LiteralPath (Join-Path $secretDir $name) -Value 'validation-only' -NoNewline
  }

  @"
COMPOSE_PROJECT_NAME=mkideas-validation
MKIDEAS_DOMAIN=hub.example.invalid
MKIDEAS_SECRETS_DIR=$($secretDir -replace '\\','/')
MKIDEAS_RELAY_IMAGE=example.invalid/mkideas/relay@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
MKIDEAS_AGENT_IMAGE=example.invalid/mkideas/agent@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
MKIDEAS_PUSH_IMAGE=example.invalid/mkideas/push@sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
RESTIC_REPOSITORY=s3:https://backup.example.invalid/mkideas
"@ | Set-Content -LiteralPath $envFile

  & docker compose --env-file $envFile -f (Join-Path $deployDir 'compose.yml') --profile '*' config --quiet
  if ($LASTEXITCODE -ne 0) { throw 'docker compose config failed' }
  Write-Output 'MK Ideas Compose configuration is valid for every profile.'
}
finally {
  if (Test-Path -LiteralPath $envFile) { Remove-Item -LiteralPath $envFile -Force }
  if (Test-Path -LiteralPath $secretDir) { Remove-Item -LiteralPath $secretDir -Recurse -Force }
}

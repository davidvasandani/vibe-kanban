-- Organization-scoped environment variables.
-- Values are stored encrypted at rest (AES-256-GCM via JwtService).
-- Not synced via ElectricSQL (values are sensitive; read on demand by admins).

CREATE TABLE organization_env_vars (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    encrypted_value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, name)
);

CREATE INDEX idx_organization_env_vars_org ON organization_env_vars(organization_id);

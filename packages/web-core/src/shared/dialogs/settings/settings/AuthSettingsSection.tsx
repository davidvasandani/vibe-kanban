import { AwsSettingsSection } from './AwsSettingsSection';
import { ReauthSettingsCard } from './ReauthSettingsCard';

/**
 * Settings → Auth: the dashboard of every credential this host re-signs in
 * unattended, then AWS SSO profile management (the AWS scopes the dashboard
 * lists come from those profiles).
 */
export function AuthSettingsSection() {
  return (
    <>
      <ReauthSettingsCard />
      <AwsSettingsSection />
    </>
  );
}

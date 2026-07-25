import {
  GearIcon,
  GitBranchIcon,
  BuildingsIcon,
  CloudIcon,
  CpuIcon,
  PlugIcon,
  BroadcastIcon,
  PaperclipIcon,
  KanbanIcon,
  SlackLogoIcon,
  WrenchIcon,
  KeyIcon,
  ListChecksIcon,
} from '@phosphor-icons/react';
import type { Icon } from '@phosphor-icons/react';
import { useParams } from '@tanstack/react-router';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { GeneralSettingsSection } from './GeneralSettingsSection';
import { ReposSettingsSection } from './ReposSettingsSection';
import { OrganizationsSettingsSection } from './OrganizationsSettingsSection';
import { RemoteProjectsSettingsSection } from './RemoteProjectsSettingsSection';
import { AgentsSettingsSection } from './AgentsSettingsSection';
import { McpSettingsSection } from './McpSettingsSection';
import { RelaySettingsSectionContent } from './RelaySettingsSection';
import { AttachmentsSettingsSection } from './AttachmentsSettingsSection';
import { CliToolsSettingsSection } from './CliToolsSettingsSection';
import { AwsSettingsSection } from './AwsSettingsSection';
import { JiraSyncSettingsSection } from './JiraSyncSettingsSection';
import { SlackSettingsSection } from './SlackSettingsSection';
import { PipelinesSettingsSection } from './PipelinesSettingsSection';

export type SettingsSectionType =
  | 'general'
  | 'repos'
  | 'organizations'
  | 'remote-projects'
  | 'agents'
  | 'mcp'
  | 'cli-tools'
  | 'aws'
  | 'pipelines'
  | 'relay'
  | 'attachments'
  | 'jira-sync'
  | 'slack';

export type SettingsSectionGroup = 'host' | 'universal';

export type SettingsSectionInitialState = {
  general: undefined;
  repos: { repoId?: string } | undefined;
  organizations: { organizationId?: string } | undefined;
  'remote-projects':
    | { organizationId?: string; projectId?: string }
    | undefined;
  agents: { executor?: string; variant?: string } | undefined;
  mcp: undefined;
  'cli-tools': undefined;
  aws: undefined;
  pipelines: undefined;
  relay: { hostId?: string } | undefined;
  attachments: undefined;
  'jira-sync': { organizationId?: string; projectId?: string } | undefined;
  slack: { organizationId?: string } | undefined;
};

export interface SettingsSectionDefinition {
  id: SettingsSectionType;
  icon: Icon;
  group: SettingsSectionGroup;
}

export const SETTINGS_SECTION_DEFINITIONS: SettingsSectionDefinition[] = [
  { id: 'general', icon: GearIcon, group: 'host' },
  { id: 'repos', icon: GitBranchIcon, group: 'host' },
  { id: 'agents', icon: CpuIcon, group: 'host' },
  { id: 'mcp', icon: PlugIcon, group: 'host' },
  { id: 'cli-tools', icon: WrenchIcon, group: 'host' },
  { id: 'aws', icon: KeyIcon, group: 'host' },
  { id: 'pipelines', icon: ListChecksIcon, group: 'host' },
  { id: 'organizations', icon: BuildingsIcon, group: 'universal' },
  { id: 'remote-projects', icon: CloudIcon, group: 'universal' },
  { id: 'relay', icon: BroadcastIcon, group: 'universal' },
  { id: 'attachments', icon: PaperclipIcon, group: 'universal' },
  { id: 'jira-sync', icon: KanbanIcon, group: 'universal' },
  { id: 'slack', icon: SlackLogoIcon, group: 'universal' },
];

function RouteScopedMcpSettingsSection() {
  const { projectId } = useParams({ strict: false });

  return projectId ? (
    <ProjectProvider projectId={projectId}>
      <McpSettingsSection />
    </ProjectProvider>
  ) : (
    <McpSettingsSection />
  );
}

export function isHostSpecificSettingsSection(
  type: SettingsSectionType
): boolean {
  return (
    SETTINGS_SECTION_DEFINITIONS.find((section) => section.id === type)
      ?.group === 'host'
  );
}

export function renderSettingsSection(
  type: SettingsSectionType,
  initialState?: SettingsSectionInitialState[SettingsSectionType],
  onClose?: () => void
) {
  switch (type) {
    case 'general':
      return <GeneralSettingsSection />;
    case 'repos':
      return (
        <ReposSettingsSection
          initialState={initialState as SettingsSectionInitialState['repos']}
        />
      );
    case 'organizations':
      return <OrganizationsSettingsSection />;
    case 'remote-projects':
      return (
        <RemoteProjectsSettingsSection
          initialState={
            initialState as SettingsSectionInitialState['remote-projects']
          }
        />
      );
    case 'agents':
      return <AgentsSettingsSection />;
    case 'mcp':
      return <RouteScopedMcpSettingsSection />;
    case 'cli-tools':
      return <CliToolsSettingsSection />;
    case 'aws':
      return <AwsSettingsSection />;
    case 'pipelines':
      return <PipelinesSettingsSection />;
    case 'relay':
      return (
        <RelaySettingsSectionContent
          initialState={initialState as SettingsSectionInitialState['relay']}
          onClose={onClose}
        />
      );
    case 'attachments':
      return <AttachmentsSettingsSection />;
    case 'jira-sync':
      return (
        <JiraSyncSettingsSection
          initialState={
            initialState as SettingsSectionInitialState['jira-sync']
          }
        />
      );
    case 'slack':
      return (
        <SlackSettingsSection
          initialState={initialState as SettingsSectionInitialState['slack']}
        />
      );
    default:
      return <GeneralSettingsSection />;
  }
}
